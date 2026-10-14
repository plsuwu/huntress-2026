#!/usr/bin/env python3
"""Day 5 script-analysis "Hatch Careful" (script-analysis-day-05-python-carrier-lab).

Day 4's composite archive (PowerShell + JScript + Batch + VBScript, solved by
`../day-04-script-analysis/dim_and_dimmer.py`, imported unchanged) plus fourteen
Python specimens (`python-<specimen>-*` questions) and one inert
PowerShell->Python handoff precursor.  Python specimens are analysed by
`py_static.py` (ast + whitelisted interpreter; nothing is executed).

    hatch_careful.py --zip evidence.zip --questions start.json [--out answers.json] [--log log.json]
    hatch_careful.py --dir extracted/  --questions start.json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "day-04-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-03-script-analysis"))

import dim_and_dimmer as dd  # noqa: E402
import expansion_pack as ep  # noqa: E402
from py_static import PyAnalysis, PyUnsupported, analyse_python, analyse_ps_handoff, is_python_specimen  # noqa: E402

PY_LANGUAGE_TECHNIQUE = "T1059.006"  # Command and Scripting Interpreter: Python

# question-id suffix -> descriptor key candidates (suffix hyphens -> underscores is tried first)
PY_VALUE_KEYS = dict(ep.VALUE_KEYS)
PY_VALUE_KEYS.update({"handoff-id": ["handoff_id"], "user-agent": ["user_agent"], "host-id": ["host_id"]})


def answer_python(qid: str, a: PyAnalysis, plan: "ep.Plan") -> str:
    if qid.endswith("-role"):
        return ep.role_of(plan.steps)
    if qid.endswith("-attack"):
        return ep.attack_of(plan.steps, PY_LANGUAGE_TECHNIQUE)
    if qid.endswith("-adapter"):
        return plan.adapter
    if qid.endswith("-syntax"):
        return a.syntax
    if qid.endswith("-obfuscation"):
        return a.obfuscation
    if qid.endswith("-carrier"):
        return a.carrier
    if qid.endswith("-windows-chain"):
        return a.windows_chain
    suffixes = sorted({"-" + k for k in PY_VALUE_KEYS} | {"-campaign"}, key=len, reverse=True)
    for suffix in suffixes:
        if qid.endswith(suffix):
            s = suffix[1:]
            for key in [s.replace("-", "_")] + PY_VALUE_KEYS.get(s, []):
                if key in plan.values:
                    return plan.values[key]
            raise ep.Unsupported(f"{qid}: none of {s} keys in {sorted(plan.values)}")
    raise ep.Unsupported(f"unrouted python question {qid}")


def _cross_check(a: PyAnalysis, plan: "ep.Plan") -> None:
    """The descriptor repeats syntax/obfuscation/carrier/windows_chain: flag any disagreement."""
    for key, got in (("syntax", a.syntax), ("obfuscation", a.obfuscation), ("carrier", a.carrier),
                     ("windows_chain", a.windows_chain)):
        want = plan.values.get(key)
        if want is not None and want != got:
            a.notes.append(f"descriptor {key}={want!r} != surface/obfuscation function {got!r}")
    if a.operation_plan and a.operation_plan[0] != plan.adapter:
        a.notes.append(f"operation_plan[0]={a.operation_plan[0]!r} != descriptor adapter={plan.adapter!r}")


def _is_python_question(qid: str) -> bool:
    """`python-...` in single/time-trial, `setN.python-...` in mastery."""
    return re.sub(r"^set\d+\.", "", qid).startswith("python-")


def solve(deliveries, questions):
    py_q = [q for q in questions if _is_python_question(q["id"])]
    rest = [q for q in questions if not _is_python_question(q["id"])]
    answers, errors, log = dd.solve(deliveries, rest)
    cache: dict = {}
    for q in py_q:
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
                a = analyse_python(text)
                plan = ep.parse_pipe_descriptor(a.descriptor)
                _cross_check(a, plan)
                cache[key] = (a, plan)
                log[f"{di}:{name}"] = {"kind": "python", "descriptor": a.descriptor, "codec": a.codec,
                                       "surface": list(a.surface), "obfuscation": a.obfuscation,
                                       "operation_plan": [list(x) if isinstance(x, tuple) else x for x in a.operation_plan],
                                       "notes": a.notes}
            a, plan = cache[key]
            answers[qid] = answer_python(qid, a, plan)
        except (ep.Unsupported, PyUnsupported, KeyError, ValueError, IndexError) as e:
            errors[qid] = f"{type(e).__name__}: {e}"
    return answers, errors, log


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--zip")
    ap.add_argument("--dir")
    ap.add_argument("--questions", required=True)
    ap.add_argument("--out")
    ap.add_argument("--log")
    args = ap.parse_args()
    if args.zip:
        files = ep.read_zip(args.zip)
    else:
        files = {p.name: p.read_bytes() for p in Path(args.dir).iterdir() if p.is_file()}
    questions = json.load(open(args.questions))["questions"]
    deliveries = ep.expand_deliveries(files, Path(args.zip).name if args.zip else "")
    answers, errors, log = solve(deliveries, questions)
    for qid, err in errors.items():
        print(f"ERR {qid}: {err}", file=sys.stderr)
    for k, v in log.items():
        if v.get("notes"):
            print(f"NOTE {k}: {v['notes']}", file=sys.stderr)
    text = json.dumps(answers, indent=2, ensure_ascii=False)
    if args.out:
        Path(args.out).write_text(text + "\n")
    else:
        print(text)
    if args.log:
        Path(args.log).write_text(json.dumps(log, indent=2, ensure_ascii=False) + "\n")
    print(f"{len(answers)} answers / {len(questions)} questions / {len(errors)} errors", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
