#!/usr/bin/env python3
"""Reusable reference solver for the day-21 web-exploitation families (curriculum
06-reflect-upon-this, 07-stored-xss, 08-session-impression). Single_solve, time_trial
and mastery all use this same dispatcher: mastery just has more than one target to
classify independently (see docs/web-worker-lessons.md).

Recovery logic lives in ../recovery_lib.py (shared with day-22). This file is only
the thin per-attempt CLI: read a start.json, join questions[].artifact to
targets[].id, recover each proof from the CURRENT live target, print the answers.

Usage (reference only - port what you need into your own job's solver; do not run
this against a live attempt without reading it first):
    python3 -I solve.py start.json

Stdlib only. Never talks to the Game Master; only to the target origins start.json
names.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from recovery_lib import Target, classify_and_recover  # noqa: E402


def main():
    start = json.loads(Path(sys.argv[1]).read_text())
    targets = {t["id"]: t["url"] for t in start.get("targets", [])}
    evidence_root = Path(sys.argv[2]) if len(sys.argv) > 2 else None

    answers, all_evidence = {}, {}
    for question in start["questions"]:
        qid, target_id = question["id"], question.get("artifact")
        url = targets.get(target_id)
        if not url:
            raise SystemExit(f"no target url for {qid} ({target_id!r})")
        evidence = {}
        t = Target(url, evidence_root / target_id.replace(" ", "_") if evidence_root else None)
        proof = classify_and_recover(t, evidence)
        all_evidence[qid] = {"target": target_id, "url": url, **evidence}
        print(f"{qid} [{target_id}] class={evidence.get('classification')} -> {proof}", file=sys.stderr)
        if proof:
            answers[qid] = proof

    missing = [q["id"] for q in start["questions"] if q["id"] not in answers]
    print(json.dumps({"evidence": all_evidence}, indent=2), file=sys.stderr)
    if missing:
        raise SystemExit(f"missing proofs for: {missing}")
    print(json.dumps({"answers": answers}, indent=2))


if __name__ == "__main__":
    main()
