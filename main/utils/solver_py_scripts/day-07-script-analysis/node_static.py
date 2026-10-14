"""Static analyser for Day 7 Node.js operation-graph specimens (`node-*.mjs`). Nothing is executed.

Specimen: `Material_<id>` (string, possibly split/concatenated), `Codec_<id>` label (cross-check only),
`load_<id>()` (representation), `decode_<id>()` (transform; ops read in source order), `recover_<id>()` (which decode
functions are applied, in order).  Output is Day 3's pipe descriptor `v=1|steps=..|adapter=..|...`.
"""
from __future__ import annotations

import base64
import json
import re
from dataclasses import dataclass, field


class NodeUnsupported(Exception):
    pass


@dataclass
class NodeAnalysis:
    descriptor: str
    codec: str = ""
    notes: list = field(default_factory=list)


def _func_body(text: str, prefix: str, is_async: bool = False) -> tuple[str, str] | None:
    m = re.search(r"function\s+(%s_\w+)\s*\(([^)]*)\)\s*\{" % prefix, text)
    if not m:
        return None
    i = m.end()
    depth = 1
    while i < len(text) and depth:
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        i += 1
    return m.group(1), text[m.end():i - 1]


def _material(text: str) -> str:
    m = re.search(r"const\s+Material_\w+\s*=\s*(.*?);\s*\n", text, re.S)
    if not m:
        raise NodeUnsupported("no Material_")
    rhs = m.group(1)
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', rhs)
    if not parts:
        raise NodeUnsupported("Material_ has no string literal")
    if len(parts) == 1 and rhs.strip().startswith('"'):
        return json.loads('"' + parts[0] + '"') if '\\"' in parts[0] and "\\x" not in parts[0] else _unescape(parts[0])
    return "".join(_unescape(p) for p in parts)


def _unescape(s: str) -> str:
    # JS literal `\\x9a` in source is the 4-char text `\x9a` after unescaping `\\`; `\"` is a quote.
    return s.replace('\\"', '"').replace("\\\\", "\\")


def _load(body: str, material: str) -> list[int]:
    if "Buffer.from(" in body and '"base64"' in body:
        return list(base64.b64decode(material + "=" * (-len(material) % 4)))
    if "Buffer.from(" in body and '"hex"' in body:
        return list(bytes.fromhex(material))
    ma = re.search(r"matchAll\(\s*/((?:[^/\\]|\\.)+)/(\w*)\s*\)", body)
    if ma:  # `\\x..`, `%..`, `\\u00..`, `0x..` etc: the regex literal is applied to the material as-is
        radix = re.search(r"parseInt\(\s*\w+\[1\]\s*,\s*(\d+)\s*\)", body)
        base = int(radix.group(1)) if radix else 16
        flags = re.I if "i" in ma.group(2) else 0
        return [int(m.group(1), base) for m in re.finditer(ma.group(1), material, flags)]
    if "JSON.parse" in body:
        data = json.loads(material)
        radix = re.search(r"parseInt\(\s*\w+\s*,\s*(\d+)\s*\)", body)
        base = int(radix.group(1)) if radix else 10
        return [int(v, base) if isinstance(v, str) else int(v) for v in data]
    sp = re.search(r"\.split\(\s*(\"(?:[^\"\\]|\\.)*\"|/[^/]+/)\s*\)", body)
    if sp:
        sep = sp.group(1)
        parts = re.split(sep[1:-1], material) if sep.startswith("/") else material.split(json.loads(sep))
        radix = re.search(r"parseInt\(\s*\w+\s*,\s*(\d+)\s*\)", body)
        base = int(radix.group(1)) if radix else 10
        parts = [p for p in parts if p != ""]
        if "reverse" in body:
            parts = parts[::-1]
        return [int(p, base) for p in parts]
    raise NodeUnsupported(f"load body not understood: {body[:120]}")


_OP = re.compile(
    r"(?P<rev>\.reverse\(\))"
    r"|(?P<rolling>(?P<rv>\w+)\s*=\s*value\b)"
    r"|(?P<keyx>\^\s*\w+\[[^\]]+\]\s*)"
    r"|(?P<xor>\^\s*(?P<k>0x[0-9a-fA-F]+|\d+))"
    r"|(?P<add>\+\s*(?P<a>\d+)\s*\)?\s*(?:&\s*(?:255|0xff)|%\s*256))"
    r"|(?P<sub>-\s*(?P<s>\d+)\s*\+\s*256|\(\s*value\s*-\s*(?P<s2>\d+)\s*\+\s*256\s*\))"
)


def _apply_decode(body: str, data: list[int]) -> list[int]:
    seed = re.search(r"let\s+(\w+)\s*=\s*(0x[0-9a-fA-F]+|\d+)\s*;", body)
    if "const decoded = value ^" in body and seed:  # rolling xor (out[i] = in[i] ^ in[i-1], seed = start)
        prev = int(seed.group(2), 0)
        out = []
        for v in data:
            out.append(v ^ prev)
            prev = v
        return out
    keyarr = re.search(r"(?:const|let)\s+(\w+)\s*=\s*\[([\d,\s x a-fA-F]+)\]\s*;", body)
    out = list(data)
    found = False
    for m in _OP.finditer(body):
        found = True
        if m.group("rev"):
            out = out[::-1]
        elif m.group("keyx"):
            if not keyarr:
                raise NodeUnsupported("key xor without key array")
            key = [int(x, 0) for x in keyarr.group(2).split(",") if x.strip()]
            out = [v ^ key[i % len(key)] for i, v in enumerate(out)]
        elif m.group("xor"):
            k = int(m.group("k"), 0)
            out = [v ^ k for v in out]
        elif m.group("add"):
            k = int(m.group("a"))
            out = [(v + k) & 255 for v in out]
        elif m.group("sub"):
            k = int(m.group("s") or m.group("s2"))
            out = [(v - k) & 255 for v in out]
    if re.search(r"\.map\(|\.reverse\(|\^", body) and not found:
        raise NodeUnsupported(f"decode body not understood: {body[:160]}")
    return out


def analyse_node(text: str) -> NodeAnalysis:
    material = _material(text)
    codec = re.search(r"const\s+Codec_\w+\s*=\s*\"([^\"]*)\"", text)
    load = _func_body(text, "load")
    if not load:
        raise NodeUnsupported("no load_ function")
    data = _load(load[1], material)
    rec = re.search(r"async function recover_\w+\([^)]*\)\s*\{(.*?)\n\}", text, re.S)
    rec_body = rec.group(1) if rec else text
    order = []
    for name in re.findall(r"\bdecode_\w+\b", rec_body):
        if name not in order:
            order.append(name)
    if not order:
        raise NodeUnsupported("recover_ applies no decode_ function")
    for name in order:
        fm = re.search(r"function\s+%s\s*\(([^)]*)\)\s*\{" % re.escape(name), text)
        if not fm:
            raise NodeUnsupported(f"{name} not defined")
        b = _func_body_at(text, fm.end())
        data = _apply_decode(b, data)
    s = bytes(data).decode("latin-1")
    if not s.startswith("v="):
        raise NodeUnsupported(f"decoded descriptor looks wrong: {s[:60]!r}")
    return NodeAnalysis(s, codec.group(1) if codec else "")


def _func_body_at(text: str, i: int) -> str:
    start = i
    depth = 1
    while i < len(text) and depth:
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        i += 1
    return text[start:i - 1]


def is_node_specimen(name: str, data: bytes) -> bool:
    return name.endswith((".mjs", ".cjs", ".js")) and b"Material_" in data and b"HUNTRESS_INSPECT_ONLY" in data and b"Buffer" in data
