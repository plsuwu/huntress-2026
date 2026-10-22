#!/usr/bin/env python3
"""Leaderboard survey: for every in-scope timed-mode route, attempts left (status) joined with the scoreboard (our
rank/time vs the leaders). Only routes of registered lanes are queried - never anything else the API lists.

  survey.py [--days A-B] [--json OUT] [--plan PLAN.json]

Routes come from the solver's route cache (refresh it first). --plan writes a campaign plan: every route with attempts
left and a rank (or no entry yet); budget = campaign.default_budget (small_pool_budget on pools <= small_pool), and one
`past_first` run on routes where we are already #1. Edit the plan before running it.
Note: eligibility flags may read false just because the latest attempt was untimed; attempts-left is the real gate.
"""
import json
import os
import re
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import orchcfg  # noqa: E402

C = orchcfg.load()
SB = C["api"]


def board(route):
    base, mode = route.split("/")
    rows = C.api_get("scoreboard", timeout=120, mode=mode, base=base).get(SB["scoreboard_rows"], [])
    me = next((r for r in rows if r.get(SB["scoreboard_team"]) == C["event"]["team"]), None)
    return {"teams": len(rows), "top": [r[SB["scoreboard_time"]] for r in rows[:3]],
            "rank": me and me[SB["scoreboard_rank"]], "elapsed": me and me[SB["scoreboard_time"]]}


def survey(rid):
    out = {"route": rid}
    try:
        st = C.api_get("status", timeout=120, id=rid)
        for k in ("ranked_attempts_remaining", "unique_attempts_remaining", "regular_variants_remaining",
                  "hall_of_fame_eligible", "run_for_time", "result", "deadline", "error"):
            out[k] = st.get(k)
        # Dual-pool routes report ranked attempts; timed-by-default ones only unique attempts.
        out["left"] = next((st[k] for k in ("ranked_attempts_remaining", "unique_attempts_remaining")
                            if isinstance(st.get(k), int)), None)
    except Exception as e:  # noqa: BLE001
        out["status_error"] = repr(e)[:120]
    if SB.get("scoreboard"):
        try:
            out.update(board(rid))
        except Exception as e:  # noqa: BLE001
            out["board_error"] = repr(e)[:120]
    return out


def main():
    a = sys.argv[1:]
    lo, hi = C["event"]["first_day"], C["event"]["last_day"]
    if "--days" in a:
        lo, hi = map(int, a[a.index("--days") + 1].split("-"))
    cache = json.load(open(C.route_cache()))
    rows = cache.get("challenges", cache) if isinstance(cache, dict) else cache
    cfg = {c.get("id"): c for c in rows}
    rids = []
    for c in rows:
        rid = c.get("id", "")
        if not C.in_scope(rid) or "/" not in rid or rid.split("/")[1] not in C.timed_modes:
            continue
        day = c.get("day")
        if day is None:
            m = re.search(r"-day-(\d+)", rid)
            day = int(m.group(1)) if m else None
        if day is not None and lo <= int(day) <= hi:
            rids.append(rid)
    with ThreadPoolExecutor(4) as ex:  # small bounded concurrency: never saturate the shared endpoint
        res = list(ex.map(survey, sorted(rids)))
    for r in res:
        c = cfg.get(r["route"], {})
        r.update(rft=c.get("run_for_time_supported"), deadline_ms=c.get("ranked_deadline_ms"), pool=c.get("ranked_pool_size"))
        top = r.get("top") or []
        gap = (r["elapsed"] - top[0]) if r.get("elapsed") and top else None
        print(f"{r['route']:72} rft={str(r['rft'])[0]} pool={str(r['pool']):>4} left={str(r.get('left')):>4} "
              f"rank={str(r.get('rank')):>4}/{str(r.get('teams')):<3} ours={str(r.get('elapsed')):>6} top={top} gap={gap} "
              f"{r.get('status_error') or r.get('board_error') or ''}")
    if "--json" in a:
        json.dump(res, open(a[a.index("--json") + 1], "w"), indent=1)
    if "--plan" in a:
        k = C["campaign"]
        plan = []
        for r in res:
            if (r.get("left") or 0) <= 0:
                continue
            if r.get("rank") == 1:
                plan.append({"route": r["route"], "rft": bool(r["rft"]), "budget": 1, "past_first": True})
            else:
                small = (r.get("pool") or 10**9) <= k["small_pool"]
                plan.append({"route": r["route"], "rft": bool(r["rft"]),
                             "budget": k["small_pool_budget"] if small else k["default_budget"]})
        json.dump(plan, open(a[a.index("--plan") + 1], "w"), indent=1)
        print(f"plan: {len(plan)} routes, at most {sum(p['budget'] for p in plan)} runs")


if __name__ == "__main__":
    main()
