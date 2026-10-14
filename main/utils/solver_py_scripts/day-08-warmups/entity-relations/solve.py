#!/usr/bin/env python3
"""Day 8 warmups: Entity Relations (HTML entities `&#78;` / `&#x4e;` / `&amp;`, possibly layered).

Superset of day-07 percent-chance: adds html.unescape (repeated while entities remain) to
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


def decode_row(v: str) -> str:
    """Mastery rows: one fragment each, in some encoding per row (autodetected)."""
    v = v.strip()
    if ENT.search(v):
        return unescape_all(v)
    if re.search(r"%[0-9a-fA-F]{2}", v):
        return unquote_all(v)
    if " " in v or re.fullmatch(r"\d+", v):
        if (t := decode_numbers(v)) is not None:
            return t
    if re.fullmatch(r"(?:[0-9a-fA-F]{2})+", v) and (t := _printable(bytes.fromhex(v))):
        return t
    if re.fullmatch(r"[A-Za-z0-9+/]+=*", v):
        try:
            if t := _printable(base64.b64decode(v + "=" * (-len(v) % 4), validate=True)):
                return t
        except Exception:
            pass
    return codecs.decode(v, "rot13")


def solve(data: str) -> str:
    if re.match(r"\d+\t", data):  # mastery format
        rows = [l.split("\t", 1)[1] for l in data.splitlines() if "\t" in l]
        data = "".join(decode_row(r) for r in rows)
        m = SLUG.findall(data)
        return m[-1] if m else data.strip()
    text = data
    while True:
        if ENT.search(text):
            text = unescape_all(text)
        elif re.search(r"%[0-9a-fA-F]{2}", text):
            text = unquote_all(text)
        elif (nxt := decode_numbers(text)) is not None:
            text = nxt
        else:
            break
    lines = [l for l in text.splitlines() if l.strip()]
    last = lines[-1]
    m = SLUG.findall(last)
    return m[-1] if m else last.strip()


if __name__ == "__main__":
    print(solve(open(sys.argv[1]).read()))
