#!/usr/bin/env python3
"""Day 07 metadata-forensics 'Seven-Zip, No Relation' - single_solve / time_trial / mastery.

usage: solve.py [single_solve|time_trial|mastery] [--retry] [--submit] [--timed]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Timed deadline is ~14s, so the whole
start -> download -> derive -> submit loop runs in one invocation (python3 -I).
Dispatch is on the question-id suffix (after `dNN-zip-NNN-` / `setN.`); unknown kinds are reported
with a ZIP fact dump to stderr and block the submit."""
import json, os, re, struct, sys, urllib.request
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-07-seven-zip-no-relation"
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1]))
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


# ---------------------------------------------------------------- ZIP helpers (raw bytes, no zipfile)
EOCD = b"PK\x05\x06"


def eocd(d):
    """Raw fields of the EOCD record nearest the end (spec: scan backwards), or None."""
    i = d.rfind(EOCD)
    while i >= 0 and len(d) - i < 22:
        i = d.rfind(EOCD, 0, i)
    if i < 0:
        return None
    disk, cd_disk, n_disk, n_total, cd_size, cd_off, clen = struct.unpack("<4H2IH", d[i + 4:i + 22])
    return dict(offset=i, disk=disk, cd_disk=cd_disk, entries_this_disk=n_disk, total_entries=n_total,
                cd_size=cd_size, cd_offset=cd_off, comment_len=clen, comment=d[i + 22:i + 22 + clen])


def count_sig(d, sig):
    return d.count(sig)


def facts(d):
    return {"size": len(d), "eocd": {k: v for k, v in (eocd(d) or {}).items()},
            "eocd_count": d.count(EOCD), "lfh": d.count(b"PK\x03\x04"), "cdh": d.count(b"PK\x01\x02"),
            "zip64": d.count(b"PK\x06\x06"), "head": d[:16].hex()}


def _e(key):
    return lambda m, b: str((eocd(b) or {}).get(key, "none"))


HANDLERS = [
    ("eocd-total-entries", _e("total_entries")),
    ("eocd-entries-this-disk", _e("entries_this_disk")),
    ("eocd-cd-size", _e("cd_size")),
    ("eocd-cd-offset", _e("cd_offset")),
    ("eocd-comment-length", _e("comment_len")),
    ("eocd-offset", _e("offset")),
    ("eocd-count", lambda m, b: str(b.count(EOCD))),
    ("local-header-count", lambda m, b: str(b.count(b"PK\x03\x04"))),
    ("central-header-count", lambda m, b: str(b.count(b"PK\x01\x02"))),
    ("size-bytes", lambda m, b: str(len(b))),
]


def answer(q, reg):
    members, member, data = resolve(reg, q["artifact"])
    if data is None:
        print("NOFILE", q["id"], q["artifact"], list(members), file=sys.stderr)
        return None
    qid = q["id"].split(".", 1)[-1].replace("_", "-").replace(".", "-")
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            return fn(members, data)
    print("FACTS", member, json.dumps(facts(data), default=repr), file=sys.stderr)
    return None


def main():
    mode = next((a for a in sys.argv[1:] if not a.startswith("--")), "single_solve")
    chal = CHAL if mode == "single_solve" else f"{CHAL}/{mode}"
    body = {"op": "retry" if "--retry" in sys.argv else "start", "challenge": chal}
    if mode != "single_solve" and "--timed" in sys.argv:
        body["run_for_time"] = True
    st = post(body)
    art = HERE / "artifacts"
    art.mkdir(exist_ok=True)
    (art / f"{mode}-start.json").write_text(json.dumps({k: v for k, v in st.items() if k != "guidance"}, indent=1))
    if "download" not in st:
        sys.exit(f"no download in response: {json.dumps(st)[:300]}")
    data = call(BASE.rstrip("/") + st["download"])
    (art / f"{mode}-latest.zip").write_bytes(data)
    reg = load_registry(data)
    answers, unhandled = {}, 0
    for q in st["questions"]:
        a = answer(q, reg)
        if a is None:
            unhandled += 1
            print("UNHANDLED", q["id"], "-", q["question"], file=sys.stderr)
        else:
            answers[q["id"]] = a
    print(json.dumps(answers, indent=1), st.get("deadline"), st.get("unique_attempts_remaining"))
    if "--submit" in sys.argv and not unhandled:
        r = post({"op": "submit", "challenge": chal, "answers": answers})
        print(json.dumps({k: r[k] for k in r if k in ("result", "remaining", "flag", "status")}))


if __name__ == "__main__":
    main()
