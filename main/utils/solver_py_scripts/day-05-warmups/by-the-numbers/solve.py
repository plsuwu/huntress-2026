#!/usr/bin/env python3
"""Day 5 warmups: By the Numbers.

Blob is whitespace-separated decimal ASCII codes. Decoding is iterated while the
output still looks like a numeric list (handles nested layers / other bases),
then the last non-empty line is scanned for the answer slug.

usage: solve.py <blob.txt>
"""
import base64
import codecs
import re
import sys

SLUG = re.compile(r"\b[a-z]+-[a-z]+-[a-z]+-\d{2}\b")


def decode_numbers(text: str) -> str | None:
    toks = re.split(r"[\s,;]+", text.strip())
    if not toks or not all(toks):
        return None
    for base, pat in ((10, r"\d+"), (16, r"[0-9a-fA-F]{2}"), (8, r"[0-7]+"), (2, r"[01]+")):
        if all(re.fullmatch(pat, t) for t in toks):
            try:
                vals = [int(t, base) for t in toks]
            except ValueError:
                continue
            if all(0 < v < 0x110000 for v in vals) and all(
                chr(v).isprintable() or chr(v) in "\r\n\t" for v in vals
            ):
                return "".join(map(chr, vals))
    return None


def _printable(b: bytes) -> str | None:
    try:
        t = b.decode()
    except UnicodeDecodeError:
        return None
    return t if t and all(c.isprintable() or c in "\r\n\t" for c in t) else None


def decode_row(v: str) -> str:
    """Mastery rows: one fragment each, in a different encoding per row."""
    v = v.strip()
    if re.fullmatch(r"[01]+( [01]+)*", v) and " " in v or re.fullmatch(r"[01]{8}", v):
        return "".join(chr(int(x, 2)) for x in v.split())
    if re.fullmatch(r"\d+( \d+)*", v):
        return "".join(chr(int(x)) for x in v.split())
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
    # mastery format: "NN<TAB>encoded fragment" per line; concatenate decoded fragments in order
    if re.match(r"\d+\t", data):
        rows = [l.split("\t", 1)[1] for l in data.splitlines() if "\t" in l]
        data = "".join(decode_row(r) for r in rows)
        m = SLUG.findall(data)
        return m[-1] if m else data.strip()
    text = data
    while (nxt := decode_numbers(text)) is not None:
        text = nxt
    lines = [l for l in text.splitlines() if l.strip()]
    last = lines[-1]
    m = SLUG.findall(last)
    return m[-1] if m else last.strip()


if __name__ == "__main__":
    print(solve(open(sys.argv[1]).read()))
