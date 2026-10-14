#!/usr/bin/env python3
"""Static analyser for Day 5 ("Hatch Careful") Python specimens.

The specimens are *inert* fixtures that still look like real droppers, so we
never `exec`/`import`/`compile` them.  Instead the module is parsed with
`ast.parse` (parsing alone executes nothing) and walked by a small, whitelisted
interpreter that supports exactly the Python subset the specimens use:

    assignments, functions/lambdas/closures, classes with @staticmethod,
    decorators, if/while/for/try/match, comprehensions, f-strings, walrus,
    str/bytes/list/tuple/dict methods, base64/binascii/io helpers.

Anything outside the whitelist raises `PyUnsupported` (never guesses).
Attribute names starting with `_` are always rejected, `os.environ.get` returns
its default (the fixture env is unset), and a step budget bounds every run.

Public API:  analyse_python(text) -> PyAnalysis
"""
from __future__ import annotations

import ast
import base64
import binascii
import builtins
import io
import re
import zlib
from dataclasses import dataclass, field


class PyUnsupported(Exception):
    """Raised instead of guessing."""


class _Return(Exception):
    def __init__(self, value):
        self.value = value


class _Break(Exception):
    pass


class _Continue(Exception):
    pass


# ---------------------------------------------------------------------------
# whitelists
# ---------------------------------------------------------------------------

_EXC = {n: getattr(builtins, n) for n in (
    "Exception", "ValueError", "TypeError", "KeyError", "IndexError", "LookupError", "RuntimeError",
    "AttributeError", "UnicodeDecodeError", "OSError", "NameError", "ArithmeticError", "ZeroDivisionError",
    "StopIteration", "AssertionError", "NotImplementedError", "BaseException")}
_EXC["binascii.Error"] = binascii.Error

_SAFE_METHODS = {
    str: {"split", "rsplit", "join", "replace", "strip", "lstrip", "rstrip", "lower", "upper", "encode",
          "startswith", "endswith", "format", "format_map", "zfill", "partition", "rpartition", "title",
          "capitalize", "swapcase", "casefold", "find", "rfind", "index", "count", "removeprefix",
          "removesuffix", "translate", "isdigit", "isalpha", "isalnum", "ljust", "rjust", "center"},
    bytes: {"decode", "hex", "split", "join", "replace", "strip", "lstrip", "rstrip", "translate", "find",
            "startswith", "endswith", "count", "upper", "lower", "index", "rfind", "partition", "zfill"},
    bytearray: {"decode", "hex", "append", "extend", "reverse", "join", "replace", "find", "translate"},
    list: {"append", "extend", "reverse", "index", "count", "copy", "insert", "pop", "sort"},
    tuple: {"index", "count"},
    dict: {"get", "items", "keys", "values", "copy", "update", "pop", "setdefault"},
    set: {"add", "union", "intersection", "difference"},
    io.BytesIO: {"read", "getvalue", "write", "seek"},
    io.StringIO: {"read", "getvalue", "write", "seek"},
    memoryview: {"tobytes", "tolist"},
}


class _Module:
    def __init__(self, name, attrs):
        self.name, self.attrs = name, attrs


class _Environ:
    def get(self, key, default=None):
        return default

    def __contains__(self, key):
        return False

    def __getitem__(self, key):
        raise KeyError(key)


_MODULES = {
    "base64": _Module("base64", {n: getattr(base64, n) for n in (
        "b64decode", "urlsafe_b64decode", "b32decode", "b16decode", "a85decode", "b85decode",
        "standard_b64decode", "b64encode", "urlsafe_b64encode")}),
    "binascii": _Module("binascii", {"unhexlify": binascii.unhexlify, "a2b_hex": binascii.a2b_hex,
                                     "hexlify": binascii.hexlify, "a2b_base64": binascii.a2b_base64}),
    "io": _Module("io", {"BytesIO": io.BytesIO, "StringIO": io.StringIO}),
    "os": _Module("os", {"environ": _Environ(), "getenv": lambda k, d=None: d, "sep": "\\", "linesep": "\r\n"}),
    "zlib": _Module("zlib", {"decompress": zlib.decompress}),
    "sys": _Module("sys", {"platform": "win32", "argv": ["script"]}),
}


def _staticmethod(f):
    return f


_BUILTINS = {
    "bytes": bytes, "bytearray": bytearray, "int": int, "str": str, "len": len, "range": range,
    "reversed": reversed, "tuple": tuple, "list": list, "dict": dict, "set": set, "sorted": sorted,
    "iter": iter, "next": next, "memoryview": memoryview, "chr": chr, "ord": ord, "enumerate": enumerate,
    "zip": zip, "min": min, "max": max, "sum": sum, "any": any, "all": all, "bool": bool, "hex": hex,
    "abs": abs, "isinstance": isinstance, "staticmethod": _staticmethod, "classmethod": _staticmethod,
    "True": True, "False": False, "None": None, "bytes.fromhex": None, "filter": filter, "map": map,
    "print": lambda *a, **k: None, "format": format, "repr": repr, "divmod": divmod,
}
_BUILTINS.update(_EXC)

_BINOPS = {
    ast.Add: lambda a, b: a + b, ast.Sub: lambda a, b: a - b, ast.Mult: lambda a, b: a * b,
    ast.Mod: lambda a, b: a % b, ast.FloorDiv: lambda a, b: a // b, ast.BitXor: lambda a, b: a ^ b,
    ast.BitAnd: lambda a, b: a & b, ast.BitOr: lambda a, b: a | b, ast.LShift: lambda a, b: a << b,
    ast.RShift: lambda a, b: a >> b, ast.Div: lambda a, b: a / b, ast.Pow: lambda a, b: a ** b,
}
_CMPOPS = {
    ast.Eq: lambda a, b: a == b, ast.NotEq: lambda a, b: a != b, ast.Lt: lambda a, b: a < b,
    ast.LtE: lambda a, b: a <= b, ast.Gt: lambda a, b: a > b, ast.GtE: lambda a, b: a >= b,
    ast.In: lambda a, b: a in b, ast.NotIn: lambda a, b: a not in b, ast.Is: lambda a, b: a is b,
    ast.IsNot: lambda a, b: a is not b,
}


class _Func:
    def __init__(self, interp, node, closure, name=None):
        self.interp, self.node, self.closure = interp, node, closure
        self.name = name or getattr(node, "name", "<lambda>")

    def __call__(self, *args, **kwargs):
        return self.interp.call_func(self, args, kwargs)


class _ClassObj:
    def __init__(self, name, ns):
        self.name, self.ns = name, ns


class _Scope:
    def __init__(self, parent=None):
        self.vars: dict = {}
        self.parent = parent

    def lookup(self, name):
        s = self
        while s is not None:
            if name in s.vars:
                return s.vars[name]
            s = s.parent
        if name in _BUILTINS:
            return _BUILTINS[name]
        raise PyUnsupported(f"unknown name {name!r}")

    def assign(self, name, value):
        self.vars[name] = value


class Interp:
    def __init__(self, budget: int = 400_000):
        self.budget = budget
        self.globals = _Scope()

    # -- plumbing ----------------------------------------------------------
    def tick(self):
        self.budget -= 1
        if self.budget < 0:
            raise PyUnsupported("step budget exhausted")

    def run_module(self, tree: ast.Module):
        for stmt in tree.body:
            self.exec_stmt(stmt, self.globals)

    def call_func(self, fn: _Func, args, kwargs):
        node = fn.node
        scope = _Scope(fn.closure)
        a = node.args
        params = [p.arg for p in a.posonlyargs + a.args]
        defaults = [self.eval(d, fn.closure) for d in a.defaults]
        if len(args) > len(params) and a.vararg is None:
            raise PyUnsupported(f"too many args to {fn.name}")
        for i, p in enumerate(params):
            if i < len(args):
                scope.assign(p, args[i])
            elif p in kwargs:
                scope.assign(p, kwargs[p])
            else:
                di = i - (len(params) - len(defaults))
                if di < 0:
                    raise PyUnsupported(f"missing arg {p} for {fn.name}")
                scope.assign(p, defaults[di])
        if a.vararg is not None:
            scope.assign(a.vararg.arg, tuple(args[len(params):]))
        if isinstance(node, ast.Lambda):
            return self.eval(node.body, scope)
        try:
            for stmt in node.body:
                self.exec_stmt(stmt, scope)
        except _Return as r:
            return r.value
        return None

    # -- statements --------------------------------------------------------
    def exec_block(self, body, scope):
        for stmt in body:
            self.exec_stmt(stmt, scope)

    def assign_target(self, target, value, scope):
        if isinstance(target, ast.Name):
            scope.assign(target.id, value)
        elif isinstance(target, (ast.Tuple, ast.List)):
            vals = list(value)
            if len(vals) != len(target.elts):
                raise PyUnsupported("unpack length mismatch")
            for t, v in zip(target.elts, vals):
                self.assign_target(t, v, scope)
        elif isinstance(target, ast.Subscript):
            obj = self.eval(target.value, scope)
            if not isinstance(obj, (dict, list, bytearray)):
                raise PyUnsupported("subscript assignment on unsupported type")
            obj[self.eval(target.slice, scope)] = value
        else:
            raise PyUnsupported(f"assignment target {type(target).__name__}")

    def exec_stmt(self, node, scope):
        self.tick()
        t = type(node)
        if t is ast.Assign:
            v = self.eval(node.value, scope)
            for tg in node.targets:
                self.assign_target(tg, v, scope)
        elif t is ast.AnnAssign:
            if node.value is not None:
                self.assign_target(node.target, self.eval(node.value, scope), scope)
        elif t is ast.AugAssign:
            if not isinstance(node.target, ast.Name):
                raise PyUnsupported("augassign target")
            cur = scope.lookup(node.target.id)
            scope.assign(node.target.id, _BINOPS[type(node.op)](cur, self.eval(node.value, scope)))
        elif t is ast.Expr:
            self.eval(node.value, scope)
        elif t is ast.Return:
            raise _Return(self.eval(node.value, scope) if node.value is not None else None)
        elif t is ast.Pass:
            pass
        elif t is ast.Break:
            raise _Break()
        elif t is ast.Continue:
            raise _Continue()
        elif t is ast.If:
            self.exec_block(node.body if self.eval(node.test, scope) else node.orelse, scope)
        elif t is ast.While:
            while self.eval(node.test, scope):
                self.tick()
                try:
                    self.exec_block(node.body, scope)
                except _Break:
                    break
                except _Continue:
                    continue
            else:
                self.exec_block(node.orelse, scope)
        elif t is ast.For:
            broke = False
            for item in self.eval(node.iter, scope):
                self.tick()
                self.assign_target(node.target, item, scope)
                try:
                    self.exec_block(node.body, scope)
                except _Break:
                    broke = True
                    break
                except _Continue:
                    continue
            if not broke:
                self.exec_block(node.orelse, scope)
        elif t is ast.Try:
            try:
                try:
                    self.exec_block(node.body, scope)
                except (_Return, _Break, _Continue):
                    raise
                except PyUnsupported:
                    raise
                except Exception as exc:  # interpreted exception
                    for h in node.handlers:
                        if h.type is None or self._handler_matches(h.type, exc, scope):
                            if h.name:
                                scope.assign(h.name, exc)
                            self.exec_block(h.body, scope)
                            break
                    else:
                        raise
                else:
                    self.exec_block(node.orelse, scope)
            finally:
                self.exec_block(node.finalbody, scope)
        elif t is ast.Raise:
            if node.exc is None:
                raise PyUnsupported("bare raise")
            exc = self.eval(node.exc, scope)
            if isinstance(exc, type) and issubclass(exc, BaseException):
                exc = exc()
            raise exc
        elif t is ast.FunctionDef:
            fn = _Func(self, node, scope)
            for dec in reversed(node.decorator_list):
                fn = self.eval(dec, scope)(fn)
            scope.assign(node.name, fn)
        elif t is ast.ClassDef:
            cscope = _Scope(scope)
            self.exec_block(node.body, cscope)
            scope.assign(node.name, _ClassObj(node.name, dict(cscope.vars)))
        elif t is ast.Import:
            for a in node.names:
                if a.name not in _MODULES:
                    raise PyUnsupported(f"import {a.name}")
                scope.assign((a.asname or a.name).split(".")[0], _MODULES[a.name])
        elif t is ast.ImportFrom:
            mod = _MODULES.get(node.module or "")
            if mod is None:
                raise PyUnsupported(f"from {node.module} import ...")
            for a in node.names:
                scope.assign(a.asname or a.name, mod.attrs[a.name])
        elif t is ast.Match:
            subject = self.eval(node.subject, scope)
            for case in node.cases:
                if self.match_pattern(case.pattern, subject, scope) and (
                        case.guard is None or self.eval(case.guard, scope)):
                    self.exec_block(case.body, scope)
                    break
        elif t in (ast.Global, ast.Nonlocal, ast.Assert, ast.Delete):
            pass
        else:
            raise PyUnsupported(f"statement {t.__name__}")

    def _handler_matches(self, type_node, exc, scope):
        spec = self.eval(type_node, scope)
        specs = spec if isinstance(spec, tuple) else (spec,)
        return any(isinstance(s, type) and isinstance(exc, s) for s in specs)

    def match_pattern(self, pat, subject, scope):
        if isinstance(pat, ast.MatchValue):
            return subject == self.eval(pat.value, scope)
        if isinstance(pat, ast.MatchSingleton):
            return subject is pat.value
        if isinstance(pat, ast.MatchAs):
            if pat.pattern is not None and not self.match_pattern(pat.pattern, subject, scope):
                return False
            if pat.name:
                scope.assign(pat.name, subject)
            return True
        if isinstance(pat, ast.MatchOr):
            return any(self.match_pattern(p, subject, scope) for p in pat.patterns)
        raise PyUnsupported(f"match pattern {type(pat).__name__}")

    # -- expressions -------------------------------------------------------
    def eval(self, node, scope):
        self.tick()
        t = type(node)
        if t is ast.Constant:
            return node.value
        if t is ast.Name:
            return scope.lookup(node.id)
        if t is ast.Tuple:
            return tuple(self._elts(node.elts, scope))
        if t is ast.List:
            return list(self._elts(node.elts, scope))
        if t is ast.Set:
            return set(self._elts(node.elts, scope))
        if t is ast.Dict:
            return {self.eval(k, scope): self.eval(v, scope) for k, v in zip(node.keys, node.values)}
        if t is ast.JoinedStr:
            return "".join(str(self.eval(v, scope)) for v in node.values)
        if t is ast.FormattedValue:
            v = self.eval(node.value, scope)
            spec = self.eval(node.format_spec, scope) if node.format_spec else ""
            return format(v, spec)
        if t is ast.BinOp:
            op = _BINOPS.get(type(node.op))
            if op is None:
                raise PyUnsupported(f"binop {type(node.op).__name__}")
            return op(self.eval(node.left, scope), self.eval(node.right, scope))
        if t is ast.UnaryOp:
            v = self.eval(node.operand, scope)
            if isinstance(node.op, ast.Not):
                return not v
            if isinstance(node.op, ast.USub):
                return -v
            if isinstance(node.op, ast.Invert):
                return ~v
            return +v
        if t is ast.BoolOp:
            if isinstance(node.op, ast.And):
                v = True
                for x in node.values:
                    v = self.eval(x, scope)
                    if not v:
                        return v
                return v
            v = False
            for x in node.values:
                v = self.eval(x, scope)
                if v:
                    return v
            return v
        if t is ast.Compare:
            left = self.eval(node.left, scope)
            for op, comp in zip(node.ops, node.comparators):
                right = self.eval(comp, scope)
                if not _CMPOPS[type(op)](left, right):
                    return False
                left = right
            return True
        if t is ast.IfExp:
            return self.eval(node.body if self.eval(node.test, scope) else node.orelse, scope)
        if t is ast.Subscript:
            obj = self.eval(node.value, scope)
            sl = node.slice
            if isinstance(sl, ast.Slice):
                key = slice(*(self.eval(p, scope) if p is not None else None for p in (sl.lower, sl.upper, sl.step)))
            else:
                key = self.eval(sl, scope)
            return obj[key]
        if t is ast.Attribute:
            return self.get_attr(self.eval(node.value, scope), node.attr)
        if t is ast.Call:
            return self.eval_call(node, scope)
        if t is ast.Lambda:
            return _Func(self, node, scope, "<lambda>")
        if t is ast.NamedExpr:
            v = self.eval(node.value, scope)
            self.assign_target(node.target, v, scope)
            return v
        if t is ast.Starred:
            return self.eval(node.value, scope)
        if t in (ast.ListComp, ast.SetComp, ast.GeneratorExp, ast.DictComp):
            out = list(self._comprehend(node, 0, _Scope(scope)))
            if t is ast.ListComp:
                return out
            if t is ast.SetComp:
                return set(out)
            if t is ast.DictComp:
                return dict(out)
            return iter(out)
        raise PyUnsupported(f"expression {t.__name__}")

    def _elts(self, elts, scope):
        for e in elts:
            if isinstance(e, ast.Starred):
                yield from self.eval(e.value, scope)
            else:
                yield self.eval(e, scope)

    def _comprehend(self, node, gi, scope):
        if gi == len(node.generators):
            if isinstance(node, ast.DictComp):
                yield (self.eval(node.key, scope), self.eval(node.value, scope))
            else:
                yield self.eval(node.elt, scope)
            return
        gen = node.generators[gi]
        for item in self.eval(gen.iter, scope):
            self.tick()
            self.assign_target(gen.target, item, scope)
            if all(self.eval(c, scope) for c in gen.ifs):
                yield from self._comprehend(node, gi + 1, scope)

    def get_attr(self, obj, name):
        if name.startswith("_"):
            raise PyUnsupported(f"private attribute {name}")
        if isinstance(obj, _Module):
            if name not in obj.attrs:
                raise PyUnsupported(f"{obj.name}.{name}")
            return obj.attrs[name]
        if isinstance(obj, _ClassObj):
            if name not in obj.ns:
                raise PyUnsupported(f"{obj.name}.{name}")
            return obj.ns[name]
        if isinstance(obj, _Environ):
            return getattr(obj, name)
        if obj is bytes and name == "fromhex":
            return bytes.fromhex
        if obj is str and name in ("maketrans",):
            return str.maketrans
        if obj is bytes and name == "maketrans":
            return bytes.maketrans
        for typ, names in _SAFE_METHODS.items():
            if isinstance(obj, typ) and name in names:
                return getattr(obj, name)
        raise PyUnsupported(f"attribute {type(obj).__name__}.{name}")

    def eval_call(self, node, scope):
        fn = self.eval(node.func, scope)
        args = list(self._elts(node.args, scope))
        kwargs = {}
        for kw in node.keywords:
            if kw.arg is None:
                kwargs.update(self.eval(kw.value, scope))
            else:
                kwargs[kw.arg] = self.eval(kw.value, scope)
        if isinstance(fn, _Func):
            return fn(*args, **kwargs)
        if isinstance(fn, _ClassObj):
            raise PyUnsupported("class instantiation")
        if callable(fn):
            return fn(*args, **kwargs)
        raise PyUnsupported(f"call of {type(fn).__name__}")


# ---------------------------------------------------------------------------
# analysis driver
# ---------------------------------------------------------------------------


@dataclass
class PyAnalysis:
    ident: str = ""
    descriptor: str = ""
    codec: str = ""                      # CODEC_<id> literal ("representation|transform|key")
    surface: tuple = ()                  # surface_<id>() result
    obfuscation: str = ""                # obfuscation_<id>() result
    operation_plan: tuple = ()           # plan_<id>()[:-2] when callable
    notes: list[str] = field(default_factory=list)

    @property
    def syntax(self) -> str:
        return self.surface[0]

    @property
    def carrier(self) -> str:
        return self.surface[2]

    @property
    def windows_chain(self) -> str:
        return self.surface[3]


def is_python_specimen(text: str) -> bool:
    return bool(re.search(r"^\s*def\s+recover_\w+\s*\(", text, re.M) and re.search(r"^\s*MATERIAL_\w+\s*=", text, re.M)
                and re.search(r"^\s*def\s+surface_\w+\s*\(", text, re.M))


def analyse_python(text: str) -> PyAnalysis:
    try:
        tree = ast.parse(text)
    except SyntaxError as e:
        raise PyUnsupported(f"syntax error: {e}") from e
    it = Interp()
    it.run_module(tree)
    g = it.globals.vars
    mats = [k for k in g if k.startswith("MATERIAL_")]
    if len(mats) != 1:
        raise PyUnsupported(f"expected one MATERIAL_ constant, found {mats}")
    ident = mats[0][len("MATERIAL_"):]
    a = PyAnalysis(ident=ident)
    a.codec = g.get(f"CODEC_{ident}", "")
    for name in ("inspect", "surface", "obfuscation"):
        if f"{name}_{ident}" not in g:
            raise PyUnsupported(f"missing {name}_{ident}")
    a.descriptor = it.call_func(g[f"inspect_{ident}"], (), {})
    a.surface = tuple(it.call_func(g[f"surface_{ident}"], (), {}))
    a.obfuscation = it.call_func(g[f"obfuscation_{ident}"], (), {})
    if len(a.surface) != 4 or not all(isinstance(s, str) for s in a.surface):
        raise PyUnsupported(f"surface tuple shape {a.surface!r}")
    if a.surface[1] != a.obfuscation:
        a.notes.append(f"surface[1]={a.surface[1]!r} != obfuscation()={a.obfuscation!r}")
    if f"plan_{ident}" in g:
        try:
            plan = tuple(it.call_func(g[f"plan_{ident}"], (), {}))
            a.operation_plan = plan[:-2]
            if tuple(plan[-2:]) != (a.surface, a.obfuscation):
                a.notes.append("plan() tail disagrees with surface/obfuscation")
        except (PyUnsupported, KeyError, ValueError) as e:
            a.notes.append(f"plan_{ident}: {type(e).__name__}: {e}")
    return a


def analyse_ps_handoff(text: str) -> dict[str, str]:
    """`invoke-python-stage.ps1`: inert PowerShell-to-Python precursor ($Var = 'literal' lines)."""
    vals: dict[str, str] = {}
    for m in re.finditer(r"^\s*\$(\w+)\s*=\s*'([^']*)'\s*$", text, re.M):
        vals[m.group(1)] = m.group(2)
    for m in re.finditer(r"^\s*(\w+)\s*=\s*'([^']*)'\s*$", text, re.M):  # inside [pscustomobject]@{...}
        vals.setdefault(m.group(1), m.group(2))
    if not vals:
        raise PyUnsupported("no handoff literals")
    return vals
