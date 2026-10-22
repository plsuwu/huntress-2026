#!/usr/bin/env python3
"""Leaderboard campaign over many routes (run it on the run host): spends ranked attempts with the solver until each
route is good enough, judged by the live scoreboard. Every run is a real ranked attempt - run only with approval.

  campaign.py PLAN.json OUT.jsonl [DONE_ROUTE,...]   (the third argument resumes a stopped campaign)

PLAN.json: [{"route": "<base>/<mode>", "rft": bool, "budget": N, "past_first": bool?}, ...] (see survey.py --plan).
`rft`: dual-pool route (ranked = retry + timed flag); false = timed by default (retry + one-shot flag).
Per route: up to `budget` runs; stop early on #1 (unless past_first), after `patience` runs in a row without a better
time, when no attempts are left, or on ANY result other than CORRECT (investigate before spending more). Transient
server errors (5xx / temporarily unavailable) back off 30 s and retry, at most twice per route - but check whether a
5xx on submit consumed the attempt. Lanes with `hedge = true` add the hedge flag. One JSON line per run in OUT.jsonl.
Launch detached: setsid nohup python3 -u tools/campaign.py PLAN OUT > LOG 2>&1 < /dev/null &
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import orchcfg  # noqa: E402

C = orchcfg.load()
F = C["solver"]["flags"]
K = C["campaign"]
SB = C["api"]


def get(key, tries=4, **kw):
    """Bookkeeping queries ride out the server's 503s with backoff."""
    for k in range(tries):
        try:
            return C.api_get(key, timeout=120, **kw)
        except Exception:  # noqa: BLE001
            if k == tries - 1:
                raise
            time.sleep(10 * (k + 1))


def transient(rec):
    tail = " ".join(rec.get("stderr_tail") or [])
    return rec.get("result") is None and ("503" in tail or "temporarily_unavailable" in tail)


def board(route):
    if not SB.get("scoreboard"):
        return None, None, []
    base, mode = route.split("/")
    rows = get("scoreboard", mode=mode, base=base).get(SB["scoreboard_rows"], [])
    me = next((r for r in rows if r.get(SB["scoreboard_team"]) == C["event"]["team"]), None)
    return (me or {}).get(SB["scoreboard_rank"]), (me or {}).get(SB["scoreboard_time"]), [r[SB["scoreboard_time"]] for r in rows[:3]]


def left(route):
    st = get("status", id=route)
    live = st.get("deadline") and st.get("result") not in ("CORRECT", "expired") and not st.get("practice")
    n = next((st[k] for k in ("ranked_attempts_remaining", "unique_attempts_remaining") if isinstance(st.get(k), int)), None)
    return n, bool(live), st


def run(route, rft):
    base, mode = route.split("/")
    args = [C.solver_bin(), base, C.mode_flag(mode), F["retry"], F["no_preflight"], F["url"], C["event"]["api"],
            F["out"], "runs-ranked"]
    args += [F["timed"]] if rft else [F["one_shot_ok"]]
    lane = C.lane_of(base)
    if lane and lane.get("hedge"):
        args.append(F["hedge"])
    t0 = time.time()
    p = subprocess.run(args, capture_output=True, text=True, env=C.solver_env(ranked=True), timeout=900, cwd=C.solver_dir)
    rec = {"route": route, "rc": p.returncode, "wall_s": round(time.time() - t0, 1)}
    for line in p.stdout.strip().splitlines()[-1:]:  # the solver prints the final submit response as JSON
        try:
            r = json.loads(line)
            rec.update(result=r.get("result"), hof=r.get("hall_of_fame_eligible"),
                       left=r.get("ranked_attempts_remaining", r.get("unique_attempts_remaining")))
        except ValueError:
            pass
    saved = [l.split("saved", 1)[1].strip() for l in p.stderr.splitlines() if "] saved " in l]
    rec["dir"] = saved[0] if saved else None
    if rec.get("result") is None:
        rec["stderr_tail"] = p.stderr.strip().splitlines()[-6:]
    return rec


def main():
    plan, out = json.load(open(sys.argv[1])), sys.argv[2]
    skip = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else set()
    for item in plan:
        route, rft, budget = item["route"], item["rft"], item["budget"]
        if route in skip:
            continue
        if not C.in_scope(route):
            print(f"== {route}: not a registered lane - skipped", flush=True)
            continue
        rank, best, top = board(route)
        print(f"== {route}: rank {rank} best {best} top {top}, budget {budget}", flush=True)
        stale, hiccups = 0, 0
        n, live, _ = left(route)
        if live:
            print("   live attempt in progress - skipping route", flush=True)
            continue
        i = 0
        while i < budget:
            if rank == 1 and not item.get("past_first"):
                print("   #1 - done", flush=True)
                break
            if n is not None and n <= 0:
                print("   no attempts left", flush=True)
                break
            time.sleep(K["gap_s"])
            rec = run(route, rft)
            new_rank, new_best, top = board(route)
            rec.update(run=i + 1, rank_before=rank, best_before=best, rank_after=new_rank, best_after=new_best, top=top)
            with open(out, "a") as f:
                f.write(json.dumps(rec) + "\n")
            print(f"   run {i + 1}: {rec.get('result')} rank {rank}->{new_rank} best {best}->{new_best} top {top} "
                  f"left {rec.get('left')}", flush=True)
            if transient(rec) and hiccups < 2:
                hiccups += 1
                print("   server 503 - backing off 30 s and retrying", flush=True)
                time.sleep(30)
                continue  # same run index: a transient error does not use up budget
            if rec.get("result") != "CORRECT":
                print(f"   STOP: {rec.get('stderr_tail') or rec.get('result')}", flush=True)
                break
            improved = best is None or (new_best is not None and new_best < best)
            stale = 0 if improved else stale + 1
            rank, best = new_rank, new_best
            n = rec.get("left", n)
            i += 1
            if stale >= K["patience"]:
                print("   no improvement in a row - next route", flush=True)
                break


if __name__ == "__main__":
    main()
