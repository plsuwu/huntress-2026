"""Minimal stdlib MSI (OLE/CFB) reader: Property table + SummaryInformation revision."""
import struct

_ALPHA = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz._"


def decode_stream_name(name: str) -> str:
    out = []
    for ch in name:
        c = ord(ch)
        if 0x3800 <= c < 0x4800:
            v = c - 0x3800
            out.append(_ALPHA[v & 0x3F]); out.append(_ALPHA[(v >> 6) & 0x3F])
        elif 0x4800 <= c < 0x4840:
            out.append(_ALPHA[c - 0x4800])
        elif c == 0x4840:
            out.append("!")
        else:
            out.append(ch)
    return "".join(out)


def read_cfb(data: bytes) -> dict[str, bytes]:
    ssz = 1 << struct.unpack_from("<H", data, 30)[0]
    mssz = 1 << struct.unpack_from("<H", data, 32)[0]
    nfat, dirstart, cutoff, minifat_start, nminifat, difat_start, ndifat = struct.unpack_from(
        "<IIIIIII", data, 44)[0], *struct.unpack_from("<I", data, 48), *struct.unpack_from("<III", data, 56), *struct.unpack_from("<II", data, 68)
    sec = lambda i: data[512 + i * ssz: 512 + (i + 1) * ssz]
    difat = list(struct.unpack_from("<109I", data, 76))
    d = difat_start
    for _ in range(ndifat):
        s = struct.unpack("<%dI" % (ssz // 4), sec(d)); difat += list(s[:-1]); d = s[-1]
    fat = []
    for i in difat:
        if i < 0xFFFFFFFA:
            fat += struct.unpack("<%dI" % (ssz // 4), sec(i))

    def chain(start, getsec=sec):
        out, i, seen = [], start, set()
        while i < 0xFFFFFFFA and i not in seen:
            seen.add(i); out.append(getsec(i)); i = fat[i]
        return b"".join(out)

    dirs = chain(dirstart)
    ents = []
    for o in range(0, len(dirs), 128):
        e = dirs[o:o + 128]
        nl = struct.unpack_from("<H", e, 64)[0]
        name = e[:max(nl - 2, 0)].decode("utf-16le")
        typ = e[66]
        start, size = struct.unpack_from("<I", e, 116)[0], struct.unpack_from("<Q", e, 120)[0]
        ents.append((name, typ, start, size))
    root = ents[0]
    ministream = chain(root[2])[:root[3]]
    minifat = struct.unpack("<%dI" % (len(chain(minifat_start)) // 4), chain(minifat_start)) if nminifat else ()
    msec = lambda i: ministream[i * mssz:(i + 1) * mssz]

    def mchain(start):
        out, i, seen = [], start, set()
        while i < 0xFFFFFFFA and i not in seen:
            seen.add(i); out.append(msec(i)); i = minifat[i]
        return b"".join(out)

    res = {}
    for name, typ, start, size in ents:
        if typ != 2:
            continue
        raw = chain(start) if size >= cutoff else mchain(start)
        res[decode_stream_name(name)] = raw[:size]
    return res


def string_pool(streams):
    pool, sd = streams["!_StringPool"], streams["!_StringData"]
    cp, flags = struct.unpack_from("<HH", pool, 0)
    big = bool(flags & 0x8000)
    strs, pos, o = [""], 0, 4
    while o + 4 <= len(pool):
        ln, rc = struct.unpack_from("<HH", pool, o); o += 4
        if ln == 0 and rc != 0:  # long string: 4-byte length follows
            ln = rc << 16 | struct.unpack_from("<H", pool, o)[0]; o += 4  # rare
        strs.append(sd[pos:pos + ln].decode("cp1252", "replace")); pos += ln
    return strs, (3 if big else 2)


def property_table(data: bytes) -> dict[str, str]:
    st = read_cfb(data)
    strs, w = string_pool(st)
    raw = st["!Property"]
    n = len(raw) // (2 * w)
    rd = lambda off: int.from_bytes(raw[off:off + w], "little")
    return {strs[rd(i * w)]: strs[rd(n * w + i * w)] for i in range(n)}


def revision_number(data: bytes) -> str | None:
    st = read_cfb(data)
    si = next((v for k, v in st.items() if k.endswith("SummaryInformation") and "Document" not in k), None)
    if si is None:
        return None
    nsec = struct.unpack_from("<I", si, 24)[0]
    off = struct.unpack_from("<I", si, 44)[0]
    cnt = struct.unpack_from("<I", si, off + 4)[0]
    for i in range(cnt):
        pid, po = struct.unpack_from("<II", si, off + 8 + i * 8)
        if pid == 9:
            ln = struct.unpack_from("<I", si, off + po + 4)[0]
            return si[off + po + 8: off + po + 8 + ln].split(b"\0")[0].decode("cp1252")
    return None
