#!/usr/bin/env python3
"""Turn graded run directories into permanent replay fixtures.

  python3 -I tools/fixtures_from_runs.py RUN_DIR... [--slug SLUG]

For each run whose saved verdict is CORRECT: copies start.json + deliveries into solver.artifacts_dir and writes
<fixtures_dir>/<lane fixtures>/dNN-<mode>.json with the submitted answers as the expected answers. The lane comes from
the registry (challenge-id prefix); the day and slug from the id (`<prefix><NN>[-<slug>]`), or pass --slug when the id
has no slug part. Ranked runs get their own `<mode>_ranked` fixture so they never overwrite the regular one.
Run directory layout: meta.json ("challenge", optional "run_for_time"), start.json, deliveries/, answers.json,
result.json (a list of submit records {label, ms, response}; the last is the verdict - or one result object).
"""
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import orchcfg  # noqa: E402

C = orchcfg.load()


def verdict(run):
    res = json.load(open(os.path.join(run, "result.json")))
    if isinstance(res, list):
        res = (res[-1].get("response") or {}) if res else {}
    return res.get("result")


def one(run, slug_override):
    meta = json.load(open(os.path.join(run, "meta.json")))
    if verdict(run) != "CORRECT":
        print(f"skip {run}: verdict {verdict(run)!r}")
        return
    challenge = meta["challenge"]
    base, _, mode = challenge.partition("/")
    mode = mode or C.single_mode
    lane = C.lane_of(base)
    if not lane:
        sys.exit(f"{challenge}: no registered lane owns this id")
    m = re.match(r"^(\d+)(?:-(.*))?$", base[len(lane["prefix"]):])
    if not m:
        sys.exit(f"{challenge}: expected <prefix><NN>[-slug]")
    day, slug = int(m.group(1)), slug_override or m.group(2)
    if not slug:
        sys.exit(f"{challenge}: no slug in the id; pass --slug")
    tag = mode + ("_ranked" if meta.get("run_for_time") else "")
    # Resolve through symlinks: lane worktrees symlink the fixtures dir to the main checkout's, and manifest paths are
    # relative to the real fixtures directory, so artifacts must land beside the real one.
    fixdir = os.path.realpath(C.fixtures_dir(lane["name"]))
    root = os.path.realpath(C.repo)
    dest = os.path.join(root, C.fmt(C["solver"]["artifacts_dir"], day=day, lane=lane["name"], slug=slug, mode=tag))
    os.makedirs(dest, exist_ok=True)
    os.makedirs(fixdir, exist_ok=True)
    shutil.copy(os.path.join(run, "start.json"), os.path.join(dest, "start.json"))
    deliveries = []
    for name in sorted(os.listdir(os.path.join(run, "deliveries"))):
        shutil.copy(os.path.join(run, "deliveries", name), os.path.join(dest, name))
        deliveries.append({"path": os.path.relpath(os.path.join(dest, name), fixdir),
                           "name": name.split("-", 1)[1] if re.match(r"^\d+-", name) else name})
    answers = json.load(open(os.path.join(run, "answers.json")))
    answers = answers.get("answers", answers)
    manifest = {
        "challenge": challenge,
        "start": os.path.relpath(os.path.join(dest, "start.json"), fixdir),
        "deliveries": deliveries,
        "expected": answers,
        "graded": "correct",
        "note": f"{os.path.basename(run)}, graded CORRECT",
    }
    out = os.path.join(fixdir, f"d{day:02d}-{tag}.json")
    with open(out, "w") as f:
        json.dump(manifest, f, indent=1)
        f.write("\n")
    print(f"{out} <- {run} ({len(answers)} answers)")


def main():
    a = sys.argv[1:]
    slug = None
    if "--slug" in a:
        i = a.index("--slug")
        slug = a[i + 1]
        del a[i:i + 2]
    if not a:
        sys.exit(__doc__)
    for run in a:
        one(run, slug)


if __name__ == "__main__":
    main()
