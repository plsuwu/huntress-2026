"""Static VBScript helper for the Day 4 "Dim and Dimmer" specimens.

Nothing is executed through WSH.  The specimens are shaped as

    Const Material_<id> = "..."            ' encoded descriptor
    Const Codec_<id>    = "<repr>|<family>|<key>"
    Function Load_<id>() / Decode_<id>(x)  ' small, line-oriented byte loops
    Function Recover_<id>()                ' wrapper + Decode call + Chr loop
    Function Inspect_<id>()

`Load_` and `Decode_` are evaluated with a tiny interpreter for the VBScript
subset they use (Dim/ReDim, assignment, indexed assignment, For/Next, a handful
of builtins, + - * \\ Mod Xor & and parentheses).  The byte-transform *family*
is read off the decoder's behaviour (probe vectors), not off its name or the
`Codec_` label; the label is only a cross-check.  The control-flow wrapper is
read lexically off the lines that enclose the `Decode_<id>(` call in
`Recover_<id>`.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field

MAX_STEPS = 2_000_000


class VbsUnsupported(Exception):
    pass


# ---------------------------------------------------------------------------
# expression evaluation
# ---------------------------------------------------------------------------

_TOKEN = re.compile(
    r'\s*(?:(?P<str>"(?:[^"]|"")*")|(?P<hex>&H[0-9A-Fa-f]+&?)|(?P<num>\d+)|(?P<id>[A-Za-z_]\w*)'
    r'|(?P<op><>|<=|>=|[-+*\\/&()=<>,]))'
)

# binary precedence, low -> high (VBScript)
_BIN = [
    ["Xor"], ["Or"], ["And"], ["=", "<>", "<", ">", "<=", ">="], ["&"], ["+", "-"], ["Mod"], ["\\"], ["*", "/"],
]


def _tokenize(src: str) -> list[tuple[str, str]]:
    out, pos = [], 0
    src = src.rstrip()
    while pos < len(src):
        m = _TOKEN.match(src, pos)
        if not m or m.end() == pos:
            raise VbsUnsupported(f"cannot tokenize {src[pos:pos + 30]!r}")
        pos = m.end()
        for kind in ("str", "hex", "num", "id", "op"):
            if m.group(kind) is not None:
                out.append((kind, m.group(kind)))
                break
    return out


class Interp:
    """One function's local scope plus the file's constants."""

    def __init__(self, consts: dict[str, object], helpers: dict | None = None):
        self.consts = consts
        self.helpers = {k.lower(): v for k, v in (helpers or {}).items()}
        self.vars: dict[str, object] = {}
        self.steps = 0

    # -- name lookup is case-insensitive, as in VBScript -------------------
    def _key(self, name: str) -> str:
        return name.lower()

    def get(self, name: str):
        k = self._key(name)
        if k in self.vars:
            return self.vars[k]
        for cname, val in self.consts.items():
            if cname.lower() == k:
                return val
        if k in ("true", "false", "empty"):
            return {"true": True, "false": False, "empty": 0}[k]
        raise VbsUnsupported(f"unknown name {name}")

    def set(self, name: str, value) -> None:
        self.vars[self._key(name)] = value

    # -- expressions --------------------------------------------------------
    def eval(self, src: str):
        self.toks = _tokenize(src)
        self.i = 0
        v = self._expr(0)
        if self.i != len(self.toks):
            raise VbsUnsupported(f"trailing tokens in {src!r}")
        return v

    def _peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def _expr(self, level: int):
        if level == len(_BIN):
            return self._unary()
        left = self._expr(level + 1)
        while True:
            kind, val = self._peek()
            hit = None
            if kind in ("op", "id"):
                for cand in _BIN[level]:
                    if (kind == "id" and val.lower() == cand.lower()) or (kind == "op" and val == cand):
                        hit = cand
            if hit is None:
                return left
            self.i += 1
            right = self._expr(level + 1)
            left = self._binop(hit, left, right)

    @staticmethod
    def _binop(op: str, a, b):
        if op == "&":
            return f"{a}{b}"
        if op == "+":
            return a + b
        if op == "-":
            return a - b
        if op == "*":
            return a * b
        if op == "/":
            return a / b
        if op == "\\":
            return int(a) // int(b)
        if op == "Mod":
            return int(a) % int(b)
        if op == "Xor":
            return int(a) ^ int(b)
        if op == "And":
            return int(a) & int(b)
        if op == "Or":
            return int(a) | int(b)
        if op == "=":
            return a == b
        if op == "<>":
            return a != b
        if op == "<":
            return a < b
        if op == ">":
            return a > b
        if op == "<=":
            return a <= b
        if op == ">=":
            return a >= b
        raise VbsUnsupported(op)

    def _unary(self):
        kind, val = self._peek()
        if kind == "op" and val == "-":
            self.i += 1
            return -self._unary()
        return self._primary()

    def _args(self) -> list:
        # at '(' ; returns evaluated args, consumes ')'
        assert self._peek() == ("op", "(")
        self.i += 1
        args = []
        if self._peek() == ("op", ")"):
            self.i += 1
            return args
        while True:
            args.append(self._expr(0))
            kind, val = self._peek()
            self.i += 1
            if (kind, val) == ("op", ")"):
                return args
            if (kind, val) != ("op", ","):
                raise VbsUnsupported("bad argument list")

    def _primary(self):
        kind, val = self._peek()
        self.i += 1
        if kind == "num":
            return int(val)
        if kind == "hex":
            return int(val[2:].rstrip("&"), 16)
        if kind == "str":
            return val[1:-1].replace('""', '"')
        if (kind, val) == ("op", "("):
            v = self._expr(0)
            if self._peek() != ("op", ")"):
                raise VbsUnsupported("missing )")
            self.i += 1
            return v
        if kind == "id":
            low = val.lower()
            if self._peek() == ("op", "("):
                args = self._args()
                return self._call(low, val, args)
            return self.get(val)
        raise VbsUnsupported(f"unexpected token {val!r}")

    def _call(self, low: str, name: str, a: list):
        if low in self.helpers:
            return self.helpers[low](*a)
        if low == "ubound":
            return len(a[0]) - 1
        if low == "len":
            return len(str(a[0]))
        if low == "mid":
            s, start = str(a[0]), int(a[1])
            return s[start - 1:start - 1 + int(a[2])] if len(a) > 2 else s[start - 1:]
        if low == "cint" or low == "clng" or low == "cbyte":
            return int(a[0]) if not isinstance(a[0], str) else self._str_to_int(a[0])
        if low == "chr":
            return chr(int(a[0]))
        if low == "asc":
            return ord(str(a[0])[0])
        if low == "lcase":
            return str(a[0]).lower()
        if low == "ucase":
            return str(a[0]).upper()
        if low == "array":
            return list(a)
        if low == "split":
            return str(a[0]).split(str(a[1]))
        if low == "join":
            return str(a[1] if len(a) > 1 else " ").join(map(str, a[0]))
        # array element read
        k = self._key(name)
        if k in self.vars and isinstance(self.vars[k], list):
            return self.vars[k][int(a[0])]
        raise VbsUnsupported(f"unknown function {name}")

    @staticmethod
    def _str_to_int(s: str) -> int:
        s = s.strip()
        if s.upper().startswith("&H"):
            return int(s[2:], 16)
        return int(s)

    # -- statements ---------------------------------------------------------
    def run(self, lines: list[str]) -> None:
        self._block(lines, 0, len(lines))

    def _block(self, lines: list[str], lo: int, hi: int) -> None:
        i = lo
        while i < hi:
            self.steps += 1
            if self.steps > MAX_STEPS:
                raise VbsUnsupported("step budget exceeded")
            line = strip_comment(lines[i]).strip()
            if not line:
                i += 1
                continue
            low = line.lower()
            if low.startswith("dim "):
                for part in re.split(r",\s*", line[4:]):
                    nm = re.match(r"\s*([A-Za-z_]\w*)\s*(\(\s*\))?", part)
                    if nm and nm.group(1).lower() not in self.vars:
                        self.vars[nm.group(1).lower()] = [] if nm.group(2) else 0
                i += 1
            elif low.startswith("redim "):
                m = re.match(r"redim\s+(?:preserve\s+)?(\w+)\s*\((.*)\)\s*$", line, re.I)
                if not m:
                    raise VbsUnsupported(line)
                self.set(m.group(1), [0] * (int(self.eval(m.group(2))) + 1))
                i += 1
            elif low.startswith("for "):
                m = re.match(r"for\s+(\w+)\s*=\s*(.+?)\s+to\s+(.+?)(?:\s+step\s+(.+))?$", line, re.I)
                if not m:
                    raise VbsUnsupported(line)
                end = matching(lines, i, hi, "for", "next")
                lo_v, hi_v = int(self.eval(m.group(2))), int(self.eval(m.group(3)))
                step = int(self.eval(m.group(4))) if m.group(4) else 1
                v = lo_v
                while (v <= hi_v) if step > 0 else (v >= hi_v):
                    self.set(m.group(1), v)
                    self._block(lines, i + 1, end)
                    v += step
                self.set(m.group(1), v)
                i = end + 1
            elif low.startswith("while "):
                end = matching(lines, i, hi, "while", "wend")
                while self.eval(line[6:].strip()):
                    self._block(lines, i + 1, end)
                i = end + 1
            elif low.startswith(("function ", "sub ", "select case", "if ", "do", "on error", "call ")):
                raise VbsUnsupported(f"unsupported statement in decoder: {line}")
            else:
                m = re.match(r"(\w+)\s*(?:\((.*)\))?\s*=\s*(.+)$", line)
                if not m:
                    raise VbsUnsupported(line)
                val = self.eval(m.group(3))
                if m.group(2) is not None:
                    arr = self.get(m.group(1))
                    arr[int(self.eval(m.group(2)))] = val
                else:
                    self.set(m.group(1), val)
                i += 1


def strip_comment(line: str) -> str:
    out, in_str = [], False
    for ch in line:
        if ch == '"':
            in_str = not in_str
        if ch == "'" and not in_str:
            break
        out.append(ch)
    return "".join(out)


def matching(lines: list[str], start: int, hi: int, opener: str, closer: str) -> int:
    depth = 0
    for j in range(start, hi):
        t = strip_comment(lines[j]).strip().lower()
        if re.match(rf"{opener}\b", t):
            depth += 1
        elif re.match(rf"{closer}\b", t):
            depth -= 1
            if depth == 0:
                return j
    raise VbsUnsupported(f"unterminated {opener}")


# ---------------------------------------------------------------------------
# specimen analysis
# ---------------------------------------------------------------------------


@dataclass
class VbsAnalysis:
    suffix: str
    descriptor: str
    representation: str  # derived from Load_
    representation_label: str
    transform: str  # derived from Decode_ behaviour
    transform_label: str
    key_label: str
    control: str
    wrappers: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


def _functions(text: str) -> dict[str, list[str]]:
    funcs: dict[str, list[str]] = {}
    cur = None
    for raw in text.splitlines():
        t = strip_comment(raw).strip()
        m = re.match(r"(?:Function|Sub)\s+(\w+)", t, re.I)
        if m and cur is None:
            cur = m.group(1)
            funcs[cur] = []
            continue
        if cur and re.match(r"End\s+(?:Function|Sub)\b", t, re.I):
            cur = None
            continue
        if cur is not None:
            funcs[cur].append(raw)
    return funcs


def _consts(text: str) -> dict[str, str]:
    out = {}
    for m in re.finditer(r'^\s*Const\s+(\w+)\s*=\s*"((?:[^"]|"")*)"', text, re.M | re.I):
        out[m.group(1)] = m.group(2).replace('""', '"')
    return out


def _params(text: str, fname: str) -> list[str]:
    m = re.search(rf"^\s*(?:Function|Sub)\s+{re.escape(fname)}\s*\(([^)]*)\)", text, re.M | re.I)
    if not m:
        return []
    return [re.sub(r"^(?:ByRef|ByVal)\s+", "", p.strip(), flags=re.I) for p in m.group(1).split(",") if p.strip()]


def _b64_helper(alphabet: str):
    def decode(text) -> list[int]:
        out, acc, bits = [], 0, 0
        for ch in str(text).replace("\r", "").replace("\n", ""):
            if ch == "=":
                continue
            v = alphabet.find(ch)
            if v < 0:
                raise VbsUnsupported("invalid base64 character")
            acc, bits = acc * 64 + v, bits + 6
            if bits >= 8:
                bits -= 8
                out.append((acc >> bits) & 255)
                acc &= (1 << bits) - 1
        return out
    return decode


def helpers_for(text: str, funcs: dict[str, list[str]]) -> dict:
    """In-file Base64_<id> helpers: only the alphabet literal is read, the loop
    is the standard 6-bit accumulator (checked by the presence of the shape)."""
    out = {}
    for fname, body in funcs.items():
        if not re.match(r"base64_", fname, re.I):
            continue
        src = "\n".join(strip_comment(l) for l in body)
        m = re.search(r'alphabet\s*=\s*"([^"]{64})"', src, re.I)
        if not m or not re.search(r"\*\s*64\b", src) or not re.search(r"bitCount|bits", src, re.I):
            raise VbsUnsupported(f"unrecognised base64 helper {fname}")
        out[fname] = _b64_helper(m.group(1))
    return out


def run_function(text: str, fname: str, args: list, consts: dict, funcs: dict) -> list:
    it = Interp(consts, helpers_for(text, funcs))
    for p, a in zip(_params(text, fname), args):
        it.set(p, a)
    it.set(fname, 0)
    it.run(funcs[fname])
    return it.get(fname)


def _family(decode, decoder_sign: str = "+") -> tuple[str, str]:
    """Behavioural family of a decoder callable (list[int] -> list[int]).

    Returns (family named for the *encoder*, key text) - the decoder of an
    `add_modulo_256` encoding subtracts, so a decoder that adds N is reported as
    `subtract_modulo_256`, matching the artifact's own labels."""
    n = 16
    zero = decode([0] * n)
    ramp = decode(list(range(1, n + 1)))
    ident = list(range(1, n + 1))
    if ramp == ident:
        return "identity", ""
    if ramp == ident[::-1]:
        return "reverse_bytes", ""
    d = [(r - i) % 256 for r, i in zip(ramp, ident)]
    if len(set(d)) == 1:
        # A constant offset mod 256 is "+k" and "-(256-k)" at once; the decoder's own
        # operator decides.  Decoder "+k" => encoder subtracted k => subtract_modulo_256.
        k = d[0]
        if decoder_sign == "-":
            return "add_modulo_256", f"{(256 - k) % 256:02x}"
        return "subtract_modulo_256", f"{k:02x}"
    x = [r ^ i for r, i in zip(ramp, ident)]
    if len(set(x)) == 1:
        return "single_byte_xor", f"{x[0]:02x}"
    for m in range(2, 9):
        if all(x[j] == x[j % m] for j in range(n)) and len(set(x[:m])) > 1:
            key = decode_key = [zero[j] for j in range(m)]
            return "repeating_key_xor", ",".join(f"{b:02x}" for b in key)
    # rolling_xor: out[i] = in[i] ^ in[i-1], out[0] = in[0] ^ seed
    if all(ramp[j] == ident[j] ^ ident[j - 1] for j in range(1, n)):
        return "rolling_xor", f"{ramp[0] ^ ident[0]:02x}"
    # reverse_then_xor: reverse, then single-byte xor
    rev = ident[::-1]
    x = [r ^ i for r, i in zip(ramp, rev)]
    if len(set(x)) == 1:
        return "reverse_then_xor", f"{x[0]:02x}"
    return "unknown", ""


def _representation(load_lines: list[str]) -> str:
    body = "\n".join(strip_comment(l) for l in load_lines)
    reverse = bool(re.search(r"result\s*\(\s*UBound\s*\(\s*\w+\s*\)\s*-\s*\w+\s*\)\s*=", body, re.I))
    split = re.search(r'Split\s*\(\s*\w+\s*,\s*"(.)"\s*\)', body, re.I)
    hexed = "&H" in body
    if re.search(r"Base64_\w+\s*\(", body, re.I):
        return "base64_chunks" if re.search(r"Mid\s*\(.*&.*Mid\s*\(", body, re.I) else "base64"
    if split and hexed:
        return {":": "hex_colon", "-": "hex_dash", ",": "hex_comma", "|": "hex_pipe"}.get(split.group(1), f"hex_split_{split.group(1)}")
    if split:
        name = {"|": "decimal_pipe", "-": "decimal_dash", ",": "decimal_comma"}.get(split.group(1), "decimal_split")
        return f"reversed_decimal" if reverse and split.group(1) == "," else name
    if re.search(r"Mid\s*\(", body, re.I) and hexed:
        return "hex_compact"
    return "unknown"


def classify_control(recover_lines: list[str], suffix: str) -> tuple[str, list[str]]:
    """Wrapper around the `Decode_<suffix>(` call: innermost loop/Select block
    that is still open at that line, else On Error Resume Next, else direct.
    `If True Then` and single-line If ... Then ... Else are not wrappers
    (`if_guard` is not an accepted value)."""
    stack: list[str] = []
    on_error = False
    for raw in recover_lines:
        t = strip_comment(raw).strip()
        low = t.lower()
        if not t:
            continue
        if re.search(rf"\bDecode_{suffix}\s*\(", t, re.I):
            break
        if re.match(r"on error resume next", low):
            on_error = True
        elif re.match(r"on error goto 0", low):
            on_error = False
        elif re.match(r"for\b(?!\s+each)", low):
            stack.append("for_once")
        elif re.match(r"next\b", low):
            _pop(stack, "for_once")
        elif re.match(r"do\b", low):
            stack.append("do_once")
        elif re.match(r"loop\b", low):
            _pop(stack, "do_once")
        elif re.match(r"while\b", low):
            stack.append("while_once")
        elif re.match(r"wend\b", low):
            _pop(stack, "while_once")
        elif re.match(r"select case\b", low):
            stack.append("select_case")
        elif re.match(r"end select\b", low):
            _pop(stack, "select_case")
        elif re.match(r"if\b.*\bthen\s*$", low):
            stack.append("if_block")
        elif re.match(r"end if\b", low):
            _pop(stack, "if_block")
    else:
        raise VbsUnsupported("no Decode call in Recover")
    wrappers = [s for s in stack if s != "if_block"]
    if wrappers:
        return wrappers[-1], list(stack) + (["on_error"] if on_error else [])
    if on_error:
        return "error_guard", ["on_error"]
    return "direct", list(stack)


def _pop(stack: list[str], kind: str) -> None:
    for j in range(len(stack) - 1, -1, -1):
        if stack[j] == kind:
            del stack[j]
            return


def analyse_vbs(text: str) -> VbsAnalysis:
    consts = _consts(text)
    funcs = _functions(text)
    m = re.search(r"^\s*Function\s+Recover_(\w+)", text, re.M | re.I)
    if not m:
        raise VbsUnsupported("no Recover_ function")
    sfx = m.group(1)
    material = consts.get(f"Material_{sfx}")
    if material is None:
        raise VbsUnsupported("no Material constant")
    label = consts.get(f"Codec_{sfx}", "||")
    lab = (label.split("|") + ["", "", ""])[:3]
    load_name, dec_name = f"Load_{sfx}", f"Decode_{sfx}"
    data = run_function(text, load_name, [], consts, funcs)
    out = run_function(text, dec_name, [data], consts, funcs)
    descriptor = "".join(chr(b) for b in out)

    def decoder(xs):
        return run_function(text, dec_name, [list(xs)], consts, funcs)

    dec_src = "\n".join(strip_comment(l) for l in funcs[dec_name])
    sign = re.search(r"\(\s*\w+\s*\(\s*\w+\s*\)\s*([+-])\s*\d+", dec_src)
    family, key = _family(decoder, sign.group(1) if sign else "+")
    control, wrappers = classify_control(funcs[f"Recover_{sfx}"], sfx)
    rep = _representation(funcs[load_name])
    notes = []
    if rep != lab[0]:
        notes.append(f"representation: code={rep!r} label={lab[0]!r}")
    if family != lab[1]:
        notes.append(f"transform: behaviour={family!r} label={lab[1]!r}")
    if key and lab[2] and key != lab[2].lower():
        notes.append(f"key: behaviour={key!r} label={lab[2]!r}")
    return VbsAnalysis(sfx, descriptor, rep, lab[0], family, lab[1], lab[2], control, wrappers, notes)
