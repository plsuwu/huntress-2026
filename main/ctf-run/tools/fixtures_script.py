#!/usr/bin/env python3
"""Writes replay manifests for the script-analysis lane: ctf-run/fixtures/script/<case>.json.

Every utils/solver_py_scripts/day-NN-script-analysis/fixtures/<mode>/ directory with a start.json and saved answers
becomes a case. Deliveries are the saved downloads (numbered `N-*` files when present, else `evidence*.zip`), in
order. Expected answers: `final.json` (oracle-verified) when present, else `answers.json`; graded = "correct" when the
saved result says CORRECT (or final.json exists), else "solver".

usage: python3 -I tools/fixtures_script.py
"""
import glob
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.dirname(CRATE)
PY = os.path.join(ROOT, "utils", "solver_py_scripts")
OUT = os.path.join(CRATE, "fixtures", "script")


def rel(p):
    return os.path.relpath(p, OUT)


def is_zip(p):
    with open(p, "rb") as f:
        return f.read(2) == b"PK"


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for d in sorted(glob.glob(os.path.join(PY, "day-*-script-analysis", "fixtures", "*"))):
        if not os.path.isdir(d) or not os.path.exists(os.path.join(d, "start.json")):
            continue
        day = re.search(r"day-(\d\d)", d).group(1)
        mode_dir = os.path.basename(d)
        files = sorted(os.listdir(d))
        exp_file = "final.json" if "final.json" in files else ("answers.json" if "answers.json" in files else None)
        if not exp_file:
            continue
        numbered = [f for f in files if re.match(r"\d+-", f) and is_zip(os.path.join(d, f))]
        zips = numbered or [f for f in files if f.startswith("evidence") and f.endswith(".zip") and is_zip(os.path.join(d, f))]
        if not zips:
            continue
        expected = json.load(open(os.path.join(d, exp_file)))
        expected = expected.get("answers", expected)
        graded = "correct" if exp_file == "final.json" else "solver"
        for r in ("result.json", "submit-result.json"):
            if r in files:
                try:
                    if json.load(open(os.path.join(d, r))).get("result") == "CORRECT":
                        graded = "correct"
                except (ValueError, AttributeError):
                    pass
        mode = "mastery" if mode_dir.startswith("mastery") else "time_trial" if mode_dir.startswith("time-trial") else "single_solve"
        json.dump({
            "challenge": f"script-analysis-day-{day}" + {"mastery": "/mastery", "time_trial": "/time_trial"}.get(mode, ""),
            "start": rel(os.path.join(d, "start.json")),
            "deliveries": [{"path": rel(os.path.join(d, z)), "name": z.split("-", 1)[1] if re.match(r"\d+-", z) else z} for z in zips],
            "expected": expected,
            "graded": graded,
        }, open(os.path.join(OUT, f"d{day}-{mode_dir}.json"), "w"), indent=1)
        count += 1
    print(f"wrote {count} manifests to {OUT}")


if __name__ == "__main__":
    main()
