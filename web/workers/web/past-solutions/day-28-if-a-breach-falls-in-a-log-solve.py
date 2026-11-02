#!/usr/bin/env python3
"""Solve for day-25 'If a Breach Falls in a Log' (Slim framework).

Bug class: incomplete audit logging on a privileged action. POSTing to the
feature_route performs a protected export (returns an export_id) but the audit
log only records a benign 'page_view' event - the decisive 'protected_export'
event is missing. The secondary_route lets an analyst report a missing event:
POST {export_id, missing_event, observed_event} with the operation name from
the export response and the event actually observed in the log, and it returns
the proof.

Generic: routes and field names are derived from the live target (/api/clues
and the status route's own 'report' hint), never hard-coded per attempt.
"""
import json
import re
import sys
import time
import urllib.parse
import urllib.request
import urllib.error

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.8


def request(base, path, method="GET", payload=None):
    time.sleep(DELAY)
    body = json.dumps(payload).encode() if payload is not None else None
    headers = {"User-Agent": UA}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(base + path, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:
        return err.code, err.read().decode("utf-8", "replace")


def get_json(base, path):
    status, body = request(base, path)
    return status, (json.loads(body) if body else None)


def post_json(base, path, payload):
    status, body = request(base, path, method="POST", payload=payload)
    return status, (json.loads(body) if body else None)


def solve(base):
    base = base.rstrip("/")
    # 1. Workspace notes disclose the feature and secondary routes.
    status, clues = get_json(base, "/api/clues")
    if status != 200 or not clues:
        return None
    feature = clues.get("feature_route")
    secondary = clues.get("secondary_route")
    if not feature or not secondary:
        return None

    # 2. The status route's own 'report' block discloses the expected fields
    #    for the missing-event report (export_id / missing_event / observed_event).
    status, before = get_json(base, secondary)
    report_fields = (before or {}).get("report", {}).get("fields",
                     ["export_id", "missing_event", "observed_event"])
    id_field, missing_field, observed_field = (report_fields + ["export_id", "missing_event", "observed_event"])[:3]

    # 3. Run the protected export - this is the decisive action the log misses.
    status, export = post_json(base, feature, {})
    if status != 200 or not export:
        return None
    export_id = export.get(id_field) or export.get("export_id")
    operation = export.get("operation", "protected_export")
    if not export_id:
        return None

    # 4. Read the audit log: it records only a benign event for this export_id.
    status, after = get_json(base, secondary)
    observed = None
    for event in (after or {}).get("events", []):
        if event.get(id_field) == export_id or event.get("export_id") == export_id:
            observed = event.get("event")
    if not observed:
        events = (after or {}).get("events", [])
        observed = events[-1].get("event") if events else "page_view"

    # 5. Report the decisive event the log missed; the proof comes back.
    report = {id_field: export_id, missing_field: operation, observed_field: observed}
    status, result = post_json(base, secondary, report)
    text = json.dumps(result) if isinstance(result, (dict, list)) else str(result)
    match = PROOF_RE.search(text)
    if match:
        return match.group(0)

    # Fallback: if the naive guess did not match, the server may name the
    # missing event differently; answer with what we saw anyway.
    return None


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <target-base-url>")
    proof = solve(sys.argv[1])
    if not proof:
        sys.exit("no proof recovered")
    print(proof)


if __name__ == "__main__":
    main()
