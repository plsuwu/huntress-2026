#!/usr/bin/env python3
"""Day 08 metadata-forensics 'What\'s in the Box?' - single_solve / time_trial / mastery.

usage: solve.py [single_solve|time_trial|mastery] [--retry] [--submit] [--timed]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Timed deadline is ~14s, so the whole
start -> download -> derive -> submit loop runs in one invocation (python3 -I).
Dispatch is on the question-id suffix (after `dNN-zip-NNN-` / `setN.`); unknown kinds are reported
with a ZIP fact dump to stderr and block the submit."""
import hashlib, json, os, re, struct, sys, urllib.request
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-08-whats-in-the-box"
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



def sof_dims(d):
    """'<w>x<h>' from the first SOF marker, else 'none' (same as day-04 picture-this)."""
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


def png_dims(d):
    i = d.find(b"IHDR")
    if i < 0:
        return "none"
    w, h = struct.unpack(">II", d[i + 4:i + 12])
    return f"{w}x{h}"


def pdf_header_version(d):
    m = re.search(rb"%PDF-(\d+\.\d+)", d[:1024])
    return m.group(1).decode() if m else "none"


def cd_names(d):
    """Central-directory member names, raw and in stored order (full paths, no normalisation).
    Walks the CD from the EOCD offset; falls back to scanning for PK\\x01\\x02 signatures."""
    e = eocd(d)
    out = []
    if e and d[e["cd_offset"]:e["cd_offset"] + 4] == b"PK\x01\x02":
        p = e["cd_offset"]
        while d[p:p + 4] == b"PK\x01\x02":
            flag, = struct.unpack("<H", d[p + 8:p + 10])
            n, x, c = struct.unpack("<3H", d[p + 28:p + 34])
            raw = d[p + 46:p + 46 + n]
            out.append(raw.decode("utf-8" if flag & 0x800 else "cp437", "replace"))
            p += 46 + n + x + c
        return out
    for m in re.finditer(re.escape(b"PK\x01\x02"), d):
        p = m.start()
        flag, = struct.unpack("<H", d[p + 8:p + 10])
        n, = struct.unpack("<H", d[p + 28:p + 30])
        out.append(d[p + 46:p + 46 + n].decode("utf-8" if flag & 0x800 else "cp437", "replace"))
    return out


def _first_note(members):
    return next((n for n in members if n.lower().startswith("evidence-note")),
                next(n for n in members if n.lower().endswith(".txt")))


def _e(key):
    return lambda m, b: str((eocd(b) or {}).get(key, "none"))


HANDLERS = [
    ("evidence-note-filename", lambda m, b: _first_note(m)),
    ("evidence-note-size-bytes", lambda m, b: str(len(b))),
    ("evidence-note-md5", lambda m, b: hashlib.md5(b).hexdigest()),
    ("evidence-note-sha1", lambda m, b: hashlib.sha1(b).hexdigest()),
    ("evidence-note-sha256", lambda m, b: hashlib.sha256(b).hexdigest()),
    ("evidence-note-magic-prefix-hex-16", lambda m, b: b[:16].hex()),
    ("evidence-note-text-utf8-bom-present", lambda m, b: "true" if b.startswith(b"\xef\xbb\xbf") else "false"),
    ("jpeg-sof-dimensions-pixels", lambda m, b: sof_dims(b)),
    ("png-ihdr-dimensions-pixels", lambda m, b: png_dims(b)),
    ("pdf-header-version", lambda m, b: pdf_header_version(b)),
    ("zip-central-directory-member-names", lambda m, b: json.dumps(cd_names(b), separators=(",", ":"), ensure_ascii=False)),
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
    if not member:
        member = _first_note(members)
        data = members[member]
    if data is None:
        print("NOFILE", q["id"], q["artifact"], list(members), file=sys.stderr)
        return None
    qid = re.sub(r"^set\d+\.", "", q["id"]).replace("_", "-").replace(".", "-")
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            try:
                return fn(members, data)
            except Exception as ex:  # noqa: BLE001
                print("HANDLER-ERR", q["id"], repr(ex), file=sys.stderr)
                return None
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
