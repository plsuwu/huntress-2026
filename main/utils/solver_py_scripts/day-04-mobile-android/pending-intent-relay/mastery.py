#!/usr/bin/env python3
"""Pending Intent Relay - mastery (also works for single/time_trial).

start -> download -> find every APK's assets/huntress_puzzle.json (artifact may be a bare APK, or a zip
holding APKs) -> map each question to its APK by the question's `artifact` name -> decode -> one batched submit.
Raw start response + download are dumped to $TMPDIR/pir_mastery_* first so a failure can be inspected.

    HUNTRESS_GM_TOKEN=... python3 -I mastery.py [/mastery]   # /time_trial also works
"""
import io
from concurrent.futures import ThreadPoolExecutor
import json
import os
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from solve import CHALLENGE, ANS_RE, decode, gm  # noqa: E402

PUZZLE = "assets/huntress_puzzle.json"


def find_puzzles(data, name, out):
    """Recursively walk zip bytes; out[basename] = puzzle dict for every archive that carries a puzzle asset."""
    try:
        z = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile:
        return out
    names = z.namelist()
    if PUZZLE in names:
        out[name.rsplit("/", 1)[-1]] = json.loads(z.read(PUZZLE))
    for n in names:
        if not n.endswith("/") and n != PUZZLE:
            b = z.read(n)
            if b[:2] == b"PK":
                find_puzzles(b, n, out)
    return out


def artifact_name(q):
    a = q.get("artifact") or ""
    return a.partition("->")[0].split(": ")[-1].strip().rsplit("/", 1)[-1]


def main():
    chal = CHALLENGE + (sys.argv[1] if len(sys.argv) > 1 else "/mastery")
    d = os.environ.get("TMPDIR", "/tmp")
    start = json.loads(gm({"op": "start", "challenge": chal}))
    json.dump(start, open(os.path.join(d, "pir_mastery_start.json"), "w"), indent=1)
    print("deadline", start.get("deadline"), "attempts_left", start.get("unique_attempts_remaining"), file=sys.stderr)
    # single/time_trial: one `download`; mastery: `artifacts` = [{download, filename, sha256}, ...] (bare APKs)
    arts = start.get("artifacts") or [{"download": start["download"], "filename": start.get("artifact")}]
    with ThreadPoolExecutor(len(arts)) as ex:
        blobs = list(ex.map(lambda a: gm(query=a["download"].lstrip("/")), arts))
    puzzles = {}
    for a, blob in zip(arts, blobs):
        open(os.path.join(d, "pir_mastery_" + (a.get("filename") or "artifact")), "wb").write(blob)
        find_puzzles(blob, a.get("filename") or "download", puzzles)
    qs = start["questions"]
    answers = {}
    for i, q in enumerate(qs):
        p = puzzles.get(artifact_name(q))
        if p is None and len(puzzles) == len(qs):  # positional fallback
            p = list(puzzles.values())[i]
        try:
            a = decode(p).decode()
        except Exception as e:  # leave unsolved, report
            print("unsolved", q["id"], q.get("artifact"), repr(e), file=sys.stderr)
            continue
        if ANS_RE.fullmatch(a.encode()):
            answers[q["id"]] = a
        else:
            print("bad format", q["id"], a, file=sys.stderr)
    print(f"{len(answers)}/{len(qs)} answered", file=sys.stderr)
    if not answers:
        return
    res = json.loads(gm({"op": "submit", "challenge": chal, "answers": answers}))
    res.pop("guidance", None)
    res.pop("questions", None)
    print(json.dumps(res))


if __name__ == "__main__":
    main()
