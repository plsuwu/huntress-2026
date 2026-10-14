#!/usr/bin/env python3
"""Static analyser for the Day 6 Bash "behavior graph" specimens.

Nothing is executed.  A specimen looks like::

    readonly Material_<id>='<encoded descriptor>'     # '..''..' chunks are concatenated
    readonly Codec_<id>='<representation>|<transform>|<key-hex>'
    Load_<id>()    # text -> int list   (decimal_space/comma/pipe/dash, hex_*, percent_hex, base64_*, reversed_*)
    Decode_<id>()  # int list -> int list (xor / +- modulo 256 / reverse / identity / rolling xor)
    Recover_<id>() # decoy locals + Load + Decode + printf %b loop
    Inspect_<id>() / Plan_<id>()  # field filter, command plan (declare -p ...)

The representation and transform are derived from the *code* of Load_/Decode_ (the `Codec_` label is only
cross-checked and reported through `notes`).
"""
from __future__ import annotations

import ast
import base64
import operator
import re
from dataclasses import dataclass, field


class ShUnsupported(Exception):
    pass


@dataclass
class ShAnalysis:
    descriptor: str = ""
    codec: str = ""
    representation: str = ""
    transform: str = ""
    key: int | None = None
    notes: list[str] = field(default_factory=list)
    plan_commands: dict[str, str] = field(default_factory=dict)


_FUNC_RE = r"^\s*{name}\s*\(\)\s*\{{"


def _func_body(text: str, prefix: str) -> str | None:
    """Body of the function named `<prefix>_<id>() { ... }` (brace matched, strings ignored)."""
    m = re.search(r"^[ \t]*(" + prefix + r"_[0-9a-f]+)\s*\(\)\s*\{", text, re.M)
    if not m:
        return None
    i = m.end()
    depth = 1
    n = len(text)
    while i < n and depth:
        c = text[i]
        if c == "'":
            j = text.find("'", i + 1)
            i = (j if j != -1 else n - 1) + 1
            continue
        if c == "\\":
            i += 2
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        i += 1
    return text[m.end():i - 1]


def _single_quoted_value(text: str, name_prefix: str) -> tuple[str, str]:
    m = re.search(r"^[ \t]*(?:readonly\s+|declare\s+-r\s+)?(" + name_prefix + r"_[0-9a-f]+)=", text, re.M)
    if not m:
        raise ShUnsupported(f"{name_prefix}_<id> assignment not found")
    i = m.end()
    out = []
    while i < len(text) and text[i] in "'\"":
        q = text[i]
        j = text.find(q, i + 1)
        if j == -1:
            raise ShUnsupported("unterminated literal")
        out.append(text[i + 1:j])
        i = j + 1
    if not out:
        raise ShUnsupported(f"{m.group(1)}: not a quoted literal")
    return m.group(1), "".join(out)


# ---------------------------------------------------------------------------
# Load_: representation
# ---------------------------------------------------------------------------

def _load_representation(body: str, material: str) -> tuple[str, list[int]]:
    if "base64 --decode" in body or "base64 -d" in body:
        try:
            raw = base64.b64decode(material.encode(), validate=False)
        except Exception as e:  # noqa: BLE001
            raise ShUnsupported(f"base64: {e}")
        return "base64", list(raw)
    hexmode = "16#" in body
    m_chunk = re.search(r"\$\{\w+:\w+:(\d+)\}", body)
    if m_chunk and hexmode:  # ${raw:i:2} stepping by 2
        n = int(m_chunk.group(1))
        if len(material) % n:
            raise ShUnsupported("hex_compact: material length not a multiple of the chunk")
        return "hex_compact", [int(material[i:i + n], 16) for i in range(0, len(material), n)]
    if hexmode and "//%/" in body:
        parts = [p for p in material.replace("%", " ").split() if p]
        return "percent_hex", [int(p, 16) for p in parts]
    ifs = re.search(r"IFS=(['\"])(.*?)\1", body)
    reversed_ = bool(re.search(r"=\s*\$\{#\w+\[@\]\}\s*-\s*1\s*;\s*\w+\s*>=\s*0", body))
    if ifs:
        sep = ifs.group(2)
        parts = [p for p in material.split(sep) if p != ""] if sep else []
        try:
            vals = [int(p, 16) if hexmode else int(p) for p in parts]
        except ValueError as e:
            raise ShUnsupported(f"bad numeric list: {e}")
        names = {" ": "space", ",": "comma", "|": "pipe", "-": "dash", ":": "colon", ";": "semicolon"}
        kind = "hex" if hexmode else "decimal"
        if reversed_:
            return "reversed_" + kind, vals[::-1]
        return f"{kind}_{names.get(sep, 'sep')}", vals
    raise ShUnsupported("unrecognised Load_ body")


# ---------------------------------------------------------------------------
# Decode_: a tiny interpreter for the per-byte loop (arithmetic expansion only)
# ---------------------------------------------------------------------------

_BINOPS = {
    ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul, ast.Mod: operator.mod,
    ast.FloorDiv: operator.floordiv, ast.BitXor: operator.xor, ast.BitAnd: operator.and_,
    ast.BitOr: operator.or_, ast.LShift: operator.lshift, ast.RShift: operator.rshift,
}


def _to_py(expr: str) -> str:
    e = expr.strip()
    e = re.sub(r"\$\{#(\w+)\[@\]\}", r"LEN_\1", e)
    e = re.sub(r"\$\{(\w+)\[([^\]]+)\]\}", r"\1[\2]", e)
    e = re.sub(r"\$\{(\w+)\}", r"\1", e)
    e = re.sub(r"\$(\w+)", r"\1", e)
    e = re.sub(r"\b(\d+)#([0-9a-fA-F]+)\b", lambda m: str(int(m.group(2), int(m.group(1)))), e)
    e = e.replace("/", "//")
    return e


def _eval(expr: str, env: dict):
    tree = ast.parse(_to_py(expr), mode="eval")

    def ev(n):
        if isinstance(n, ast.Expression):
            return ev(n.body)
        if isinstance(n, ast.Constant) and isinstance(n.value, int):
            return n.value
        if isinstance(n, ast.Name):
            if n.id in env:
                v = env[n.id]
                return int(v) if not isinstance(v, (list, dict)) else v
            if n.id.startswith("LEN_") and n.id[4:] in env:
                return len(env[n.id[4:]])
            return 0  # unset bash variable in arithmetic
        if isinstance(n, ast.Subscript):
            arr = ev(n.value)
            idx = ev(n.slice)
            if isinstance(arr, dict):
                return arr.get(idx, 0)
            return arr[idx]
        if isinstance(n, ast.BinOp) and type(n.op) in _BINOPS:
            return _BINOPS[type(n.op)](ev(n.left), ev(n.right))
        if isinstance(n, ast.UnaryOp) and isinstance(n.op, ast.USub):
            return -ev(n.operand)
        if isinstance(n, ast.UnaryOp) and isinstance(n.op, ast.UAdd):
            return ev(n.operand)
        raise ShUnsupported(f"arithmetic node {type(n).__name__} in {expr!r}")

    return ev(tree)


def _match_bracket(text: str, open_idx: int) -> int | None:
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == "[":
            depth += 1
        elif text[i] == "]":
            depth -= 1
            if depth == 0:
                return i
    return None


def _value_expr(rhs: str) -> str:
    """RHS of an assignment -> arithmetic expression text: `"$((E))"` -> E, `"${a[i]}"`/`$x`/`12` -> itself."""
    r = rhs.strip()
    ex = _strip_dollar_paren(r)
    if ex is not None:
        return ex
    if len(r) >= 2 and r[0] == r[-1] and r[0] in "\"'":
        r = r[1:-1]
    return r


def _strip_dollar_paren(rhs: str) -> str | None:
    m = re.fullmatch(r'"?\$\(\((.*)\)\)"?', rhs.strip())
    return m.group(1) if m else None


def _decode_data(body: str, load_names: tuple[str, str], data: list[int], passes: int) -> list[int]:
    """Interpret Decode_<id>: input array (`load_names[0]`) -> output array (`load_names[1]`)."""
    in_name, out_name = load_names
    lines = [l.strip() for l in body.splitlines()]
    lines = [l for l in lines if l and not l.startswith("#")]
    out: dict[int, int] | list[int] = []
    for _ in range(passes):
        env: dict = {in_name: list(data), out_name: {}}
        appended: list[int] = []
        # key/constant arrays and scalars declared before the loop
        i = 0
        loop_body: list[str] = []
        have_loop = False
        while i < len(lines):
            l = lines[i]
            m_for = re.match(r"for\s*\(\(\s*(\w+)\s*=\s*([^;]+);\s*([^;]+);\s*([^)]+)\)\)\s*;?\s*do\s*$", l)
            if m_for:
                var, init, cond, step = m_for.groups()
                j = i + 1
                depth = 1
                while j < len(lines):
                    if re.match(r"for\b|while\b|until\b", lines[j]) and lines[j].rstrip().endswith("do"):
                        depth += 1
                    if lines[j] == "done":
                        depth -= 1
                        if depth == 0:
                            break
                    j += 1
                loop_body = lines[i + 1:j]
                env[var] = _eval(init, env)
                cm = re.fullmatch(r"(\w+)\s*(<|<=|>|>=|!=)\s*(.+)", cond.strip())
                if not cm:
                    raise ShUnsupported(f"loop condition {cond!r}")
                guard = 0
                while True:
                    lhs = env.get(cm.group(1), 0)
                    rhs = _eval(cm.group(3), env)
                    ok = {"<": lhs < rhs, "<=": lhs <= rhs, ">": lhs > rhs, ">=": lhs >= rhs, "!=": lhs != rhs}[cm.group(2)]
                    if not ok:
                        break
                    _run_lines(loop_body, env, out_name, appended)
                    sm = re.fullmatch(r"(\w+)(\+\+|--)", step.strip())
                    if sm:
                        env[sm.group(1)] += 1 if sm.group(2) == "++" else -1
                    else:
                        am = re.fullmatch(r"(\w+)\s*(\+=|-=)\s*(.+)", step.strip())
                        if not am:
                            raise ShUnsupported(f"loop step {step!r}")
                        v = _eval(am.group(3), env)
                        env[am.group(1)] += v if am.group(2) == "+=" else -v
                    guard += 1
                    if guard > 100000:
                        raise ShUnsupported("loop runaway")
                have_loop = True
                i = j + 1
                continue
            _run_lines([l], env, out_name, appended)
            i += 1
        if not have_loop:
            raise ShUnsupported("no per-byte loop in Decode_")
        outd = env[out_name]
        if appended and not outd:
            data = appended
        else:
            if appended:
                raise ShUnsupported("mixed indexed and appended output")
            data = [outd[k] for k in sorted(outd)]
    return data


def _run_lines(lines: list[str], env: dict, out_name: str, appended: list[int]) -> None:
    for l in lines:
        l = l.rstrip(";").strip()
        if not l or l in ("do", "done", ":") or l.startswith(("local ", "declare ")) and "=" not in l:
            continue
        m = re.fullmatch(r"local\s+-a\s+(\w+)=\(([^)]*)\)", l)
        if m:
            env[m.group(1)] = [int(x) for x in m.group(2).split()]
            continue
        m = re.fullmatch(r"(?:local\s+)?(\w+)=\(([^)]*)\)", l)
        if m and not m.group(2).strip() and m.group(1) in (out_name,):
            continue  # `d=()` initialiser of the output array
        if m and all(re.fullmatch(r"\d+", x) for x in m.group(2).split()):
            env[m.group(1)] = [int(x) for x in m.group(2).split()]
            continue
        m = re.fullmatch(r"(\w+)\+=\(\s*(.+?)\s*\)", l)
        if m and m.group(1) == out_name:
            appended.append(_eval(_value_expr(m.group(2)), env))
            continue
        m = re.match(r"(\w+)\[", l)
        if m and m.group(1) == out_name:
            j = _match_bracket(l, m.end() - 1)
            if j is not None and l[j + 1:j + 2] == "=":
                idx = l[m.end():j]
                env[out_name][_eval(idx, env)] = _eval(_value_expr(l[j + 2:]), env)
                continue
        m = re.fullmatch(r"\(\(\s*(\w+)\s*(\+=|-=|=)\s*(.+?)\s*\)\)", l)
        if m:
            v = _eval(m.group(3), env)
            cur = env.get(m.group(1), 0)
            env[m.group(1)] = v if m.group(2) == "=" else (cur + v if m.group(2) == "+=" else cur - v)
            continue
        m = re.fullmatch(r"(?:local\s+)?(\w+)=(.+)", l)
        if m:
            env[m.group(1)] = _eval(_value_expr(m.group(2)), env)
            continue
        raise ShUnsupported(f"Decode_ statement not understood: {l!r}")


def _array_names(load_body: str, decode_body: str) -> tuple[str, str]:
    m_in = re.search(r"\b(\w+)\+?=\(\)", load_body) or re.search(r"(\w+)=\(\)", load_body)
    m_out = re.search(r"\b(\w+)=\(\)", decode_body)
    if not m_in or not m_out:
        raise ShUnsupported("cannot find input/output arrays")
    # the decoder reads the Load_ array and fills its own: find the one it indexes on the right-hand side
    return m_in.group(1), m_out.group(1)


def _classify_transform(body: str) -> str:
    """Descriptive label only (the data comes from the interpreter)."""
    rev = bool(re.search(r"\[\s*\$\{#\w+\[@\]\}\s*-\s*1\s*-\s*\w+\s*\]", body))
    keyed = bool(re.search(r"%\s*\$\{#\w+\[@\]\}", body))
    if re.search(r"\^", body):
        if keyed:
            return "repeating_key_xor"
        if re.search(r"prev", body, re.I):
            return "rolling_xor"
        return "reverse_then_xor" if rev else "single_byte_xor"
    if re.search(r"-\s*\d+\s*\+\s*256", body):
        return "reverse_then_add_modulo_256" if rev else "add_modulo_256"
    if re.search(r"\+\s*\d+\s*\)\s*%\s*256", body):
        return "reverse_then_subtract_modulo_256" if rev else "subtract_modulo_256"
    return "reverse" if rev else "identity"


def _decode_passes(recover_body: str | None) -> int:
    if not recover_body:
        return 1
    m = re.search(r"for\s+\w+\s+in\s+([\d\s]+);\s*do\s+Decode_", recover_body)
    if m:
        return len(m.group(1).split())
    m = re.search(r"while\s*\(\(\s*\w+\s*<\s*(\d+)\s*\)\)\s*;\s*do\s+Decode_", recover_body)
    if m:
        return int(m.group(1))
    return 1


def analyse_sh(text: str) -> ShAnalysis:
    a = ShAnalysis()
    _, material = _single_quoted_value(text, "Material")
    _, codec = _single_quoted_value(text, "Codec")
    a.codec = codec
    load = _func_body(text, "Load")
    decode = _func_body(text, "Decode")
    if load is None or decode is None:
        raise ShUnsupported("Load_/Decode_ function not found")
    rep, data = _load_representation(load, material)
    names = _array_names(load, decode)
    passes = _decode_passes(_func_body(text, "Recover"))
    out = _decode_data(decode, names, data, passes)
    fam = _classify_transform(decode)
    a.representation, a.transform = rep, fam
    km = re.search(r"\^\s*(\d+)", decode)
    a.key = int(km.group(1)) if km else None
    if any(not 0 <= b <= 255 for b in out):
        raise ShUnsupported("decoded value outside 0..255")
    try:
        a.descriptor = bytes(out).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as e:
        raise ShUnsupported(f"descriptor not utf-8: {e}")
    label = codec.split("|")
    lab0 = "base64" if label and label[0].startswith("base64") else (label[0] if label else "")
    if len(label) >= 2 and (lab0 != rep or label[1] != fam):
        a.notes.append(f"Codec label {label[:2]} != code-derived ({rep}, {fam})")
    if "v=" not in a.descriptor or "steps=" not in a.descriptor:
        a.notes.append("descriptor lacks v=/steps= (decode suspect)")
    return a


def is_sh_specimen(name: str, data: bytes) -> bool:
    head = data[:3000].decode("utf-8", "replace")
    return bool(re.search(r"^\s*readonly\s+Material_[0-9a-f]+=", head + data.decode("utf-8", "replace"), re.M)) and \
        bool(re.search(r"^\s*Recover_[0-9a-f]+\s*\(\)", data.decode("utf-8", "replace"), re.M))
