#!/usr/bin/env python3
"""Hall of Fame survey: every time_trial/mastery route's ranked attempts left (op=status) joined with the
speed scoreboard (undocumented /api/speed-scoreboard) - our rank/elapsed vs the leaders.

  hof_survey.py [--days 1-15] [--json OUT]      (token: $HUNTRESS_GM_TOKEN or ~/.huntress_token)

Routes come from ctf-run's cache (~/.cache/ctf-run/routes.json; `ctf-run routes --refresh`).
"""
import json, os, re, sys, urllib.parse, urllib.request
from concurrent.futures import ThreadPoolExecutor

API = "https://2026.huntress.ctf.games"
TEAM = "help"
TOKEN = os.environ.get("HUNTRESS_GM_TOKEN") or open(os.path.expanduser("~/.huntress_token")).read().strip()
ROUTES = os.path.expanduser("~/.cache/ctf-run/routes.json")
# Only ctf-run's lanes: new lanes (web-exploitation from Day 20) must not be touched at all, not even by status queries.
LANES = ("warmups-", "metadata-forensics-", "script-analysis-", "binary-reverse-engineering-", "mobile-android-")


def get(url):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {TOKEN}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read())


def survey(rid):
    base, mode = rid.split("/")
    out = {"route": rid}
    try:
        st = get(f"{API}/?op=status&challenge={urllib.parse.quote(rid)}")
        for k in ("ranked_attempts_remaining", "unique_attempts_remaining", "regular_variants_remaining",
                  "hall_of_fame_eligible", "run_for_time", "result", "deadline", "error"):
            out[k] = st.get(k)
        # Dual-pool routes report ranked_attempts_remaining; timed-by-default ones only unique_attempts_remaining.
        out["left"] = next((st[k] for k in ("ranked_attempts_remaining", "unique_attempts_remaining") if isinstance(st.get(k), int)), None)
    except Exception as e:  # noqa: BLE001
        out["status_error"] = repr(e)[:120]
    try:
        q = urllib.parse.urlencode({"mode": mode, "view": "rankings", "limit": 200, "offset": 0,
                                    "sort": "elapsed_ms", "direction": "asc", "challenge": base})
        rows = get(f"{API}/api/speed-scoreboard?{q}").get("rows", [])
        me = next((r for r in rows if r.get("team_name") == TEAM), None)
        # `cohort` + `par_ms` differ between teams on one route (the pool/generation an attempt came from; d10
        # mastery mixes par 200 s and 28 s entries in one ranking). Rows carry no timestamp.
        out.update(teams=len(rows), top=[r["elapsed_ms"] for r in rows[:3]],
                   rank=me and me["rank"], elapsed=me and me["elapsed_ms"],
                   cohort=me and me.get("cohort"), par=me and me.get("par_ms"),
                   top_cohorts=[r.get("cohort") for r in rows[:3]])
    except Exception as e:  # noqa: BLE001
        out["board_error"] = repr(e)[:120]
    return out


def main():
    a = sys.argv[1:]
    lo, hi = 1, 99
    if "--days" in a:
        lo, hi = map(int, a[a.index("--days") + 1].split("-"))
    cache = json.load(open(ROUTES))
    rows = cache.get("challenges", cache) if isinstance(cache, dict) else cache
    cfg = {c.get("id"): c for c in rows}
    rids = []
    for c in rows:
        rid = c.get("id", "")
        m = re.search(r"-day-(\d+)", rid)
        if rid.startswith(LANES) and "/" in rid and rid.split("/")[1] in ("time_trial", "mastery") and m and lo <= int(m.group(1)) <= hi:
            rids.append(rid)
    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(survey, sorted(rids)))
    for r in res:
        c = cfg.get(r["route"], {})
        r.update(rft=c.get("run_for_time_supported"), deadline_ms=c.get("ranked_deadline_ms"), pool=c.get("ranked_pool_size"))
        left = r.get("left")
        top = r.get("top") or []
        gap = (r["elapsed"] - top[0]) if r.get("elapsed") and top else None
        print(f"{r['route']:72} rft={str(r['rft'])[0]} dl={str(r['deadline_ms']):>6} pool={str(r['pool']):>4} left={str(left):>4} rank={str(r.get('rank')):>4}/{str(r.get('teams')):<3}"
              f" ours={str(r.get('elapsed')):>6} top={top} gap={gap} {r.get('status_error') or r.get('board_error') or ''}")
    if "--json" in a:
        json.dump(res, open(a[a.index("--json") + 1], "w"), indent=1)


if __name__ == "__main__":
    main()
