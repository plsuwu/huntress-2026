#!/usr/bin/env python3
"""Day 4 script-analysis "Dim and Dimmer" (script-analysis-day-04-shell-contrast).

Day 3's composite archive (PowerShell + JScript + Batch) plus nine VBScript
specimens.  PS/JS/Batch are solved by the Day 3 solver (`expansion_pack.py`,
imported unchanged); VBScript is solved by `vbs_static.py`.  Nothing in the
archive is executed.

    dim_and_dimmer.py --zip evidence.zip --questions start.json [--out answers.json] [--log log.json]
    dim_and_dimmer.py --dir extracted/  --questions start.json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "day-03-script-analysis"))

import expansion_pack as ep  # noqa: E402
from vbs_static import VbsAnalysis, VbsUnsupported, analyse_vbs  # noqa: E402

VBS_LANGUAGE_TECHNIQUE = "T1059.005"  # Command and Scripting Interpreter: Visual Basic


def classify(name: str, data: bytes) -> str:
    text = data.decode("utf-8", "replace")
    if re.search(r"^\s*Function\s+Recover_\w+", text, re.M | re.I) and re.search(r"^\s*Const\s+Material_", text, re.M | re.I):
        return "vbs"
    return ep.classify_member(name, data)


def answer_vbs(qid: str, a: VbsAnalysis, plan: "ep.Plan") -> str:
    if qid.endswith("-role"):
        return ep.role_of(plan.steps)
    if qid.endswith("-attack"):
        return ep.attack_of(plan.steps, VBS_LANGUAGE_TECHNIQUE)
    if qid.endswith("-representation"):
        return a.representation_label or a.representation
    if qid.endswith("-transform"):
        return a.transform_label or a.transform
    if qid.endswith("-control"):
        return a.control
    return ep.answer_question(qid, "", "vbs", type("A", (), {"plan": plan})())


def solve(deliveries, questions):
    cache: dict = {}
    answers: dict[str, str] = {}
    errors: dict[str, str] = {}
    log: dict[str, dict] = {}
    for q in questions:
        qid = q["id"]
        try:
            label = q.get("artifact", "")
            di, files = ep.pick_delivery(label, deliveries)
            member = ep.member_name(label)
            name = next((n for n in files if n == member or n.endswith("/" + member)), None)
            if name is None:
                if len(files) == 1:
                    name = next(iter(files))
                else:
                    raise ep.Unsupported(f"member {member!r} not in delivery {di}")
            key = (di, name)
            if key not in cache:
                kind = classify(name, files[name])
                text = files[name].decode("utf-8", "replace")
                if kind == "vbs":
                    a = analyse_vbs(text)
                    plan = ep.parse_pipe_descriptor(a.descriptor)
                    cache[key] = (kind, (a, plan))
                    log[f"{di}:{name}"] = {"kind": kind, "descriptor": a.descriptor, "representation": a.representation_label,
                                           "transform": a.transform, "transform_label": a.transform_label, "key": a.key_label,
                                           "control": a.control, "wrappers": a.wrappers, "notes": a.notes}
                else:
                    an = {"batch": ep.analyse_batch, "js": ep.analyse_js, "ps1": ep.analyse_ps1}[kind](text)
                    cache[key] = (kind, an)
            kind, an = cache[key]
            if kind == "vbs":
                answers[qid] = answer_vbs(qid, *an)
            else:
                answers[qid] = ep.answer_question(qid, q.get("question", ""), kind, an)
        except (ep.Unsupported, VbsUnsupported, KeyError, ValueError, IndexError) as e:
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
        if v["notes"]:
            print(f"NOTE {k}: {v['notes']}", file=sys.stderr)
    text = json.dumps(answers, indent=2, ensure_ascii=False)
    if args.out:
        Path(args.out).write_text(text + "\n")
    else:
        print(text)
    if args.log:
        Path(args.log).write_text(json.dumps(log, indent=2, ensure_ascii=False) + "\n")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
