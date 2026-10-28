#!/usr/bin/env python3
"""Per-target classifier for web-exploitation-day-21-double-spent mastery.

Mastery is cumulative (see docs/web-worker-lessons.md): classify every
assigned target independently rather than assuming this family's own bug
(credit double-spend race) applies to all of them. Ports the earlier-class
recoverers (`rec_backup`, `rec_idor`, `rec_sqli_group_close`) from
`recovery_lib.py` plus this family's own `rec_double_spend` from
`solve.py`, each scoped to a single target origin, no GM calls.

Usage:
    python3 mastery.py <evidence_root> <base_url_1> [<base_url_2> ...]

For each base URL: fetch /, fetch /api/clues, and dispatch in evidence-led
order (clues-driven classes first, then IDOR/SQLi HTML signs, then backup
exposure last) the same way recovery_lib.classify_and_recover does, extended
with this family's clue shape: feature_route + secondary_route with NO
inventory_route/protected_reference/event_type/owner_scope/state_format_route
(distinguishing it from every other feature_route-driven class already in
recovery_lib - notably the SSRF guard there also matches
feature_route+secondary_route alone, so this day's own class must be tried
first and SSRF only as a fallback if double-spend finds no proof).
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from solve import Target, rec_double_spend, PROOF_RE  # noqa: E402

RECOVERY_LIB = Path(__file__).resolve().parents[2] / "recovery_lib.py"
sys.path.insert(0, str(RECOVERY_LIB.parent))
import recovery_lib  # noqa: E402


def classify_target(base, evidence_dir):
    t = Target(base, evidence_dir)
    evidence = {}
    status, html = t.get("/", save="root")
    evidence["root_status"] = status
    if status != 200:
        evidence["classification"] = "root request failed"
        return None, evidence

    clues_status, clues = t.get_json("/api/clues", save="clues")
    if clues_status == 200 and clues:
        evidence["clues"] = clues
        # This family's own class: feature_route + secondary_route, no other
        # clue keys at all (distinguishes it from SSRF, which shares the
        # same two keys - try double-spend first on this exact shape).
        if clues.get("feature_route") and clues.get("secondary_route") \
                and "inventory_route" not in clues and "protected_reference" not in clues \
                and "event_type" not in clues and "state_format_route" not in clues \
                and "session_hint" not in clues and "callback_nonce" not in clues:
            evidence["classification"] = "credit double-spend (race)"
            proof, results, verdict = rec_double_spend(t, clues)
            evidence["race_results"] = [r[1:] for r in results]
            if proof:
                return proof, evidence
            # Fall through to SSRF in case this target is actually that class
            evidence["classification"] = "feature_route+secondary_route, not double-spend - trying SSRF"
            proof = recovery_lib.rec_ssrf(t, clues)
            if proof:
                evidence["classification"] = "SSRF (fetch proxy)"
                return proof, evidence

    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        evidence["classification"] = "IDOR"
        proof = recovery_lib.rec_idor(t, html)
        if proof:
            return proof, evidence

    if re.search(r'<form[^>]*action=[\'"][^\'"]*(search|lookup|explore)', html, re.I):
        evidence["classification"] = "SQL injection (group-closing boolean)"
        proof = recovery_lib.rec_sqli_group_close(t, html)
        if proof:
            return proof, evidence

    if re.search(r"crawler polic|robots", html, re.I):
        evidence["classification"] = "backup exposure"
        proof = recovery_lib.rec_backup(t)
        if proof:
            return proof, evidence

    # last resorts
    if "<form" in html:
        proof = recovery_lib.rec_sqli_group_close(t, html)
        if proof:
            evidence["classification"] = "SQL injection (fallback)"
            return proof, evidence
    evidence.setdefault("classification", "unclassified; tried backup exposure as a last resort")
    return recovery_lib.rec_backup(t), evidence


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(2)
    evidence_root = Path(sys.argv[1])
    bases = sys.argv[2:]
    results = {}
    for i, base in enumerate(bases, start=1):
        edir = evidence_root / f"target_{i}"
        proof, evidence = classify_target(base, edir)
        results[f"Target {i}"] = {"base": base, "proof": proof,
                                   "classification": evidence.get("classification")}
        print(f"Target {i} ({base}): {evidence.get('classification')} -> {proof}")
    (evidence_root / "classification.json").write_text(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
