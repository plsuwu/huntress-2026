#!/usr/bin/env python3
"""Day 10 metadata-forensics "Where Do We Begin?" (copy of day-11/12 solver; same handlers, CHAL swapped).

usage: python3 -I solve.py [single_solve|time_trial|mastery] [--retry] [--submit] [--timed]
Token from HUNTRESS_GM_TOKEN (env or nearest .env). Timed ranked deadline is ~50s (TT) / 80s (mastery),
so start -> download -> derive -> submit runs in one invocation. Dispatch is on the question-id suffix
(after `setN.` and `d13-msi-NNN-`); unknown kinds print an MSI fact dump to stderr and block the submit."""
import json, os, re, sys, urllib.request
from pathlib import Path

BASE = "https://2026.huntress.ctf.games/"
CHAL = "metadata-forensics-day-10-where-do-we-begin"
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1]))
from nested_zip import load_registry, resolve  # noqa: E402
import msi_meta  # noqa: E402


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



# ---------------------------------------------------------------- generic helpers (from day-09)
import hashlib, struct
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


def pe_machine(d):
    """COFF Machine u16 as '0x%04x' via e_lfanew -> 'PE\\0\\0'; 'none' for decoys (no MZ / no PE signature)."""
    if d[:2] != b"MZ" or len(d) < 0x40:
        return "none"
    off, = struct.unpack("<I", d[0x3C:0x40])
    if d[off:off + 4] != b"PE\0\0" or len(d) < off + 6:
        return "none"
    return "0x%04x" % struct.unpack("<H", d[off + 4:off + 6])[0]


def elf_machine(d):
    """ELF e_machine u16 (honours EI_DATA endianness); 'none' if not ELF."""
    if d[:4] != b"\x7fELF" or len(d) < 20:
        return "none"
    return "0x%04x" % struct.unpack("<H" if d[5] == 1 else ">H", d[18:20])[0]


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



def pe_entry_rva(d):
    if d[:2] != b"MZ" or len(d) < 0x40:
        return "none"
    off, = struct.unpack("<I", d[0x3C:0x40])
    if d[off:off + 4] != b"PE\0\0":
        return "none"
    return "0x%08x" % struct.unpack("<I", d[off + 24 + 16:off + 24 + 20])[0]


def pe_original_filename(d):
    """VS_VERSION_INFO StringFileInfo 'OriginalFilename' value (UTF-16LE), 'none' if absent."""
    key = "OriginalFilename".encode("utf-16le") + b"\0\0"
    i = d.find(key)
    if i < 0:
        return "none"
    p = i + len(key)
    while d[p:p + 2] == b"\0\0":
        p += 2
    e = p
    while d[e:e + 2] != b"\0\0":
        e += 2
    return d[p:e].decode("utf-16le")


def dotnet_mvid(d):
    """Module-table MVID via the #GUID heap, as lowercase 8-4-4-4-12."""
    import uuid
    r = d.find(b"BSJB")
    if r < 0:
        return "none"
    vl, = struct.unpack("<I", d[r + 12:r + 16])
    p = r + 16 + vl + 2
    ns, = struct.unpack("<H", d[p:p + 2]); p += 2
    streams = {}
    for _ in range(ns):
        off, size = struct.unpack("<II", d[p:p + 8]); p += 8
        e = d.index(b"\0", p); name = d[p:e].decode(); p = (e + 1 + 3) & ~3
        streams[name] = (r + off, size)
    t, _ = streams.get("#~", streams.get("#-"))
    heaps = d[t + 6]
    valid, = struct.unpack("<Q", d[t + 8:t + 16])
    q = t + 24 + 4 * bin(valid).count("1")  # first table = Module
    sidx = 4 if heaps & 1 else 2
    gidx = 4 if heaps & 2 else 2
    q += 2 + sidx
    gi = int.from_bytes(d[q:q + gidx], "little")
    g, _ = streams["#GUID"]
    return str(uuid.UUID(bytes_le=d[g + (gi - 1) * 16:g + gi * 16]))


def facts(data):
    """Everything derivable from one MSI: Property table + SummaryInformation revision."""
    props = msi_meta.property_table(data)
    return {"props": props, "revision": msi_meta.revision_number(data)}


def kind(qid):
    return re.sub(r"^set\d+\.", "", qid).replace("_", "-")


def _msi(key):
    return lambda m, b: msi_meta.property_table(b)[key]


def _filename(m, qid):
    """`<stem>-filename` -> member whose name minus extension equals stem (evidence-note vs evidence-note-copy)."""
    stem = qid[:-len("-filename")]
    return next((n for n in m if n.rsplit(".", 1)[0].lower() == stem), None) or \
        next((n for n in m if n.lower().startswith(stem)), None)


HANDLERS = [
    ("-size-bytes", lambda m, b: str(len(b))),
    ("-md5", lambda m, b: hashlib.md5(b).hexdigest()),
    ("-sha1", lambda m, b: hashlib.sha1(b).hexdigest()),
    ("-sha256", lambda m, b: hashlib.sha256(b).hexdigest()),
    ("magic-prefix-hex-16", lambda m, b: b[:16].hex()),
    ("utf8-bom-present", lambda m, b: "true" if b.startswith(b"\xef\xbb\xbf") else "false"),
    ("jpeg-sof-dimensions-pixels", lambda m, b: sof_dims(b)),
    ("png-ihdr-dimensions-pixels", lambda m, b: png_dims(b)),
    ("pdf-header-version", lambda m, b: pdf_header_version(b)),
    ("zip-central-directory-member-names", lambda m, b: json.dumps(cd_names(b), separators=(",", ":"), ensure_ascii=False)),
    ("pe-coff-machine-u16-hex", lambda m, b: pe_machine(b)),
    ("pe-address-of-entry-point-rva-hex", lambda m, b: pe_entry_rva(b)),
    ("pe-version-original-filename", lambda m, b: pe_original_filename(b)),
    ("dotnet-mvid-guid", lambda m, b: dotnet_mvid(b)),
    # MSI GUIDs are graded as stored: uppercase WITH braces (bare/lowercase -> incorrect)
    ("msi-product-code-guid", _msi("ProductCode")),
    ("msi-upgrade-code-guid", _msi("UpgradeCode")),
    ("msi-product-version", _msi("ProductVersion")),
    ("msi-product-name", _msi("ProductName")),
    ("eocd-total-entries", lambda m, b: str((eocd(b) or {}).get("total_entries", "none"))),
]


def synthesis_sha256_order(q, reg):
    names = json.loads(re.search(r"Files: (\[.*\])", q["question"]).group(1))
    members = resolve(reg, q["artifact"])[0]
    return json.dumps(sorted(names, key=lambda n: hashlib.sha256(members[n]).hexdigest()), separators=(",", ":"))


def answer(q, reg):
    qid = kind(q["id"])
    if "synthesis-sha256-order" in qid:
        return synthesis_sha256_order(q, reg)
    members, member, data = resolve(reg, q["artifact"])
    if qid.endswith("-filename") and not qid.endswith("original-filename"):
        return _filename(members, qid) if not member else member
    if data is None:
        print("NOFILE", q["id"], q["artifact"], list(members), file=sys.stderr)
        return None
    for suffix, fn in HANDLERS:
        if qid.endswith(suffix):
            try:
                return fn(members, data)
            except Exception as ex:  # noqa: BLE001
                print("HANDLER-ERR", q["id"], repr(ex), file=sys.stderr)
                return None
    print("UNKNOWN-KIND", q["id"], member, file=sys.stderr)
    return None


def main():
    mode = next((a for a in sys.argv[1:] if not a.startswith("--")), "single_solve")
    chal = CHAL if mode == "single_solve" else f"{CHAL}/{mode}"
    body = {"op": "retry" if "--retry" in sys.argv else "practice" if "--practice" in sys.argv else "start", "challenge": chal}
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
    if "--submit" in sys.argv and (not unhandled or "--force" in sys.argv):  # --force: ranked 30 s routes, bank partial answers
        r = post({"op": "submit", "challenge": chal, "answers": answers})
        print(json.dumps({k: r[k] for k in r if k in ("result", "remaining", "flag", "status")}))


if __name__ == "__main__":
    main()
