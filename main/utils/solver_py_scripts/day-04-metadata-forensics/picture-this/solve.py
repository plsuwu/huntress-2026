#!/usr/bin/env python3
"""Day 04 metadata-forensics 'Picture This' - single_solve / time_trial / mastery.

usage: solve.py [single_solve|time_trial|mastery] [--retry] [--submit]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Deadlines are 10s, so the
whole start -> download -> derive -> submit loop runs in one invocation.

Answers are dispatched on the question id suffix, so new variants/sets with the
same question kinds need no code changes. Unknown kinds are reported, not guessed.
Archives may be nested (mastery); see utils/solver_py_scripts/nested_zip.py."""
import hashlib, json, os, re, struct, sys, urllib.request
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-04-picture-this"
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1]))  # utils/solver_py_scripts (python -I drops script dir)
from nested_zip import load_registry, resolve  # noqa: E402


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


def sof_dims(d):
    """'<w>x<h>' from the first SOF marker, else 'none'."""
    i = 2
    while i + 4 <= len(d):
        if d[i] != 0xFF:
            i += 1
            continue
        m = d[i + 1]
        if m == 0xFF:
            i += 1
        elif m == 0x01 or 0xD0 <= m <= 0xD8:
            i += 2
        else:
            if 0xC0 <= m <= 0xCF and m not in (0xC4, 0xC8, 0xCC):
                h, w = struct.unpack(">HH", d[i + 5:i + 9])
                return f"{w}x{h}"
            if m == 0xDA:
                break
            i += 2 + struct.unpack(">H", d[i + 2:i + 4])[0]
    return "none"


# suffix -> fn(zipname, member_bytes_or_None, members) ; matched with endswith
def _first_note(members):
    return next(n for n in members if n.lower().endswith(".txt"))


HANDLERS = [
    ("evidence-note-filename", lambda m, b: _first_note(m)),
    ("evidence-note-size-bytes", lambda m, b: str(len(b))),
    ("evidence-note-md5", lambda m, b: hashlib.md5(b).hexdigest()),
    ("evidence-note-sha1", lambda m, b: hashlib.sha1(b).hexdigest()),
    ("evidence-note-sha256", lambda m, b: hashlib.sha256(b).hexdigest()),
    ("evidence-note-magic-prefix-hex-16", lambda m, b: b[:16].hex()),
    ("evidence-note-text-utf8-bom-present",
     lambda m, b: "true" if b.startswith(b"\xef\xbb\xbf") else "false"),
    ("jpeg-sof-dimensions-pixels", lambda m, b: sof_dims(b)),
]


def answer(q, reg):
    members, member, data = resolve(reg, q["artifact"])
    if not member:
        member = _first_note(members)
        data = members[member]
    qid = q["id"].split(".", 1)[-1]
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            return fn(members, data)
    return None


def main():
    mode = next((a for a in sys.argv[1:] if not a.startswith("--")), "single_solve")
    chal = CHAL if mode == "single_solve" else f"{CHAL}/{mode}"
    st = post({"op": "retry" if "--retry" in sys.argv else "start", "challenge": chal})
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
