#!/usr/bin/env python3
"""Writes replay manifests for the metadata lane: ctf-run/fixtures/metadata/<case>.json.

Pairs every saved `<mode>-start.json` (also `start_single.json`) with the archive downloaded for it
(`<mode>-latest.zip`, `single*.zip`, `single/ev.zip`) and records the day's own Python `answer()` as
the expected value. Questions the Python reference could not answer are listed under "python_unanswered".

usage: python3 -I tools/fixtures_metadata.py
"""
import importlib.util
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.dirname(CRATE)
PY = os.path.join(ROOT, "utils", "solver_py_scripts")
OUT = os.path.join(CRATE, "fixtures", "metadata")
sys.path.insert(0, PY)
from nested_zip import load_registry  # noqa: E402


def load(path, n=[0]):
    n[0] += 1
    sys.path.insert(0, os.path.dirname(path))
    spec = importlib.util.spec_from_file_location(f"metadata_solver_{n[0]}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    sys.path.pop(0)
    return mod


def rel(p):
    return os.path.relpath(p, OUT)


PAIRS = [
    ("single_solve-start.json", ["single_solve-latest.zip", "single_solve.zip", "single.zip", "single/ev.zip"]),
    ("start_single.json", ["single.zip", "single_solve-latest.zip", "single/ev.zip"]),
    ("time_trial-start.json", ["time_trial-latest.zip"]),
    ("mastery-start.json", ["mastery-latest.zip"]),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for day_dir in sorted(d for d in os.listdir(PY) if re.fullmatch(r"day-\d\d-metadata-forensics", d)):
        day = day_dir[4:6]
        for chal in sorted(os.listdir(os.path.join(PY, day_dir))):
            base = os.path.join(PY, day_dir, chal)
            art = os.path.join(base, "artifacts")
            if not os.path.isdir(art):
                continue
            mod = load(os.path.join(base, "solve.py"))
            for start_name, zips in PAIRS:
                sp = os.path.join(art, start_name)
                zp = next((os.path.join(art, z) for z in zips if os.path.exists(os.path.join(art, z))), None)
                if not os.path.exists(sp) or zp is None:
                    continue
                st = json.load(open(sp))
                if not st.get("questions"):
                    continue
                data = open(zp, "rb").read()
                reg = load_registry(data)
                exp, missing = {}, []
                for q in st["questions"]:
                    try:
                        a = mod.answer(q, reg)
                    except Exception as e:  # noqa: BLE001
                        a = None
                        print("python error", sp, q["id"], repr(e), file=sys.stderr)
                    if a is None:
                        missing.append(q["id"])
                    else:
                        exp[q["id"]] = a
                challenge = getattr(mod, "CHAL")
                mode = start_name.split("-")[0].replace("start_single.json", "single_solve")
                suffix = {"single_solve": "", "time_trial": "/time_trial", "mastery": "/mastery"}.get(mode, "")
                name = f"d{day}-{mode}" + ("-b" if start_name == "start_single.json" else "")
                json.dump({
                    "challenge": challenge + suffix,
                    "start": rel(sp),
                    "deliveries": [{"path": rel(zp), "name": os.path.basename(zp)}],
                    "expected": exp,
                    "graded": "solver",
                    "python_unanswered": missing,
                }, open(os.path.join(OUT, name + ".json"), "w"), indent=1)
                count += 1
    print(f"wrote {count} manifests to {OUT}")


if __name__ == "__main__":
    main()
