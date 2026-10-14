#!/usr/bin/env python3
"""Day 12 warmups: Back at You (whole-text string reversal layer; also layered under other codecs).

Superset of day-13. Day 13 docs follow:
Day 13 warmups: Opposite Day (Atbash layer: a<->z, b<->y ...; non-letters untouched; also layered under other codecs).

Superset of day-11. Day 11 docs follow:
Day 11 warmups: Safe Travels (base64url layer: `-`/`_` alphabet, padded or not; also std base64).

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


_ATBASH = str.maketrans(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA",
)
_WORDS = re.compile(r"\b(?:the|this|to|and|answer|submit|exact|game|master|decod\w*|warmup|solve|work|nice)\b", re.I)


def atbash(t: str) -> str:
    return t.translate(_ATBASH)


def _englishness(t: str) -> int:
    return len(_WORDS.findall(t)) + 3 * len(SLUG.findall(t))


def decode_atbash(t: str) -> str | None:
    """Atbash is its own inverse, so only apply when the result is clearly more English than the input."""
    if not re.search(r"[A-Za-z]{3}", t):
        return None
    a = atbash(t)
    return a if _englishness(a) > _englishness(t) else None


def decode_reversed(t: str) -> str | None:
    """Reversal is its own inverse: only apply when the reversed text is clearly more English than the input."""
    if not re.search(r"[A-Za-z]{3}", t):
        return None
    r = t.strip()[::-1]
    return r if _englishness(r) > _englishness(t) else None


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
    if (t := decode_atbash(v)) is not None:
        return t
    return codecs.decode(v, "rot13")


_TEMPLATE = re.compile(r"Nice work decoding this warmup! To solve, submit this exact answer to the Game Master: ([a-z]+-[a-z]+-[a-z]+-\d{2})\s*$")
_LETTERY = re.compile(r"[A-Za-z0-9 \-]+")
_TRANSFORMS = (
    lambda v: v,
    lambda v: codecs.decode(v, "rot13"),
    atbash,
    lambda v: v[::-1],
)


def _structured(v: str) -> str | None:
    """Row fragments with an unambiguous codec signature (None for plain-letter / reversed / rot13 / atbash rows)."""
    if BS.search(v) or ENT.search(v) or re.search(r"%[0-9a-fA-F]{2}", v):
        return decode_row(v)
    if re.fullmatch(r"[\d ]+", v) and (" " in v or len(v) % 2):
        return decode_numbers(v)  # space-separated (or odd-length) digits; even all-digit runs may be hex pairs
    if re.fullmatch(r"(?:[0-9a-fA-F]{2})+", v) and (t := _printable(bytes.fromhex(v))):
        return t
    if re.fullmatch(r"[A-Z2-7]+=*", v) and len(v) % 8 == 0 and (t := decode_b32(v)):
        return t
    if re.search(r"[A-Z0-9=+/_]", v) and re.fullmatch(r"[A-Za-z0-9+/_=]+", v) and (t := decode_b64(v)):
        return t
    return None


def _dash_variants(text: str):
    """Row 12 style padding: `-` runs of 2+ are pad (encoded row is `reverse(frag)` + dashes, right-padded to a fixed
    width) and a genuine slug hyphen at the fragment edge is at most 1 dash. Yield text with each run -> 0 or 1 dash."""
    import itertools
    runs = list(re.finditer(r"-{2,}", text))
    if not runs:
        return
    if len(runs) > 10:
        runs = runs[:10]
    for keep in itertools.product((0, 1), repeat=len(runs)):
        out, pos = [], 0
        for m, k in zip(runs, keep):
            out.append(text[pos:m.start()])
            out.append("-" * k)
            pos = m.end()
        out.append(text[pos:])
        yield "".join(out)


def _match_template(text: str) -> str | None:
    if m := _TEMPLATE.search(text):
        return m.group(1)
    hits = {m.group(1) for v in _dash_variants(text) if (m := _TEMPLATE.search(v))}
    return hits.pop() if len(hits) == 1 else None


def solve_rows(data: str) -> str:
    """Mastery: rows `NN<TAB>fragment`. Letter-only rows are rot13 / atbash / reversed / raw (fixed per row index in
    observed variants, but not assumed): enumerate the combinations and keep the one matching the message template.
    Free rows keep leading/trailing spaces (a fragment may start with a space!); dash padding is resolved by
    `_match_template`."""
    import itertools
    raw = [l.rstrip("\r").split("\t", 1) for l in data.split("\n") if "\t" in l]
    nums = [int(a) for a, _ in raw]
    rows = [b for _, b in raw]
    fixed, free = [], []
    for i, v in enumerate(rows):
        t = _structured(v.strip())
        fixed.append(t)
        if t is None:
            free.append(i)
    # Observed (day 13): row 02 = rot13, row 12 = reversed, row 13 = atbash. Day 12 (58 sets): 02 rot13, 12 reversed+pad.
    pref = {2: 1, 12: 3, 13: 2}
    parts = list(fixed)
    for i in free:
        parts[i] = _TRANSFORMS[pref.get(nums[i], 0)](rows[i])
    if (a := _match_template("".join(parts))) is not None:
        return a
    hits = []
    # cap: 4^k combos; beyond ~9 free rows the enumeration would blow the (30 s) mastery clock
    if len(free) <= 9:
        for combo in itertools.product(range(len(_TRANSFORMS)), repeat=len(free)):
            parts = list(fixed)
            for i, k in zip(free, combo):
                parts[i] = _TRANSFORMS[k](rows[i])
            if (a := _match_template("".join(parts))) is not None:
                hits.append(a)
    if hits:
        return hits[0]
    data = "".join(decode_row(r.strip()) for r in rows)  # fallback: old behaviour
    m = SLUG.findall(data)
    return m[-1] if m else data.strip()


def solve(data: str) -> str:
    if re.match(r"\d+\t", data):  # mastery format
        return solve_rows(data)
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
        elif (nxt := decode_atbash(text)) is not None:
            text = nxt
        elif (nxt := decode_reversed(text)) is not None:
            text = nxt
        else:
            break
    lines = [l for l in text.splitlines() if l.strip()]
    last = lines[-1]
    m = SLUG.findall(last)
    return m[-1] if m else last.strip()


if __name__ == "__main__":
    print(solve(open(sys.argv[1]).read()))
