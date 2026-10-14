#!/usr/bin/env python3
"""Writes replay manifests for the binary-RE lane: ctf-run/fixtures/binary_re/<case>.json.

Every saved run directory under utils/solver_py_scripts/day-NN-binary-reverse-engineering/*/artifacts that holds a
start JSON, an answers JSON and its deliveries (evidence.zip or delivery1..N.zip) becomes a case. Expected answers are
the saved ones; `graded` is "correct" when a saved result/practice/submit JSON says CORRECT.

usage: python3 -I tools/fixtures_binary_re.py
"""
import glob
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.dirname(CRATE)
PY = os.path.join(ROOT, "utils", "solver_py_scripts")
OUT = os.path.join(CRATE, "fixtures", "binary_re")


def rel(p):
    return os.path.relpath(p, OUT)


def graded(d):
    for name in ("result.json", "submit_result.json", "practice.json", "submit.json"):
        p = os.path.join(d, name)
        if os.path.exists(p):
            try:
                if json.load(open(p)).get("result") == "CORRECT":
                    return "correct"
            except (ValueError, AttributeError):
                pass
    return "solver"


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for art in sorted(glob.glob(os.path.join(PY, "day-*-binary-reverse-engineering", "*", "artifacts"))):
        day = re.search(r"day-(\d\d)", art).group(1)
        chal = None
        for d, _, files in sorted(os.walk(art)):
            starts = [f for f in files if f in ("start.json", "start_single.json")]
            answers = [f for f in files if f in ("answers.json", "answers_single.json")]
            if not starts or not answers:
                continue
            try:
                st = json.load(open(os.path.join(d, starts[0])))
                ans = json.load(open(os.path.join(d, answers[0])))
            except ValueError:
                continue  # a crashed run left an empty/partial file
            ans = ans.get("answers", ans)
            if not st.get("questions") or not ans:
                continue
            zips = sorted((f for f in files if re.fullmatch(r"delivery\d+\.zip", f)), key=lambda f: int(re.search(r"\d+", f).group()))
            if not zips and "evidence.zip" in files:
                zips = ["evidence.zip"]
            if not zips:
                # day-13 single: evidence lives in the time-trial-independent single dir? skip if absent
                continue
            mode = st.get("mode") or ("mastery" if len(zips) > 1 else "single_solve")
            chal = chal or json.dumps(st.get("name"))
            sub = os.path.relpath(d, art).replace("/", "_").replace(".", "top")
            json.dump({
                "challenge": f"binary-reverse-engineering-day-{day}" + {"time_trial": "/time_trial", "mastery": "/mastery"}.get(mode, ""),
                "start": rel(os.path.join(d, starts[0])),
                "deliveries": [{"path": rel(os.path.join(d, z)), "name": "evidence.zip"} for z in zips],
                "expected": ans,
                "graded": graded(d),
            }, open(os.path.join(OUT, f"d{day}-{sub}.json"), "w"), indent=1)
            count += 1
    print(f"wrote {count} manifests to {OUT}")


if __name__ == "__main__":
    main()
