#!/usr/bin/env python3
"""Day 06 metadata-forensics 'Portable Deception' - single_solve / time_trial / mastery.

usage: solve.py [single_solve|time_trial|mastery] [--retry] [--submit]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Timed deadlines are 8-9s, so the whole
start -> download -> derive -> submit loop runs in one invocation (python3 -I).

Answers are dispatched on the question-id kind (after `dNN-pdf-NNN-` / `setN.`).
Unknown kinds are reported (with a PDF fact dump to stderr), never guessed; a submit is
only sent when every question was handled.
Archives may be nested (mastery); see utils/solver_py_scripts/nested_zip.py."""
import hashlib, json, os, re, sys, urllib.request
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-06-portable-deception"
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


# ---------------------------------------------------------------- PDF helpers
def header_version(d):
    """Version token of the first `%PDF-` marker (searched in the first 1024 bytes), else 'none'."""
    m = re.search(rb"%PDF-(\d+\.\d+)", d[:1024])
    return m.group(1).decode() if m else "none"


def catalog_versions(d):
    """All `/Version /x.y` names in the file, in file order (catalog overrides of the header)."""
    return [m.group(1).decode() for m in re.finditer(rb"/Version\s*/(\d+\.\d+)", d)]


def info_field(d, key):
    """Last literal-string value of /Key (...) (incremental updates append, so last wins)."""
    vals = re.findall(rb"/" + key.encode() + rb"\s*\(((?:\\.|[^\\()])*)\)", d)
    return vals[-1].decode("latin1") if vals else "none"


def page_count(d):
    m = re.findall(rb"/Type\s*/Pages\b[^>]*?/Count\s+(\d+)", d)
    return m[0].decode() if m else str(len(re.findall(rb"/Type\s*/Page\b", d)))


def eof_count(d):
    return str(d.count(b"%%EOF"))


def startxref_values(d):
    return [int(x) for x in re.findall(rb"startxref\s+(\d+)", d)]


def facts(d):
    return {
        "size": len(d), "magic16": d[:16].hex(), "header": header_version(d),
        "catalog_versions": catalog_versions(d), "eof": d.count(b"%%EOF"),
        "startxref": startxref_values(d), "pages": page_count(d),
        "info": {k: info_field(d, k) for k in
                 ("Title", "Author", "Subject", "Keywords", "Creator", "Producer", "CreationDate", "ModDate")},
        "head": d[:80].decode("latin1"),
    }


# suffix -> fn(members, member_bytes) ; matched with endswith on the id (after `dNN-pdf-NNN-`)
HANDLERS = [
    ("pdf-header-version", lambda m, b: header_version(b)),
    ("pdf-catalog-version", lambda m, b: (catalog_versions(b) or ["none"])[-1]),
    ("pdf-page-count", lambda m, b: page_count(b)),
    ("pdf-eof-count", lambda m, b: eof_count(b)),
    ("pdf-info-title", lambda m, b: info_field(b, "Title")),
    ("pdf-info-author", lambda m, b: info_field(b, "Author")),
    ("pdf-info-creator", lambda m, b: info_field(b, "Creator")),
    ("pdf-info-producer", lambda m, b: info_field(b, "Producer")),
    ("pdf-info-creationdate", lambda m, b: info_field(b, "CreationDate")),
    ("pdf-info-moddate", lambda m, b: info_field(b, "ModDate")),
    ("pdf-size-bytes", lambda m, b: str(len(b))),
    ("pdf-md5", lambda m, b: hashlib.md5(b).hexdigest()),
    ("pdf-sha1", lambda m, b: hashlib.sha1(b).hexdigest()),
    ("pdf-sha256", lambda m, b: hashlib.sha256(b).hexdigest()),
    ("pdf-magic-prefix-hex-16", lambda m, b: b[:16].hex()),
]


def answer(q, reg):
    members, member, data = resolve(reg, q["artifact"])
    if data is None:
        print("NOFILE", q["id"], q["artifact"], list(members), file=sys.stderr)
        return None
    qid = q["id"].split(".", 1)[-1]
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            return fn(members, data)
    print("FACTS", member, json.dumps(facts(data)), file=sys.stderr)
    return None


def main():
    mode = next((a for a in sys.argv[1:] if not a.startswith("--")), "single_solve")
    chal = CHAL if mode == "single_solve" else f"{CHAL}/{mode}"
    st = post({"op": "retry" if "--retry" in sys.argv else "start", "challenge": chal})
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
    print(json.dumps(answers, indent=1), st.get("deadline"))
    if "--submit" in sys.argv and not unhandled:
        r = post({"op": "submit", "challenge": chal, "answers": answers})
        print(json.dumps({k: r[k] for k in r if k in ("result", "remaining", "flag", "status")}))


if __name__ == "__main__":
    main()
