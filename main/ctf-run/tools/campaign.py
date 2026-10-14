#!/usr/bin/env python3
"""Timed-run driver for the jumpbox: latency experiments and Hall-of-Fame campaigns.

  python3 tools/campaign.py experiment ROUTE --rounds 5 --arms proxy,direct,proxy+warm,direct+warm
  python3 tools/campaign.py campaign --per-route 5 --arm direct+warm [--lanes warmups,mobile] [--days 4-13]
  python3 tools/campaign.py summary campaign/<file>.jsonl

Arms: `proxy` (http://10.0.0.219/huntress-ctf-proxy) or `direct` (https://2026.huntress.ctf.games, bearer token
from ~/.huntress_token), optionally `+warm` (ctf-run --warm 6). Every run is a real timed attempt
(`--retry --timed` on dual-pool routes, `--retry --one-shot-ok` on timed-by-default ones). Before each run the route's
status is checked: a live attempt is waited out, and routes with no attempts left are skipped.

Metric (server clock): `submit_sent - started_at` (the timer runs from `started_at` to the server receiving the
submit; client clock offset is constant across arms). Results go to campaign/<name>.jsonl, one object per run.
"""
import json
import os
import subprocess
import sys
import time
import urllib.request
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.path.join(ROOT, "target", "release", "ctf-run")
PROXY = "http://10.0.0.219/huntress-ctf-proxy"
DIRECT = "https://2026.huntress.ctf.games"
ROUTES = os.path.expanduser("~/.cache/ctf-run/routes.json")
# Days whose fixtures the lanes reproduce (see PLAN.md); timed runs elsewhere are not attempted.
VERIFIED = {"warmups": range(5, 14), "mobile-android": range(4, 14), "metadata-forensics": range(5, 14),
            "binary-reverse-engineering": range(4, 14), "script-analysis": range(4, 14)}


def token():
    with open(os.path.expanduser("~/.huntress_token")) as f:
        return f.read().strip()


def iso_ms(s):
    if not s:
        return None
    return datetime.strptime(s.replace("Z", "+0000"), "%Y-%m-%dT%H:%M:%S.%f%z").timestamp() * 1000


def status(route):
    req = urllib.request.Request(f"{PROXY}?op=status&challenge={route}")
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())


def remaining(st):
    for k in ("ranked_attempts_remaining", "unique_attempts_remaining"):
        if isinstance(st.get(k), int):
            return st[k]
    return None


def wait_if_live(route):
    """Waits out a live timed attempt (another runner or a person) instead of colliding with it."""
    for _ in range(40):
        st = status(route)
        dl = iso_ms(st.get("deadline"))
        live = dl and dl > time.time() * 1000 and st.get("result") not in ("CORRECT",)
        if not live:
            return st
        print(f"  live attempt until {st['deadline']}; waiting", flush=True)
        time.sleep(min(30, max(1, (dl - time.time() * 1000) / 1000 + 1)))
    return status(route)


def run_one(route, rft, arm, out_dir):
    """`arm` = endpoint [+modifiers]: `warm` (--warm 6), `nopre` (--no-preflight), `gapN` (sleep N s first)."""
    parts = arm.split("+")
    url = DIRECT if parts[0] == "direct" else PROXY
    args = [BIN, route, "--retry", "-u", url, "-o", out_dir]
    args += ["--timed"] if rft else ["--one-shot-ok"]
    for mod in parts[1:]:
        if mod == "warm":
            args += ["--warm", "6"]
        elif mod == "nopre":
            args += ["--no-preflight"]
        elif mod.startswith("gap"):
            time.sleep(float(mod[3:]))
    env = dict(os.environ)
    if url == DIRECT:
        env["HUNTRESS_GM_TOKEN"] = token()
    t0 = time.time()
    p = subprocess.run(args, capture_output=True, text=True, env=env, timeout=600, cwd=ROOT)
    rec = {"route": route, "arm": arm, "rc": p.returncode, "wall_s": round(time.time() - t0, 2)}
    saved = [l.split("saved", 1)[1].strip() for l in p.stderr.splitlines() if "] saved " in l]
    if p.stdout.strip().startswith("{"):
        try:
            r = json.loads(p.stdout.strip().splitlines()[-1])
            rec["result"] = r.get("result")
            rec["remaining"] = r.get("ranked_attempts_remaining", r.get("unique_attempts_remaining"))
            rec["hof"] = r.get("hall_of_fame_eligible")
        except ValueError:
            pass
    if saved and not saved[0].startswith("FAILED"):
        d = saved[0] if os.path.isabs(saved[0]) else os.path.join(ROOT, saved[0])
        rec["dir"] = d
        try:
            m = json.load(open(os.path.join(d, "meta.json")))
            c, s0 = m["clock"], iso_ms(m.get("started_at"))
            if s0:
                for k in ("start_sent", "start_received", "last_delivery", "submit_sent", "submit_received"):
                    v = iso_ms(c.get(k))
                    rec[k] = None if v is None else round(v - s0)
            rec["downloads"] = [round(x["ttfb_ms"]) for x in m.get("downloads", [])]
            rec["deadline_s"] = round((iso_ms(m.get("deadline")) - s0) / 1000, 1) if m.get("deadline") and s0 else None
        except (OSError, ValueError, KeyError) as e:
            rec["meta_error"] = repr(e)
    if p.returncode != 0 or rec.get("result") != "CORRECT":
        rec["stderr_tail"] = p.stderr.strip().splitlines()[-6:]
    return rec


def write(path, rec):
    with open(path, "a") as f:
        f.write(json.dumps(rec) + "\n")
    keys = ("route", "arm", "result", "submit_sent", "last_delivery", "start_received", "downloads", "remaining", "wall_s")
    print("  " + " ".join(f"{k}={rec.get(k)}" for k in keys), flush=True)


def routes_cfg():
    return {c["id"]: c for c in json.load(open(ROUTES))["challenges"]}


def experiment(route, rounds, arms, out):
    cfg = routes_cfg()[route]
    for r in range(rounds):
        for arm in arms:
            st = wait_if_live(route)
            if remaining(st) == 0:
                print("no attempts left"); return
            print(f"round {r + 1} {arm}", flush=True)
            write(out, run_one(route, cfg.get("run_for_time_supported", False), arm, "runs-exp"))


def campaign(per_route, arm, lanes, days, out, only=None):
    cfg = routes_cfg()
    todo = []
    for rid, c in sorted(cfg.items(), key=lambda kv: (kv[1].get("day", 0), kv[0])):
        lane, day, mode = c.get("lane"), c.get("day"), c.get("mode")
        if mode not in ("time_trial", "mastery") or lane not in VERIFIED or day not in VERIFIED[lane]:
            continue
        if (lanes and lane not in lanes) or (days and day not in days) or (only and rid not in only):
            continue
        todo.append(rid)
    print(f"{len(todo)} routes", flush=True)
    for rid in todo:
        rft = cfg[rid].get("run_for_time_supported", False)
        for i in range(per_route):
            try:
                st = wait_if_live(rid)
            except Exception as e:  # noqa: BLE001
                print(f"{rid}: status failed {e!r}"); break
            left = remaining(st)
            if left is not None and left <= 0:
                print(f"{rid}: no attempts left", flush=True)
                break
            print(f"{rid} run {i + 1}/{per_route} (left {left})", flush=True)
            rec = run_one(rid, rft, arm, "runs-campaign")
            write(out, rec)
            if rec.get("result") != "CORRECT":
                break  # look at it before spending more attempts on this route


def summary(path):
    import statistics as stt
    rows = [json.loads(l) for l in open(path)]
    by = {}
    for r in rows:
        by.setdefault((r["route"], r["arm"]), []).append(r)
    for (route, arm), rs in sorted(by.items()):
        ok = [r for r in rs if r.get("result") == "CORRECT" and r.get("submit_sent") is not None]
        t = sorted(r["submit_sent"] for r in ok)
        res = {}
        for r in rs:
            res[r.get("result")] = res.get(r.get("result"), 0) + 1
        line = f"{route:<70} {arm:<12} n={len(rs)} {res}"
        if t:
            line += f" submit_sent best={t[0]} med={stt.median(t):.0f} max={t[-1]} ms"
        print(line)


def main():
    a = sys.argv[1:]
    os.makedirs(os.path.join(ROOT, "campaign"), exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
    if a[0] == "experiment":
        rounds = int(a[a.index("--rounds") + 1]) if "--rounds" in a else 5
        arms = a[a.index("--arms") + 1].split(",") if "--arms" in a else ["proxy", "direct", "proxy+warm", "direct+warm"]
        experiment(a[1], rounds, arms, os.path.join(ROOT, "campaign", f"experiment-{stamp}.jsonl"))
    elif a[0] == "campaign":
        per = int(a[a.index("--per-route") + 1]) if "--per-route" in a else 5
        arm = a[a.index("--arm") + 1] if "--arm" in a else "proxy"
        lanes = a[a.index("--lanes") + 1].split(",") if "--lanes" in a else None
        days = None
        if "--days" in a:
            lo, hi = a[a.index("--days") + 1].split("-")
            days = range(int(lo), int(hi) + 1)
        only = a[a.index("--routes") + 1].split(",") if "--routes" in a else None
        campaign(per, arm, lanes, days, os.path.join(ROOT, "campaign", f"campaign-{stamp}.jsonl"), only)
    elif a[0] == "summary":
        summary(a[1])


if __name__ == "__main__":
    main()
