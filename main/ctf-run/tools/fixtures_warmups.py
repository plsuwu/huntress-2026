#!/usr/bin/env python3
"""Writes replay manifests for the warmups lane: ctf-run/fixtures/warmups/<case>.json.

Sources (nothing is copied; manifests point at the original files):
  - utils/solver_py_scripts/day-NN-warmups/*/artifacts/
      * single blobs (blob.txt, single_solve-blob.txt, blob_single.bin) -> expected from that day's Python solver
      * run dirs (mastery-*, time_trial-*, *_example_variant) with NN-<artifact> files + answers.json; graded answers
        when the saved result was CORRECT, else the Python solver's output
      * mastery_example_variant.zip (a bundle) -> expected from the Python solver
  - ctf-run/fixtures/raw/phase0/out-warmups (a real timed start + bundle, graded CORRECT)

usage: python3 -I tools/fixtures_warmups.py
"""
import importlib.util
import io
import json
import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.dirname(CRATE)
PY = os.path.join(ROOT, "utils", "solver_py_scripts")
OUT = os.path.join(CRATE, "fixtures", "warmups")


def load_solver(path, n=[0]):
    n[0] += 1
    spec = importlib.util.spec_from_file_location(f"warmups_solver_{n[0]}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.solve


def rel(p):
    return os.path.relpath(p, OUT)


def write(name, manifest):
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump(manifest, f, indent=1)


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for day_dir in sorted(d for d in os.listdir(PY) if re.fullmatch(r"day-\d\d-warmups", d)):
        day = day_dir[4:6]
        for chal in sorted(os.listdir(os.path.join(PY, day_dir))):
            base = os.path.join(PY, day_dir, chal)
            art = os.path.join(base, "artifacts")
            if not os.path.isdir(art):
                continue
            solve = load_solver(os.path.join(base, "solve.py"))
            challenge = f"warmups-day-{day}"
            for entry in sorted(os.listdir(art)):
                p = os.path.join(art, entry)
                if entry in ("blob.txt", "single_solve-blob.txt", "blob_single.bin"):
                    text = open(p, encoding="utf-8", errors="replace").read()
                    write(f"d{day}-single", {
                        "challenge": challenge,
                        "start": {"questions": [{"id": "message", "artifact": "blob.txt"}], "answer_ids": ["message"]},
                        "deliveries": [{"path": rel(p), "name": "blob.txt"}],
                        "expected": {"message": solve(text)},
                        "graded": "solver",
                    })
                    count += 1
                elif entry.endswith(".zip"):
                    z = zipfile.ZipFile(p)
                    names = sorted(n for n in z.namelist() if not n.endswith("/"))
                    qs = [{"id": f"set{i + 1}.message", "artifact": n} for i, n in enumerate(names)]
                    exp = {q["id"]: solve(z.read(q["artifact"]).decode()) for q in qs}
                    write(f"d{day}-mastery-bundle", {
                        "challenge": challenge + "/mastery",
                        "start": {"questions": qs, "answer_ids": [q["id"] for q in qs]},
                        "deliveries": [{"path": rel(p), "name": entry}],
                        "expected": exp,
                        "graded": "solver",
                    })
                    count += 1
                elif os.path.isdir(p) and os.path.exists(os.path.join(p, "answers.json")):
                    saved = json.load(open(os.path.join(p, "answers.json")))
                    ids = list(saved["answers"])
                    files = sorted(f for f in os.listdir(p) if re.match(r"\d\d-", f))
                    if len(files) != len(ids):
                        print("skip (file/answer count mismatch):", p, file=sys.stderr)
                        continue
                    arts = [f[3:] for f in files]
                    mode = "time_trial" if entry.startswith("time_trial") else "mastery"
                    correct = (saved.get("result") or {}).get("result") == "CORRECT"
                    exp = saved["answers"] if correct else {
                        i: solve(open(os.path.join(p, f), encoding="utf-8", errors="replace").read()) for i, f in zip(ids, files)}
                    write(f"d{day}-{entry}", {
                        "challenge": f"{challenge}/{mode}",
                        "start": {"questions": [{"id": i, "artifact": a} for i, a in zip(ids, arts)], "answer_ids": ids},
                        "deliveries": [{"path": rel(os.path.join(p, f)), "name": a} for f, a in zip(files, arts)],
                        "expected": exp,
                        "graded": "correct" if correct else "solver",
                    })
                    count += 1
    # The Phase 0 timed attempt: real start response + bundle.
    p0 = os.path.join(CRATE, "fixtures", "raw", "phase0", "out-warmups")
    if os.path.isdir(p0):
        saved = json.load(open(os.path.join(p0, "answers.json")))
        write("d13-phase0-timed-bundle", {
            "challenge": "warmups-day-13/mastery",
            "start": rel(os.path.join(p0, "start.json")),
            "deliveries": [{"path": rel(os.path.join(p0, "bundle1.bin")), "name": "warmups-day-13-mastery.zip"}],
            "expected": saved["answers"],
            "graded": "correct" if saved["final"].get("result") == "CORRECT" else "solver",
        })
        count += 1
    print(f"wrote {count} manifests to {OUT}")


if __name__ == "__main__":
    main()
