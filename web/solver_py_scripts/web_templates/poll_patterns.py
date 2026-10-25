#!/usr/bin/env python3
"""Generic plant/poll helpers for async or callback-shaped bugs - a template,
not a recoverer.

Three graded recoverers in `recovery_lib.py` share one underlying shape - cause
something privileged to happen, then observe the side effect - but only one of
them is genuinely asynchronous:

  - `rec_session_fixation`: plant a client-chosen session id, then immediately
    fetch a second route with that same id. Synchronous - no polling needed.
  - `rec_reflect_callback`: reflect a probe, then fetch a callback route with a
    forged Referer. Also synchronous.
  - `rec_stored_xss_bot`: post a payload, then an out-of-band moderator bot
    visits it on its own schedule - the only one where you must actually wait
    and re-check, which it does with a bespoke ~12-attempt / 6s-interval loop.

`poll_for_proof` below generalizes that third shape for the next genuinely
asynchronous bug: trigger whatever you need yourself (post a payload, submit a
form, whatever causes the privileged visit), then hand this a URL (or a
callable that re-derives one, if the poll target needs recomputing per
attempt - e.g. it embeds an attempt counter or a freshly-issued id) and it
polls for you, saving each attempt as evidence.

Not wired into `classify_and_recover` - still write your own trigger step;
this only replaces the "wait for the async side effect" loop. Needs a `Target`
(from `recovery_lib`) for its `.get()` - build one, don't hand-roll a second
HTTP client.
"""
import time


def poll_for_proof(t, url_or_fn, proof_re, attempts=12, interval=6.0, save_prefix="poll"):
    """GET `url_or_fn()` (or the fixed string `url_or_fn`) up to `attempts` times,
    `interval` seconds apart, until `proof_re` matches the response body or the
    attempts run out. Returns the matched proof string, or None.

    Mirrors `rec_stored_xss_bot`'s defaults (12 attempts, 6s apart - ~72s bounded
    wait) since that is the only graded timing evidence available so far; adjust
    if a new target's own evidence (e.g. a stated bot interval) suggests otherwise."""
    for i in range(attempts):
        time.sleep(interval)
        url = url_or_fn() if callable(url_or_fn) else url_or_fn
        status, body = t.get(url, save=f"{save_prefix}_{i}")
        if status != 200:
            continue
        match = proof_re.search(body)
        if match:
            return match.group(0)
    return None
