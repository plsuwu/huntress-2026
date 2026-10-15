#!/usr/bin/env python3
"""Hall of Fame campaign over many routes (jumpbox): spends ranked attempts with ctf-run until each route is
good enough, judged by the live speed scoreboard.

  hof_campaign.py PLAN.json OUT.jsonl [DONE_ROUTE,...]   (the third argument resumes a stopped campaign)

PLAN.json: [{"route": "<base>/<mode>", "rft": bool, "budget": N, "past_first": bool?}, ...]
(`past_first`: keep improving an uncontested #1 - stop only on budget/patience) (see hof_survey.py for the data).
Per route: up to `budget` runs; stop early when we are #1, after PATIENCE runs in a row that did not beat our best
scoreboard time, when no attempts are left, or on any result other than CORRECT (look before spending more).
Every run is a real ranked attempt: `--retry --timed` (dual-pool) or `--retry --one-shot-ok` (timed by default).
Small-delivery lanes add `--hedge`. One JSON line per run in OUT.jsonl; progress on stdout.
"""
import json, os, subprocess, sys, time, urllib.parse, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.path.join(ROOT, "target", "release", "ctf-run")
API = "https://2026.huntress.ctf.games"
TEAM = "help"
GAP_S = 8
PATIENCE = 3
HEDGE_LANES = ("warmups-", "mobile-android-", "metadata-forensics-")
TOKEN = os.environ.get("HUNTRESS_GM_TOKEN") or open(os.path.expanduser("~/.huntress_token")).read().strip()


def get(url, tries=4):
    """Our own bookkeeping queries ride out the server's 503s with backoff (it 503s under load)."""
    for k in range(tries):
        try:
            req = urllib.request.Request(url, headers={"Authorization": f"Bearer {TOKEN}"})
            with urllib.request.urlopen(req, timeout=120) as r:
                return json.loads(r.read())
        except Exception:  # noqa: BLE001
            if k == tries - 1:
                raise
            time.sleep(10 * (k + 1))


def transient(rec):
    """A server-side hiccup (503 / temporarily_unavailable), not a wrong answer: worth a later retry."""
    tail = " ".join(rec.get("stderr_tail") or [])
    return rec.get("result") is None and ("503" in tail or "temporarily_unavailable" in tail)


def board(route):
    base, mode = route.split("/")
    q = urllib.parse.urlencode({"mode": mode, "view": "rankings", "limit": 200, "offset": 0,
                                "sort": "elapsed_ms", "direction": "asc", "challenge": base})
    rows = get(f"{API}/api/speed-scoreboard?{q}").get("rows", [])
    me = next((r for r in rows if r.get("team_name") == TEAM), None)
    return (me or {}).get("rank"), (me or {}).get("elapsed_ms"), [r["elapsed_ms"] for r in rows[:3]]


def left(route):
    st = get(f"{API}/?op=status&challenge={urllib.parse.quote(route)}")
    live = st.get("deadline") and st.get("result") not in ("CORRECT", "expired") and not st.get("practice")
    n = next((st[k] for k in ("ranked_attempts_remaining", "unique_attempts_remaining") if isinstance(st.get(k), int)), None)
    return n, bool(live), st


def run(route, rft):
    base, mode = route.split("/")
    args = [BIN, base, "-t" if mode == "time_trial" else "-m", "--retry", "--no-preflight", "-u", API, "-o", "runs-hof"]
    args += ["--timed"] if rft else ["--one-shot-ok"]
    if base.startswith(HEDGE_LANES):
        args.append("--hedge")
    env = dict(os.environ, HUNTRESS_GM_TOKEN=TOKEN)
    t0 = time.time()
    p = subprocess.run(args, capture_output=True, text=True, env=env, timeout=900, cwd=ROOT)
    rec = {"route": route, "rc": p.returncode, "wall_s": round(time.time() - t0, 1)}
    for line in p.stdout.strip().splitlines()[-1:]:
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
    skip = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else set()  # routes already handled (resume)
    for item in plan:
        if item["route"] in skip:
            continue
        route, rft, budget = item["route"], item["rft"], item["budget"]
        rank, best, top = board(route)
        print(f"== {route}: rank {rank} best {best} top {top}, budget {budget}", flush=True)
        stale, hiccups = 0, 0
        # One status check per route (multi-MB for masteries); afterwards each run reports attempts left itself.
        n, live, _ = left(route)
        if live:
            print("   live attempt in progress - skipping route", flush=True); continue
        for i in range(budget):
            if rank == 1 and not item.get("past_first"):
                print("   #1 - done", flush=True); break
            if n is not None and n <= 0:
                print("   no attempts left", flush=True); break
            time.sleep(GAP_S)
            rec = run(route, rft)
            new_rank, new_best, top = board(route)
            rec.update(run=i + 1, rank_before=rank, best_before=best, rank_after=new_rank, best_after=new_best, top=top)
            with open(out, "a") as f:
                f.write(json.dumps(rec) + "\n")
            print(f"   run {i + 1}: {rec.get('result')} rank {rank}->{new_rank} best {best}->{new_best} top {top} left {rec.get('left')}", flush=True)
            if transient(rec) and hiccups < 2:
                hiccups += 1
                print("   server 503 - backing off 30 s and retrying", flush=True)
                time.sleep(30)
                continue
            if rec.get("result") != "CORRECT":
                print(f"   STOP: {rec.get('stderr_tail') or rec.get('result')}", flush=True); break
            improved = best is None or (new_best is not None and new_best < best)
            stale = 0 if improved else stale + 1
            rank, best = new_rank, new_best
            n = rec.get("left", n)
            if stale >= PATIENCE:
                print("   no improvement in a row - next route", flush=True); break


if __name__ == "__main__":
    main()
