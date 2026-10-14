#!/usr/bin/env python3
"""Day 04 mobile-android: Pending Intent Relay (single_solve).

The APK carries assets/huntress_puzzle.json: {"codec": "xor", "data": <b64>, "key": <int>}.
base64-decode `data`, XOR each byte with `key` -> the `ANS-<16 hex>` answer.
Static only; nothing from the APK is executed.

    python3 -I solve.py path/to/pending_intent_relay.apk
    python3 -I solve.py --submit            # fetch + solve + submit via GM (needs HUNTRESS_GM_TOKEN)
"""
import base64
import json
import os
import re
import sys
import urllib.request
import zipfile

GM = "https://2026.huntress.ctf.games/"
CHALLENGE = "mobile-android-day-04-pending-intent-relay"
ANS_RE = re.compile(rb"ANS-[0-9A-F]{16}")


def decode(puzzle):
    raw = base64.b64decode(puzzle["data"])
    codec = puzzle.get("codec", "xor")
    if codec != "xor":  # variants may differ in time_trial/mastery; extend here
        raise NotImplementedError(f"codec {codec!r}")
    key = puzzle["key"]
    key = [key] if isinstance(key, int) else list(key)
    return bytes(b ^ key[i % len(key)] for i, b in enumerate(raw))


def solve_apk(path):
    with zipfile.ZipFile(path) as z:
        puzzle = json.loads(z.read("assets/huntress_puzzle.json"))
    out = decode(puzzle)
    if not ANS_RE.fullmatch(out):
        raise ValueError(f"decoded value is not an answer code: {out!r}")
    return out.decode()


def gm(payload=None, query=None):
    tok = os.environ["HUNTRESS_GM_TOKEN"]
    hdr = {"Authorization": f"Bearer {tok}"}
    if payload is not None:
        hdr["Content-Type"] = "application/json"
        req = urllib.request.Request(GM + (query or ""), json.dumps(payload).encode(), hdr)
    else:
        req = urllib.request.Request(GM + (query or ""), headers=hdr)
    with urllib.request.urlopen(req) as r:
        return r.read()


def main():
    if sys.argv[1:2] == ["--submit"]:
        # optional 2nd arg: challenge suffix, e.g. "/time_trial" (timed: start -> download -> submit, one shot)
        chal = CHALLENGE + (sys.argv[2] if len(sys.argv) > 2 else "")
        start = json.loads(gm({"op": "start", "challenge": chal}))
        print(start.get("deadline"), start.get("unique_attempts_remaining"), file=sys.stderr)
        apk = gm(query=start["download"].lstrip("/"))
        tmp = os.path.join(os.environ.get("TMPDIR", "/tmp"), "pir.apk")
        open(tmp, "wb").write(apk)
        ans = solve_apk(tmp)
        res = json.loads(gm({"op": "submit", "challenge": chal,
                             "answers": {start["answer_ids"][0]: ans}}))
        res.pop("guidance", None)
        print(ans, json.dumps(res))
    else:
        print(solve_apk(sys.argv[1]))


if __name__ == "__main__":
    main()
