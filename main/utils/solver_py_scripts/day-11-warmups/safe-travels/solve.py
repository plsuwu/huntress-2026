#!/usr/bin/env python3
"""Day 11 warmups: Safe Travels (base64url layer: `-`/`_` alphabet, padded or not; also std base64).

Superset of day-10. Day 10 docs follow:
Day 10 warmups: Thirty-Two Below (base32 layer, RFC4648 upper alphabet, padded or not; also tolerates lowercase/whitespace).

Superset of day-09 escape-room. Day 9 docs follow:
Day 9 warmups: Escape Room (backslash escapes `\\u004e` / `\\x4e` / `\\116` / `\\n`, possibly layered, maybe quoted).

Superset of day-08 entity-relations: adds unescape_bs (JSON/C/Python-style escapes) as a blob
layer and mastery row encoding. Day-08 docs follow: html.unescape (repeated while entities remain) to
single/time-trial blobs and as a mastery row encoding. Day-07/06 docs follow.


Generalised from day-05 by-the-numbers: whitespace-separated numeric tokens in an
unknown base (oct/dec/hex/bin). All bases that parse are tried and the one whose
decode is most text-like (printable ASCII, letters/spaces) wins - this avoids the
"3-digit octal also parses as decimal" ambiguity. Layers are peeled while the
output still looks like a numeric list; the answer slug is taken from the last
non-empty line. Mastery rows (`NN<TAB>fragment`, mixed encodings) reuse day-05's decode_row.

usage: solve.py <blob.txt>
"""
import base64
import codecs
import re
import sys
import html
from urllib.parse import unquote

SLUG = re.compile(r"\b[a-z]+-[a-z]+-[a-z]+-\d{2}\b")
BASES = ((8, r"[0-7]+"), (10, r"\d+"), (16, r"[0-9a-fA-F]+"), (2, r"[01]+"))


def _score(t: str) -> float:
    ok = sum(c.isascii() and (c.isalnum() or c in " \t\r\n.,;:!?-_'\"()") for c in t)
    return ok / max(len(t), 1)


def decode_numbers(text: str) -> str | None:
    toks = text.split()
    if not toks:
        return None
    best, best_score = None, 0.0
    for base, pat in BASES:
        if not all(re.fullmatch(pat, t) for t in toks):
            continue
        try:
            vals = [int(t, base) for t in toks]
        except ValueError:
            continue
        if not all(0 < v < 128 for v in vals):
            continue
        t = "".join(map(chr, vals))
        if not all(c.isprintable() or c in "\r\n\t" for c in t):
            continue
        if (s := _score(t)) > best_score:
            best, best_score = t, s
    return best if best_score >= 0.9 else None


ENT = re.compile(r"&(?:#\d+|#[xX][0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);")


def unescape_all(t: str) -> str:
    while ENT.search(t):
        n = html.unescape(t)
        if n == t:
            break
        t = n
    return t


BS = re.compile(r"\\(?:u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|U[0-9a-fA-F]{8}|x[0-9a-fA-F]{2}|[0-7]{1,3}|[abfnrtv0\\'\"/])")
_SIMPLE = {"a": "\a", "b": "\b", "f": "\f", "n": "\n", "r": "\r", "t": "\t", "v": "\v"}


def _bs_one(m: re.Match) -> str:
    e = m.group(0)[1:]
    c = e[0]
    if c == "u" and e[1] == "{":
        return chr(int(e[2:-1], 16))
    if c in "uUx":
        return chr(int(e[1:], 16))
    if c in "01234567":
        return chr(int(e, 8))
    return _SIMPLE.get(c, c)


def unescape_bs(t: str) -> str:
    """Peel backslash escapes (repeated while any remain); surrogate pairs are recombined."""
    t = t.strip()
    if len(t) >= 2 and t[0] == t[-1] and t[0] in "\"'":
        t = t[1:-1]
    while BS.search(t):
        n = BS.sub(_bs_one, t)
        try:
            n = n.encode("utf-16", "surrogatepass").decode("utf-16")
        except UnicodeError:
            pass
        if n == t:
            break
        t = n
    return t


def unquote_all(t: str) -> str:
    while re.search(r"%[0-9a-fA-F]{2}", t):
        t = unquote(t)
    return t


def _printable(b: bytes) -> str | None:
    try:
        t = b.decode()
    except UnicodeDecodeError:
        return None
    return t if t and all(c.isprintable() or c in "\r\n\t" for c in t) else None


B32 = re.compile(r"[A-Za-z2-7]+=*")


def decode_b32(t: str) -> str | None:
    """Whole-blob base32 (whitespace stripped); None unless it decodes to printable text."""
    c = re.sub(r"\s+", "", t)
    if len(c) < 8 or not B32.fullmatch(c) or not re.search(r"[A-Z2-7]", c):
        return None
    c = c.upper().rstrip("=")
    try:
        return _printable(base64.b32decode(c + "=" * (-len(c) % 8)))
    except Exception:
        return None


B64U = re.compile(r"[A-Za-z0-9_+/\-]+=*")


def decode_b64(t: str) -> str | None:
    """Whole-blob base64, std or URL-safe alphabet, padded or not (whitespace stripped); printable text or None."""
    c = re.sub(r"\s+", "", t)
    if len(c) < 4 or not B64U.fullmatch(c):
        return None
    c = c.rstrip("=").replace("-", "+").replace("_", "/")
    try:
        return _printable(base64.b64decode(c + "=" * (-len(c) % 4), validate=True))
    except Exception:
        return None


def decode_row(v: str) -> str:
    """Mastery rows: one fragment each, in some encoding per row (autodetected)."""
    v = v.strip()
    if BS.search(v):
        return unescape_bs(v)
    if ENT.search(v):
        return unescape_all(v)
    if re.search(r"%[0-9a-fA-F]{2}", v):
        return unquote_all(v)
    if " " in v or re.fullmatch(r"\d+", v):
        if (t := decode_numbers(v)) is not None:
            return t
    if re.fullmatch(r"(?:[0-9a-fA-F]{2})+", v) and (t := _printable(bytes.fromhex(v))):
        return t
    if re.fullmatch(r"[A-Za-z2-7]+=*", v) and (t := decode_b32(v)):
        return t
    if (t := decode_b64(v)) is not None:
        return t
    return codecs.decode(v, "rot13")


def solve(data: str) -> str:
    if re.match(r"\d+\t", data):  # mastery format
        rows = [l.split("\t", 1)[1] for l in data.splitlines() if "\t" in l]
        data = "".join(decode_row(r) for r in rows)
        m = SLUG.findall(data)
        return m[-1] if m else data.strip()
    text = data
    while True:
        if BS.search(text):
            text = unescape_bs(text)
        elif ENT.search(text):
            text = unescape_all(text)
        elif re.search(r"%[0-9a-fA-F]{2}", text):
            text = unquote_all(text)
        elif (nxt := decode_numbers(text)) is not None:
            text = nxt
        elif (nxt := decode_b64(text)) is not None:
            text = nxt
        elif (nxt := decode_b32(text)) is not None:
            text = nxt
        else:
            break
    lines = [l for l in text.splitlines() if l.strip()]
    last = lines[-1]
    m = SLUG.findall(last)
    return m[-1] if m else last.strip()


if __name__ == "__main__":
    print(solve(open(sys.argv[1]).read()))
