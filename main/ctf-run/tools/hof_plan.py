#!/usr/bin/env python3
"""Build a hof_campaign.py plan from a hof_survey.py --json survey.

  hof_plan.py SURVEY.json OUT.json [--days A-B] [--unattempted] [--exclude PLAN.json,...] [--order PREFIX,...]

Every route with ranked attempts left gets an entry (the Day 22 full-pass rules):
  - not #1: budget 3, or 2 on a pool of <= 16, or 1 on a pool of <= 2 (never more than `left`);
  - already #1: budget 1 with `past_first` (try to beat our own time).
Routes that spent ranked attempts but never got a scoreboard row are skipped (not HoF-eligible).
--order runs routes by lane prefix in that order (e.g. warmups,mobile-android,metadata-forensics first: the
server-bound lanes), unlisted lanes last; within a lane, routes not yet #1 come first.
--unattempted keeps only routes with no scoreboard entry yet; --exclude drops routes listed in other plans.
`rft` comes from the route config (survey `rft`), not the last attempt's `run_for_time`.
"""
import json, re, sys


def budget(r):
    if r.get("rank") == 1:
        return 1
    pool = r.get("pool") or 0
    b = 1 if pool <= 2 else 2 if pool <= 16 else 3
    return min(b, r["left"])


def main():
    a = sys.argv[1:]
    survey, out = a[0], a[1]
    lo, hi = 1, 99
    if "--days" in a:
        lo, hi = map(int, a[a.index("--days") + 1].split("-"))
    skip = set()
    if "--exclude" in a:
        for p in a[a.index("--exclude") + 1].split(","):
            skip |= {i["route"] for i in json.load(open(p))}
    plan = []
    for r in json.load(open(survey)):
        day = int(re.search(r"day-(\d+)", r["route"]).group(1))
        if not lo <= day <= hi or r["route"] in skip or not (r.get("left") or 0) > 0:
            continue
        if "--unattempted" in a and r.get("rank") is not None:
            continue
        # Ranked attempts spent but still no scoreboard row: not HoF-eligible (binary d01/d02, metadata d01).
        if r.get("rank") is None and r.get("pool") and r["left"] < r["pool"]:
            continue
        item = {"route": r["route"], "rft": bool(r["rft"]), "budget": budget(r)}
        if r.get("rank") == 1:
            item["past_first"] = True
        plan.append(item)
    if "--order" in a:
        order = a[a.index("--order") + 1].split(",")
        lane = lambda i: next((k for k, p in enumerate(order) if i["route"].startswith(p)), len(order))
        plan.sort(key=lambda i: (lane(i), bool(i.get("past_first"))))
    json.dump(plan, open(out, "w"), indent=0)
    runs = sum(i["budget"] for i in plan)
    print(f"{len(plan)} routes, up to {runs} ranked runs -> {out}")


if __name__ == "__main__":
    main()
