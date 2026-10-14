#!/usr/bin/env python3
"""start/retry/practice -> download -> solve -> submit for "Hatch Careful" (Game Master).

    # needs pyzipper (AES zip) and, for uv pythons, SSL_CERT_FILE:
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py time_trial --op retry
    HUNTRESS_GM_TOKEN=... live_run.py mastery    --op start --dry

Everything the server returns (start JSON, every download) is written to --out
*before* solving, so a failed timed attempt still leaves a fixture to work from.
The token is only ever read from the environment and is never printed.
"""
from __future__ import annotations

import argparse
import http.client
import io
import json
import os
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import hatch_careful as hc  # noqa: E402
ep = hc.ep

HOST = "2026.huntress.ctf.games"
CHALLENGE = "script-analysis-day-05-python-carrier-lab"
HEX64 = re.compile(r"[0-9a-f]{64}")


class Client:
    def __init__(self, token: str):
        self.token = token
        self.conn = http.client.HTTPSConnection(HOST, timeout=30)

    def request(self, method: str, path: str, body: dict | None = None) -> tuple[int, bytes]:
        headers = {"Authorization": f"Bearer {self.token}"}
        data = None
        if body is not None:
            data = json.dumps(body, separators=(",", ":")).encode()
            headers["Content-Type"] = "application/json"
        for attempt in (0, 1):
            try:
                self.conn.request(method, path, body=data, headers=headers)
                resp = self.conn.getresponse()
                return resp.status, resp.read()
            except (http.client.HTTPException, OSError):
                if attempt:
                    raise
                self.conn = http.client.HTTPSConnection(HOST, timeout=30)
        raise AssertionError


def find_downloads(node, out: list[dict]) -> None:
    """Collect {link, filename, sha256} for every download-looking value, in order."""
    if isinstance(node, dict):
        link = node.get("download")
        if isinstance(link, str) and HEX64.search(link):
            out.append({"link": link, "filename": node.get("filename") or node.get("name"),
                        "sha256": node.get("sha256")})
        for k, v in node.items():
            if k == "downloads" and isinstance(v, list):
                for item in v:
                    if isinstance(item, str) and HEX64.search(item):
                        out.append({"link": item, "filename": None, "sha256": None})
            elif k != "download":
                find_downloads(v, out)
    elif isinstance(node, list):
        for item in node:
            find_downloads(item, out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["single_solve", "time_trial", "mastery"])
    ap.add_argument("--op", choices=["start", "retry", "practice"], default="start")
    ap.add_argument("--dry", action="store_true", help="solve but do not submit")
    ap.add_argument("--out", default=None, help="directory for start.json / downloads")
    args = ap.parse_args()

    token = os.environ.get("HUNTRESS_GM_TOKEN")
    if not token:
        print("HUNTRESS_GM_TOKEN not set", file=sys.stderr)
        return 2
    challenge = CHALLENGE if args.mode == "single_solve" else f"{CHALLENGE}/{args.mode}"
    t0 = time.perf_counter()
    cli = Client(token)
    if not args.dry or args.op != "start":
        cli.request("GET", "/?op=list&day=5&lane=script&search=zzz")  # warm TLS before the timer starts

    def lap(label: str) -> None:
        print(f"[{(time.perf_counter() - t0) * 1000:7.0f} ms] {label}", file=sys.stderr)

    status, raw = cli.request("POST", "/", {"op": args.op, "challenge": challenge})
    lap(f"{args.op} -> HTTP {status}, {len(raw)} bytes")
    start = json.loads(raw)
    out_dir = Path(args.out or f"fixtures/run-{int(time.time())}-{args.mode}")
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "start.json").write_bytes(raw)
    questions = start.get("questions")
    if not questions:
        print(json.dumps({k: v for k, v in start.items() if k != "guidance"})[:1500])
        return 1
    lap(f"{len(questions)} questions; deadline={start.get('deadline')}")

    downloads: list[dict] = []
    find_downloads(start, downloads)
    seen, ordered = set(), []
    for d in downloads:
        cap = HEX64.search(d["link"]).group(0)
        if cap not in seen:
            seen.add(cap)
            ordered.append({**d, "cap": cap})
    deliveries: list[dict[str, bytes]] = []
    for i, d in enumerate(ordered):
        path = f"/?op=download&challenge={challenge}&delivery={i}&capability={d['cap']}"
        status, blob = cli.request("GET", path)
        if status != 200:
            print(f"download {i}: HTTP {status}", file=sys.stderr)
            return 1
        name = d.get("filename") or f"delivery-{i + 1}.bin"
        (out_dir / f"{i + 1}-{name}").write_bytes(blob)
        if blob.startswith(b"PK\x03\x04"):
            deliveries.extend(ep.expand_deliveries(ep.read_zip(blob), name))
        else:
            d = ep.Delivery({name: blob})
            d.name = name
            deliveries.append(d)
        lap(f"download {i + 1}/{len(ordered)}: {name} ({len(blob)} bytes)")

    answers, errors, _log = hc.solve(deliveries, questions)
    lap(f"solved {len(answers)}/{len(questions)} answers, {len(errors)} errors")
    (out_dir / "answers.json").write_text(json.dumps(answers, indent=1))
    for qid, err in errors.items():
        print(f"ERR {qid}: {err}", file=sys.stderr)
    if args.dry or not answers:
        return 1 if errors else 0

    status, raw = cli.request("POST", "/", {"op": "submit", "challenge": challenge, "answers": answers})
    lap(f"submit -> HTTP {status}")
    res = json.loads(raw)
    res.pop("guidance", None)
    res.pop("questions", None)
    (out_dir / "result.json").write_text(json.dumps(res, indent=1))
    print(json.dumps(res)[:3000])
    return 0 if res.get("result") == "CORRECT" else 1


if __name__ == "__main__":
    sys.exit(main())
