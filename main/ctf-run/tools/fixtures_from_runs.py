#!/usr/bin/env python3
"""Turns graded ctf-run run directories into permanent replay fixtures.

  python3 -I tools/fixtures_from_runs.py RUN_DIR... [--slug SLUG]

For each run whose saved result is CORRECT: copies start.json + deliveries into
../utils/solver_py_scripts/day-NN-<lane>/<slug>/artifacts/<mode>/ and writes
fixtures/<lane>/dNN-<mode>.json (graded "correct", expected = the submitted answers).
The slug comes from the challenge id (`<lane>-day-NN-<slug>`); warmups use the day's prompt-free id,
so pass --slug for them.
"""
import json
import os
import re
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SOLVERS = os.path.normpath(os.path.join(ROOT, "..", "utils", "solver_py_scripts"))
LANES = {"warmups": "warmups", "mobile-android": "mobile", "metadata-forensics": "metadata",
         "binary-reverse-engineering": "binary_re", "script-analysis": "script"}


def one(run, slug_override):
    meta = json.load(open(os.path.join(run, "meta.json")))
    submits = json.load(open(os.path.join(run, "result.json")))
    result = (submits[-1].get("response") or {}) if isinstance(submits, list) and submits else {}
    if result.get("result") != "CORRECT":
        print(f"skip {run}: result {result.get('result')!r}")
        return
    challenge = meta["challenge"]
    base, _, mode = challenge.partition("/")
    mode = mode or "single_solve"
    m = re.match(r"^(.*)-day-(\d+)(?:-(.*))?$", base)
    lane, day, slug = m.group(1), int(m.group(2)), slug_override or m.group(3)
    if not slug:
        sys.exit(f"{challenge}: no slug in the id; pass --slug")
    # Ranked (run_for_time) attempts get their own artifact dir and fixture name, so they never
    # overwrite the regular attempt's copies.
    tag = mode + ("_ranked" if meta.get("run_for_time") else "")
    dest = os.path.join(SOLVERS, f"day-{day:02d}-{lane}", slug, "artifacts", tag)
    os.makedirs(dest, exist_ok=True)
    shutil.copy(os.path.join(run, "start.json"), os.path.join(dest, "start.json"))
    deliveries = []
    for name in sorted(os.listdir(os.path.join(run, "deliveries"))):
        shutil.copy(os.path.join(run, "deliveries", name), os.path.join(dest, name))
        deliveries.append({"path": os.path.relpath(os.path.join(dest, name), os.path.join(ROOT, "fixtures", LANES[lane])),
                           "name": name.split("-", 1)[1] if re.match(r"^\d+-", name) else name})
    answers = json.load(open(os.path.join(run, "answers.json")))
    answers = answers.get("answers", answers)
    manifest = {
        "challenge": challenge,
        "start": os.path.relpath(os.path.join(dest, "start.json"), os.path.join(ROOT, "fixtures", LANES[lane])),
        "deliveries": deliveries,
        "expected": answers,
        "graded": "correct",
        "note": f"ctf-run {os.path.basename(run)}, graded CORRECT",
    }
    out = os.path.join(ROOT, "fixtures", LANES[lane], f"d{day:02d}-{tag}.json")
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
    for run in a:
        one(run, slug)


if __name__ == "__main__":
    main()
