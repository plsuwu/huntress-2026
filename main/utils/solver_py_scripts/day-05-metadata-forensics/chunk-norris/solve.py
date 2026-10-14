#!/usr/bin/env python3
"""Day 05 metadata-forensics 'Chunk Norris' - single_solve / time_trial / mastery.

usage: solve.py [single_solve|time_trial|mastery] [--retry] [--submit]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Deadlines are 10s, so the whole
start -> download -> derive -> submit loop runs in one invocation (python3 -I).

Answers are dispatched on the question-id suffix (after `dNN-png-NNN-` / `setN.`).
Unknown kinds are reported (with a PNG fact dump to stderr), never guessed.
Archives may be nested (mastery); see utils/solver_py_scripts/nested_zip.py."""
import json, os, re, struct, sys, urllib.request, zlib
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-05-chunk-norris"
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1]))  # utils/solver_py_scripts (python -I drops script dir)
from nested_zip import load_registry, resolve  # noqa: E402

SIG = b"\x89PNG\r\n\x1a\n"


def token():
    t = os.environ.get("HUNTRESS_GM_TOKEN")
    if t:
        return t
    for p in HERE.parents:
        e = p / ".env"
        if e.exists():
            m = re.search(r"HUNTRESS_GM_TOKEN=(\S+)", e.read_text())
            if m:
                return m.group(1).strip("'\"")
    sys.exit("no token")


def call(url, body=None):
    h = {"Authorization": f"Bearer {token()}"}
    if body is not None:
        h["Content-Type"] = "application/json"
        body = json.dumps(body).encode()
    return urllib.request.urlopen(urllib.request.Request(url, body, h)).read()


def post(body):
    return json.loads(call(BASE, body))


def chunks(d):
    """[(type, offset, length, data, crc_ok)] walking from the signature; stops at truncation."""
    out, i = [], 8
    while i + 12 <= len(d):
        ln, t = struct.unpack(">I4s", d[i:i + 8])
        body = d[i + 8:i + 8 + ln]
        crc = d[i + 8 + ln:i + 12 + ln]
        ok = len(crc) == 4 and struct.unpack(">I", crc)[0] == zlib.crc32(t + body) & 0xFFFFFFFF
        out.append((t.decode("latin1"), i, ln, body, ok))
        i += 12 + ln
    return out


def ihdr(d):
    c = next((c for c in chunks(d) if c[0] == "IHDR"), None)
    return struct.unpack(">IIBBBBB", c[3]) if c else None


def facts(d):
    cs = chunks(d)
    h = ihdr(d)
    return {"ihdr": h, "chunks": [c[0] for c in cs], "bad_crc": [c[0] for c in cs if not c[4]],
            "trailing": len(d) - (cs[-1][1] + 12 + cs[-1][2]) if cs else None}


HANDLERS = [
    ("png-ihdr-dimensions-pixels", lambda d: f"{ihdr(d)[0]}x{ihdr(d)[1]}"),
]


def answer(q, reg):
    members, member, data = resolve(reg, q["artifact"])
    qid = q["id"].split(".", 1)[-1]
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            return fn(data)
    print("FACTS", member, json.dumps(facts(data)), file=sys.stderr)
    return None


def main():
    mode = next((a for a in sys.argv[1:] if not a.startswith("--")), "single_solve")
    chal = CHAL if mode == "single_solve" else f"{CHAL}/{mode}"
    st = post({"op": "retry" if "--retry" in sys.argv else "start", "challenge": chal})
    (HERE / "artifacts" / f"{mode}-start.json").write_text(
        json.dumps({k: v for k, v in st.items() if k != "guidance"}, indent=1))
    if "download" not in st:
        sys.exit(f"no download in response: {json.dumps(st)[:300]}")
    data = call(BASE.rstrip("/") + st["download"])
    (HERE / "artifacts" / f"{mode}-latest.zip").write_bytes(data)
    reg = load_registry(data)
    answers = {}
    for q in st["questions"]:
        a = answer(q, reg)
        if a is None:
            print("UNHANDLED", q["id"], "-", q["question"], file=sys.stderr)
        else:
            answers[q["id"]] = a
    print(json.dumps(answers, indent=1), st.get("deadline"))
    if "--submit" in sys.argv:
        r = post({"op": "submit", "challenge": chal, "answers": answers})
        print(json.dumps({k: r[k] for k in r if k in ("result", "remaining", "flag", "status")}))


if __name__ == "__main__":
    main()
