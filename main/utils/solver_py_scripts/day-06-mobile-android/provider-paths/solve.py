#!/usr/bin/env python3
"""Day 06 mobile-android: Provider Paths.

The APK carries assets/huntress_puzzle.json; day 6 single_solve: {"codec": "hex", "data": <hex of ANS-...>}.
Codecs handled: hex, base64, xor(+key), split(parts/order), generic fallback.
Static only; nothing from the APK is executed.

    python3 -I solve.py path/to/provider_paths.apk
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
CHALLENGE = "mobile-android-day-06-provider-paths"
ANS_RE = re.compile(rb"ANS-[0-9A-F]{16}")


def _xor(raw, key):
    key = [key] if isinstance(key, int) else list(key)
    return bytes(x ^ key[i % len(key)] for i, x in enumerate(raw))


def _split(puzzle):
    # parts[i] belongs at position order[i] (order is a permutation); joined -> ANS-<16 hex>
    parts, order = puzzle["parts"], puzzle["order"]
    out = [None] * len(parts)
    for part, pos in zip(parts, order):
        out[pos] = part
    return "".join(out).encode()


def _generic(puzzle):
    """Unknown codec: breadth-first search (depth 3) over cheap byte transforms of `data` / joined `parts`
    (reverse, hex, base64/url-safe, base32, rot13, xor with `key`); accept the first ANS-<16 hex> hit."""
    import codecs
    key = puzzle.get("key")
    xors = [lambda b, k=k: _xor(b, k) for k in ([key] if key is not None else [])]
    xors += [lambda b, k=k: _xor(b, k) for k in (puzzle.get("keys") or [])]
    tf = [lambda b: b[::-1], bytes.fromhex, base64.b64decode, base64.urlsafe_b64decode, base64.b32decode,
          lambda b: codecs.decode(b.decode(), "rot13").encode()] + xors
    seeds = [str(puzzle.get("data", "")), "".join(map(str, puzzle.get("parts", [])))]
    frontier = [s.encode() for s in seeds if s]
    seen = set(frontier)
    for _ in range(3):
        nxt = []
        for v in frontier:
            m = ANS_RE.search(v)
            if m:
                return m.group(0)
            for f in tf:
                try:
                    o = f(v)
                except Exception:
                    continue
                if o and o not in seen:
                    seen.add(o)
                    nxt.append(o)
        frontier = nxt
    for v in frontier:
        m = ANS_RE.search(v)
        if m:
            return m.group(0)
    raise NotImplementedError(f"codec {puzzle.get('codec')!r}: {json.dumps(puzzle)[:300]}")


def decode(puzzle):
    codec = puzzle.get("codec", "xor")
    if codec == "split":
        return _split(puzzle)
    if codec == "xor":
        return _xor(base64.b64decode(puzzle["data"]), puzzle["key"])
    if codec == "hex":  # day 6: data is the hex of the ANS- string
        return bytes.fromhex(puzzle["data"])
    if codec == "base64":
        return base64.b64decode(puzzle["data"])
    return _generic(puzzle)  # variants may differ in time_trial/mastery; extend above when one is seen


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
        d = os.environ.get("TMPDIR", "/tmp")
        json.dump(start, open(os.path.join(d, "pp_start.json"), "w"), indent=1)  # keep fixture for offline work
        tmp = os.path.join(d, "pp.apk")
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
