#!/usr/bin/env python3
"""Day 8 "Lost in Translation" (script-analysis-day-08-cross-language-iocs).

Day 7's composite (`../day-07-script-analysis/node_by_node.py`) unchanged, plus:
  * `polyglot-php-*` / `host-{hta,wsf,php}-*` specimens: v=3 descriptor in a string literal; encoding is
    plain hex / fragmented hex (`.` separated) / base64 / hex-of-base64 (nested; auto-detected, the
    `HUNTRESS_ANALYSIS_V1:` marker comment is a decoy copy and is ignored).
  * extra advanced-Bash Material forms (hex_compact pair loop, decimal_csv `printf '\\x%02x'`).

    lost_in_translation.py --zip evidence.zip --questions start.json [--out answers.json] [--log log.json]
    lost_in_translation.py --dir extracted/ --questions start.json
"""
from __future__ import annotations

import argparse
import base64
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "day-07-script-analysis"))
import node_by_node as nbn  # noqa: E402

ep = nbn.ep
NodeUnsupported = nbn.NodeUnsupported

# --- advanced bash: extend the Day 7 analyser --------------------------------------------------
_orig_adv = nbn.analyse_adv_bash


def _fields(raw: str) -> dict:
    return {k: v for k, _, v in (p.partition("=") for p in raw.split("|"))}


def analyse_adv_bash(text: str) -> dict:
    body = (re.search(r"^\s*Decode\(\)\s*\{(.*)\}\s*$", text, re.M) or [None, text])[1]
    xk = re.search(r"\^\s*16#([0-9a-fA-F]{1,2})\b", body)
    if not xk:
        try:
            return _orig_adv(text)
        except NodeUnsupported:
            pass
    m = re.search(r"readonly\s+Material_\w+=((?:'[^']*'\s*)+)", text)
    mat = "".join(re.findall(r"'([^']*)'", m.group(1)))
    codec = (re.search(r"readonly\s+Codec_\w+='([^'|]*)", text) or [None, ""])[1]
    if re.fullmatch(r"\d+(?:,\d+)*", mat):
        data = bytes(int(x) for x in mat.split(","))
    elif re.fullmatch(r"(?:[0-9a-fA-F]{2})+", mat):
        data = bytes.fromhex(mat)
    else:
        raise NodeUnsupported(f"advanced bash material form unknown (codec {codec!r}): {mat[:60]}")
    if xk:  # xor_hex: value=$((16#.. ^ 16#kk))
        data = bytes(b ^ int(xk.group(1), 16) for b in data)
    return _fields(data.decode("utf-8", "replace"))


nbn.analyse_adv_bash = analyse_adv_bash

# --- polyglot / host ---------------------------------------------------------------------------
ML_PREFIXES = ("polyglot-", "host-hta-", "host-wsf-", "host-php-", "host-sct-", "host-lua-")
LIT = re.compile(r"""(?:\$|var\s+|const\s+|let\s+)?descriptor_\w+\s*=\s*"([^"]+)"|^\s*descriptor_\w+\s*=\s*"([^"]+)"|\bDim\b""", re.M)
LIT2 = re.compile(r'descriptor_\w+\s*(?::\s*\w+\s*)?=\s*"([^"]+)"')  # Day 11: TS `const descriptor_x: string = "..."`
MOF_LIT = re.compile(r'\bDescriptor\s*=\s*"([^"]+)"\s*;')  # Day 10: `instance of HuntressAnalysis_xxx { Descriptor = "..."; }`


def decode_descriptor(lit: str) -> tuple[dict, list[str]]:
    s = lit.replace(".", "")
    raw = s.encode()
    trail = []
    for _ in range(4):
        if raw.startswith(b"v="):
            return _fields(raw.decode()), trail
        if re.fullmatch(rb"(?:[0-9a-fA-F]{2})+", raw):
            raw = bytes.fromhex(raw.decode()); trail.append("hex")
        else:
            raw = base64.b64decode(raw + b"=" * (-len(raw) % 4)); trail.append("base64")
    raise NodeUnsupported(f"descriptor did not decode: {lit[:40]}")


KEYMAP = {"campaign": ["campaign_id"], "obfuscation": ["obfuscation_profile"], "host-profile": ["host_profile"],
          "stage-profile": ["stage_profile"], "next-language": ["next_language"]}


def answer_ml(qid: str, d: dict) -> str:
    base = re.sub(r"^set\d+\.", "", qid)
    if base.endswith("-role"):
        return d["role"]
    if base.endswith("-attack"):
        return d["attack"]
    if base.endswith("-steps"):
        return ">".join(d["steps"].split(","))
    if base.endswith("-stage-count"):
        return str(int(d["stage_count"]))
    for suf in ("-obfuscation", "-host-profile", "-stage-profile", "-next-language", "-campaign",
                "-adapter", "-url", "-file-path"):
        if base.endswith(suf):
            s = suf[1:]
            for k in KEYMAP.get(s, []) + [s.replace("-", "_")]:
                if k in d:
                    return d[k]
            raise ep.Unsupported(f"{qid}: no key {s} in {sorted(d)}")
    raise ep.Unsupported(f"unrouted {qid}")


def solve(deliveries, questions):
    ml = [q for q in questions if re.sub(r"^set\d+\.", "", q["id"]).startswith(ML_PREFIXES)]
    rest = [q for q in questions if q not in ml]
    answers, errors, log = nbn.solve(deliveries, rest)
    cache: dict = {}
    for q in ml:
        qid = q["id"]
        try:
            label = q.get("artifact", "")
            di, files = ep.pick_delivery(label, deliveries)
            member = ep.member_name(label)
            name = next((n for n in files if n == member or n.endswith("/" + member)), None)
            if name is None:
                raise ep.Unsupported(f"member {member!r} not in delivery {di}")
            key = (di, name)
            if key not in cache:
                text = files[name].decode("utf-8", "replace")
                m = LIT2.search(text) or MOF_LIT.search(text)
                if not m:
                    raise NodeUnsupported(f"{name}: no descriptor literal")
                cache[key], trail = decode_descriptor(m.group(1))
                log[f"{di}:{name}"] = {"kind": "ml", "trail": trail, "descriptor": cache[key]}
            answers[qid] = answer_ml(qid, cache[key])
        except (ep.Unsupported, NodeUnsupported, KeyError, ValueError, IndexError) as e:
            errors[qid] = f"{type(e).__name__}: {e}"
    return answers, errors, log


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--zip"); ap.add_argument("--dir")
    ap.add_argument("--questions", required=True); ap.add_argument("--out"); ap.add_argument("--log")
    a = ap.parse_args()
    qs = json.loads(Path(a.questions).read_text())
    qs = qs["questions"] if isinstance(qs, dict) else qs
    if a.zip:
        deliveries = ep.expand_deliveries({Path(a.zip).name: Path(a.zip).read_bytes()})
    else:
        d = ep.Delivery()
        for p in Path(a.dir).rglob("*"):
            if p.is_file():
                d[str(p.relative_to(a.dir))] = p.read_bytes()
        deliveries = [d]
    answers, errors, log = solve(deliveries, qs)
    for k, v in errors.items():
        print("ERR", k, v[:150])
    print(f"{len(answers)} answers / {len(qs)} questions / {len(errors)} errors")
    if a.out:
        Path(a.out).write_text(json.dumps(answers, indent=1))
    if a.log:
        Path(a.log).write_text(json.dumps(log, indent=1, default=str))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
