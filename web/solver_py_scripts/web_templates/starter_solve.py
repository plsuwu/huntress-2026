#!/usr/bin/env python3
"""Drop-in starter for a web worker's own solve.py - a TEMPLATE. Copy it in and
edit ONE function; do not import this file directly as your job's solver.

Usage (from your worker directory, workers/web/<base-id>/, after
`python3 tools/wk.py begin <mode>` has written `<mode>/start.json`):

    cp solver_py_scripts/web_templates/starter_solve.py solve.py
    # edit rec_this_family() below with this family's own bug - that is the
    # only thing you should need to write.
    python3 solve.py <mode>              # single_solve | time_trial | mastery
                                          # writes <mode>/answers.json
    python3 tools/wk.py submit <mode>

What this already does for you, for free, before you write anything:
  - The HTTP client (`Target`, from recovery_lib.py): cookie jar, WAF-safe
    User-Agent, pacing, evidence-saving, origin-scope guard. Never re-derive
    this - every hand-rolled copy of it across 8 prior solve.py files turned
    out identical.
  - `classify_and_recover`: recognizes every class graded so far - backup
    exposure, IDOR and SQL injection (group-closing) have each shown up
    independently in every mastery set across 3+ release days running, plus
    session fixation, reflected-XSS callback, stored-XSS bot, CSRF, upload
    bypass, SSRF, field/action authorization bypass, OS command injection,
    reset-token/account-id confusion, GraphQL field-level bypass, and LFI.
    Run against a mastery set, this alone often solves 3 of 4 targets before
    you write a single line - `rec_this_family` below only needs to cover the
    one target that doesn't match anything on that list.
  - Evidence: every request this makes is saved under <mode>/evidence/<target>/.

Read `docs/web-worker-lessons.md` before trusting any of this against your
actual target - mastery is cumulative and never assumed from the challenge's
own name, a form alone never proves SQLi, an unknown page alone never proves
backup exposure. `recovery_lib.py` makes no GM calls and holds no credential,
but per standing policy its output is reference, not an oracle: read the
`rec_*` function for whatever class it reports before trusting the proof it
returns. If your family turns out to be GraphQL-shaped, or needs to wait on
an out-of-band callback/bot, read the sibling templates in this directory
(`graphql_helpers.py`, `poll_patterns.py`) before writing a second HTTP client
or a second polling loop - don't reimplement what's already there.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "solver_py_scripts"))
from recovery_lib import Target, classify_and_recover  # noqa: E402


def rec_this_family(t, clues):
    """TODO: this family's own bug class.

    `clues` is the parsed `/api/clues` JSON for this target (an empty dict if
    that endpoint 404'd - some bug classes, like backup exposure/IDOR/SQLi/LFI,
    are evidence-led from the root page instead; fetch `t.get("/")` yourself
    here if that's your shape, the same way `recovery_lib.rec_backup`/`rec_idor`
    do).

    Return the `proof_<hex>` string on success, or `None` (never raise) if this
    target doesn't match - `solve_target` below falls back to the generic
    classifier either way, so returning None here is always safe, even for a
    target that turns out to be one of the known recurring classes instead."""
    return None


def solve_target(t, prompt=""):
    """Try this family's own class first (most likely fit for at least one
    target), then fall back to the generic classifier. Returns
    (proof_or_None, evidence_dict) - evidence_dict is also written into the
    brief-style summary main() prints, so a worker can see at a glance which
    path found (or missed) each target's proof."""
    evidence = {}
    _, clues = t.get_json("/api/clues", save="clues")
    proof = rec_this_family(t, clues or {})
    if proof:
        evidence["classification"] = "this family's own class"
        return proof, evidence
    proof = classify_and_recover(t, evidence, prompt)
    return proof, evidence


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <mode>  (single_solve | time_trial | mastery)",
              file=sys.stderr)
        return 2
    mode = sys.argv[1]
    mode_dir = Path(mode)
    start = json.loads((mode_dir / "start.json").read_text())
    targets_by_id = {t["id"]: t["url"] for t in start.get("targets", []) if isinstance(t, dict)}
    prompt = start.get("prompt", "")

    answers, all_evidence = {}, {}
    for question in start.get("questions", []):
        qid, target_id = question["id"], question.get("artifact")
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
    (mode_dir / "answers.json").write_text(json.dumps({"answers": answers}, indent=2) + "\n")
    if missing:
        print(f"missing proofs for: {missing} - fill in rec_this_family() and re-run",
              file=sys.stderr)
        return 1
    print(f"wrote {mode_dir / 'answers.json'} - now run: python3 tools/wk.py submit {mode}",
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
