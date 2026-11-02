#!/usr/bin/env python3
"""Shared all-mode web solver, automatically copied into each worker directory.

After `python3 tools/wk.py begin <mode>`, edit only `rec_this_family()` below, then:

    python3 solve.py single_solve
    python3 tools/wk.py submit single_solve

Reuse the same file for time_trial and mastery. It supplies the scoped HTTP client,
canonical recoverers, question/target mapping, evidence capture, strict answer IDs,
and attempt-bound partial-answer reuse. Do not write another HTTP client or a
separate mastery classifier.

`rec_this_family` must check the current target's clue shape before making requests
and return None on a mismatch. This matters in mastery, where every target can be a
different bug class. Canonical recovery runs after the local hook returns None.

Answers recovered by the driver's first pass are reused only when answers.meta.json
matches the current attempt_id and target URLs. A changed attempt or target therefore
forces fresh derivation instead of accidentally reusing stale proofs.
"""
import json
import sys
import urllib.parse
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
RECOVERY_DIR = SCRIPT_DIR / "solver_py_scripts"
if not RECOVERY_DIR.is_dir():
    RECOVERY_DIR = SCRIPT_DIR.parent  # running from solver_py_scripts/web_templates
sys.path.insert(0, str(RECOVERY_DIR))
from recovery_lib import PROOF_RE, Target, classify_and_recover  # noqa: E402



def rec_this_family(t, clues):
    """Reproduce an unsigned fetch-to-worker incident, then remediate it.

    The request log supplies the unsigned fetch target. Creating that event,
    posting its token to the audit route, and issuing the disclosed worker
    command releases the incident evidence. The evidence then supplies both the
    malicious event to detect and the signed status command to retain."""
    if clues.get("framework") == "Django" and clues.get("feature_route") \
            and not clues.get("secondary_route"):
        # Day 05's canonical-path authorization gap is not yet in recovery_lib.
        route = "/" + clues["feature_route"].strip("/")
        status, body = t.get(route, save="django_unslashed_feature")
        match = PROOF_RE.search(body)
        return match.group(0) if match else None
    required = ("feature_route", "secondary_route", "final_route",
                "request_log_route", "evidence_route", "response_route",
                "fetch_query_field", "worker_query_field", "event_json_field")
    if not all(clues.get(key) for key in required):
        return None
    status, log = t.get_json(clues["request_log_route"], save="incident_request_log")
    if status != 200 or not isinstance(log, dict):
        return None
    incident = next((record for record in log.get("records", [])
                     if record.get("kind") == "fetch"
                     and record.get("signed") is False
                     and record.get("outcome") == "queued"
                     and record.get("target")), None)
    if not incident:
        return None
    token = incident["target"]
    query = urllib.parse.urlencode
    t.get(f"{clues['feature_route']}?{query({clues['fetch_query_field']: token})}",
          save="unsigned_fetch")
    status, dispatched = t.post_json(
        clues["secondary_route"], {clues["event_json_field"]: token},
        save="audit_dispatch")
    if not isinstance(dispatched, dict):
        return None
    resources = dispatched.get("resources", [])
    command = next((candidate.replace("<resource>", resources[0])
                    for candidate in dispatched.get("available_commands", [])
                    if "<resource>" in candidate and resources), None)
    if not command:
        return None
    worker_route = dispatched.get("worker_route", clues["final_route"])
    t.get(f"{worker_route}?{query({clues['worker_query_field']: command})}",
          save="unsigned_worker_dispatch")
    status, evidence = t.get_json(clues["evidence_route"], save="incident_evidence")
    if status != 200 or not isinstance(evidence, dict):
        return None
    events = evidence.get("events", [])
    attack = next((event for event in events
                   if event.get("command") == command
                   and event.get("signed") is False), None)
    benign = next((event for event in events
                   if event.get("kind") == (attack or {}).get("kind")
                   and event.get("signed") is True
                   and event.get("command")), None)
    if not attack or not benign:
        return None
    response = {
        "detector": {"kind": attack["kind"], "signed": False},
        "repair": {
            "block_internal_fetch": True,
            "require_signed_event": True,
            "worker_allowlist": [benign["command"]],
        },
    }
    status, result = t.post_json(clues["response_route"], response, save="remediation")
    match = PROOF_RE.search(json.dumps(result))
    return match.group(0) if match else None


def solve_target(t, prompt=""):
    """Try the local family hook, then canonical recovery, reusing initial recon."""
    evidence = {}
    root_status, html = t.get("/", save="root")
    clues_status, clues_body = t.get("/api/clues", save="clues")
    try:
        clues = json.loads(clues_body) if clues_status == 200 else {}
    except (TypeError, ValueError):
        clues = {}
    evidence["root_status"] = root_status
    if clues:
        evidence["clues"] = clues
    proof = rec_this_family(t, clues)
    if proof:
        evidence["classification"] = "this family's own class"
        return proof, evidence
    initial = {"/": (root_status, html), "/api/clues": (clues_status, clues_body)}
    proof = classify_and_recover(t, evidence, prompt, initial=initial)
    return proof, evidence


def current_targets(start):
    return {t["id"]: t["url"] for t in start.get("targets", [])
            if isinstance(t, dict) and t.get("id") and t.get("url")}


def load_reusable_answers(mode_dir, start):
    """Load only first-pass answers bound to this exact attempt and target assignment."""
    try:
        meta = json.loads((mode_dir / "answers.meta.json").read_text())
        raw = json.loads((mode_dir / "answers.json").read_text())
    except (OSError, ValueError):
        return {}
    if meta.get("attempt_id") != start.get("attempt_id"):
        return {}
    if meta.get("targets") != current_targets(start):
        return {}
    answers = raw.get("answers", raw) if isinstance(raw, dict) else {}
    allowed = set(start.get("answer_ids") or [])
    return {qid: value for qid, value in answers.items()
            if qid in allowed and isinstance(value, str)}


def save_answers(mode_dir, start, answers):
    (mode_dir / "answers.json").write_text(
        json.dumps({"answers": answers}, indent=2) + "\n")
    (mode_dir / "answers.meta.json").write_text(json.dumps({
        "attempt_id": start.get("attempt_id"),
        "targets": current_targets(start),
    }, indent=2) + "\n")


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <mode>  (single_solve | time_trial | mastery)",
              file=sys.stderr)
        return 2
    mode = sys.argv[1]
    mode_dir = SCRIPT_DIR / mode
    start = json.loads((mode_dir / "start.json").read_text())
    targets_by_id = current_targets(start)
    prompt = start.get("prompt", "")

    answers = load_reusable_answers(mode_dir, start)
    if answers:
        print(f"reusing {len(answers)} answer(s) from the current attempt", file=sys.stderr)
    all_evidence = {}
    for question in start.get("questions", []):
        qid, target_id = question["id"], question.get("artifact")
        if qid in answers:
            continue
        url = targets_by_id.get(target_id)
        if not url:
            print(f"{qid}: no target url for artifact {target_id!r}", file=sys.stderr)
            continue
        evidence_dir = mode_dir / "evidence" / str(target_id).replace(" ", "_")
        t = Target(url, evidence_dir)
        proof, evidence = solve_target(t, prompt)
        all_evidence[qid] = {"target": target_id, "url": url, **evidence}
        print(f"{qid} [{target_id}] class={evidence.get('classification')} -> {proof}",
              file=sys.stderr)
        if proof:
            answers[qid] = proof

    missing = [q["id"] for q in start.get("questions", []) if q["id"] not in answers]
    mode_dir.mkdir(parents=True, exist_ok=True)
    save_answers(mode_dir, start, answers)
    (mode_dir / "solver-evidence.json").write_text(json.dumps(all_evidence, indent=2) + "\n")
    if missing:
        print(f"missing proofs for: {missing} - fill in rec_this_family() and re-run",
              file=sys.stderr)
        return 1
    print(f"wrote {mode_dir / 'answers.json'} - now run: python3 tools/wk.py submit {mode}",
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
