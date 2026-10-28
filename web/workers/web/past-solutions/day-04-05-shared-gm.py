#!/usr/bin/env python3
"""Small Python-only Game Master client for the day 20 web lane."""
import argparse
import json
import os
from pathlib import Path
import re
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
BASE = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games/").rstrip("/") + "/"


def request(payload=None, **query):
    token = os.environ.get("HUNTRESS_GM_TOKEN")
    if not token:
        token = re.search(r"HUNTRESS_GM_TOKEN=(gm1\.[\w.]+)", (ROOT / "AGENTS.md").read_text())[1]
    url = BASE + ("?" + urllib.parse.urlencode(query) if query else "")
    body = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=body, headers={
        "Authorization": "Bearer " + token, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=45) as response:
            return json.load(response)
    except urllib.error.HTTPError as exc:
        return {"http_status": exc.code, **json.loads(exc.read())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("op", choices=["list", "start", "status", "submit"])
    parser.add_argument("challenge", nargs="?")
    parser.add_argument("--answers", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.op == "list":
        data = request(op="list", all="true")
        data["challenges"] = [c for c in data.get("challenges", [])
                              if "web" in str(c.get("lane", "")) or str(c.get("id", "")).startswith("web-")]
    else:
        if not args.challenge or not args.challenge.startswith("web-"):
            parser.error("an explicit web challenge ID is required")
        if args.op == "status":
            data = request(op="status", challenge=args.challenge)
        else:
            payload = {"op": args.op, "challenge": args.challenge}
            if args.op == "submit":
                if not args.answers:
                    parser.error("submit requires --answers")
                payload["answers"] = json.loads(args.answers.read_text())
            data = request(payload)
    rendered = json.dumps(data, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered + "\n")
    print(rendered)


if __name__ == "__main__":
    main()
