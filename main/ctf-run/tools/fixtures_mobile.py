#!/usr/bin/env python3
"""Writes replay manifests for the mobile lane: ctf-run/fixtures/mobile/<case>.json.

  - every saved APK under utils/solver_py_scripts/day-NN-mobile-android/*/artifacts -> one-question case
  - every saved start JSON with an `artifacts` list -> the real questions, deliveries matched by file name
  - ctf-run/fixtures/raw/phase0/out-mobile -> real regular retry, graded CORRECT
Expected answers come from that day's Python `solve_apk` (graded answers where the run saved them).

usage: python3 -I tools/fixtures_mobile.py
"""
import importlib.util
import json
import os
import re
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.dirname(CRATE)
PY = os.path.join(ROOT, "utils", "solver_py_scripts")
OUT = os.path.join(CRATE, "fixtures", "mobile")


def load(path, n=[0]):
    n[0] += 1
    spec = importlib.util.spec_from_file_location(f"mobile_solver_{n[0]}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def rel(p):
    return os.path.relpath(p, OUT)


def is_apk(p):
    try:
        return "assets/huntress_puzzle.json" in zipfile.ZipFile(p).namelist()
    except Exception:
        return False


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for day_dir in sorted(d for d in os.listdir(PY) if re.fullmatch(r"day-\d\d-mobile-android", d)):
        day = day_dir[4:6]
        for chal in sorted(os.listdir(os.path.join(PY, day_dir))):
            base = os.path.join(PY, day_dir, chal)
            art = os.path.join(base, "artifacts")
            if not os.path.isdir(art):
                continue
            mod = load(os.path.join(base, "solve.py"))
            challenge = mod.CHALLENGE
            for dirpath, _, files in sorted(os.walk(art)):
                if "/files" in dirpath:
                    continue
                apks = sorted(f for f in files if is_apk(os.path.join(dirpath, f)))
                sub = os.path.relpath(dirpath, art).replace("/", "_").replace(".", "top")
                for f in apks:
                    p = os.path.join(dirpath, f)
                    json.dump({
                        "challenge": challenge,
                        "start": {"questions": [{"id": "answer", "artifact": f}], "answer_ids": ["answer"]},
                        "deliveries": [{"path": rel(p), "name": f}],
                        "expected": {"answer": mod.solve_apk(p)},
                        "graded": "solver",
                    }, open(os.path.join(OUT, f"d{day}-{sub}-{os.path.splitext(f)[0]}.json"), "w"), indent=1)
                    count += 1
                for f in sorted(files):
                    if not f.endswith(".json") or "start" not in f:
                        continue
                    st = json.load(open(os.path.join(dirpath, f)))
                    arts = st.get("artifacts") or []
                    if len(arts) < 2 or not st.get("questions"):
                        continue
                    deliveries, exp = [], {}
                    for a in arts:
                        match = [x for x in apks if x.endswith(a["filename"])]
                        if len(match) != 1:
                            break
                        deliveries.append({"path": rel(os.path.join(dirpath, match[0])), "name": a["filename"]})
                    else:
                        for q in st["questions"]:
                            n = int(re.match(r"delivery (\d+):", q["artifact"]).group(1))
                            exp[q["id"]] = mod.solve_apk(os.path.join(OUT, deliveries[n - 1]["path"]))
                        mode = "mastery" if "mastery" in (st.get("mode") or f) else "time_trial"
                        json.dump({
                            "challenge": f"{challenge}/{mode}",
                            "start": rel(os.path.join(dirpath, f)),
                            "deliveries": deliveries,
                            "expected": exp,
                            "graded": "solver",
                        }, open(os.path.join(OUT, f"d{day}-{sub}-{os.path.splitext(f)[0]}.json"), "w"), indent=1)
                        count += 1
    p0 = os.path.join(CRATE, "fixtures", "raw", "phase0", "out-mobile")
    if os.path.isdir(p0):
        st = json.load(open(os.path.join(p0, "start.json")))
        saved = json.load(open(os.path.join(p0, "answers.json")))
        json.dump({
            "challenge": "mobile-android-day-13-keystore-misuse/mastery",
            "start": rel(os.path.join(p0, "start.json")),
            "deliveries": [{"path": rel(os.path.join(p0, f"delivery{i + 1}.apk")), "name": a["filename"]} for i, a in enumerate(st["artifacts"])],
            "expected": saved["answers"],
            "graded": "correct" if saved["final"].get("result") == "CORRECT" else "solver",
        }, open(os.path.join(OUT, "d13-phase0-regular-retry.json"), "w"), indent=1)
        count += 1
    print(f"wrote {count} manifests to {OUT}")


if __name__ == "__main__":
    main()
