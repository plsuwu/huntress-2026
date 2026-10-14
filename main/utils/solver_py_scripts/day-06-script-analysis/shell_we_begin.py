#!/usr/bin/env python3
"""Day 6 script-analysis "Shell We Begin" (script-analysis-day-06-bash-behavior-graphs).

Day 5's composite archive (PowerShell + JScript + Batch + VBScript + Python, solved unchanged by
`../day-05-script-analysis/hatch_careful.py`) plus six Bash specimens (`bash-<specimen>-*` questions)
analysed by `sh_static.py`.  New question kinds for Bash: `-stage-count` (descriptor `stage_count`) and
`-steps` (descriptor `steps` joined by `>`).  Nothing in the archive is executed.

    shell_we_begin.py --zip evidence.zip --questions start.json [--out answers.json] [--log log.json]
    shell_we_begin.py --dir extracted/  --questions start.json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "day-05-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-04-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-03-script-analysis"))

import expansion_pack as ep  # noqa: E402
import hatch_careful as hc  # noqa: E402
from sh_static import ShAnalysis, ShUnsupported, analyse_sh  # noqa: E402

SH_LANGUAGE_TECHNIQUE = "T1059.004"  # Command and Scripting Interpreter: Unix Shell

SH_VALUE_KEYS = dict(ep.VALUE_KEYS)
SH_VALUE_KEYS.update({"user-agent": ["user_agent"], "cron-path": ["cron_path"], "service-name": ["service_name"],
                      "arch-probe": ["arch_probe"], "handoff-id": ["handoff_id"]})


def _is_bash_question(qid: str) -> bool:
    """`bash-...` in single/time-trial, `setN.bash-...` in mastery."""
    return re.sub(r"^set\d+\.", "", qid).startswith("bash-")


def answer_bash(qid: str, plan: "ep.Plan") -> str:
    if qid.endswith("-role"):
        return ep.role_of(plan.steps)
    if qid.endswith("-attack"):
        return ep.attack_of(plan.steps, SH_LANGUAGE_TECHNIQUE)
    if qid.endswith("-adapter"):
        return plan.adapter
    if qid.endswith("-stage-count"):
        if plan.stage_count is None:
            raise ep.Unsupported(f"{qid}: no stage_count in descriptor")
        return str(plan.stage_count)
    if qid.endswith("-steps"):
        return ">".join(plan.steps)
    suffixes = sorted({"-" + k for k in SH_VALUE_KEYS}, key=len, reverse=True)
    for suffix in suffixes:
        if qid.endswith(suffix):
            s = suffix[1:]
            for key in [s.replace("-", "_")] + SH_VALUE_KEYS.get(s, []):
                if key in plan.values:
                    return plan.values[key]
            raise ep.Unsupported(f"{qid}: none of {s} keys in {sorted(plan.values)}")
    raise ep.Unsupported(f"unrouted bash question {qid}")


def solve(deliveries, questions):
    sh_q = [q for q in questions if _is_bash_question(q["id"])]
    rest = [q for q in questions if not _is_bash_question(q["id"])]
    answers, errors, log = hc.solve(deliveries, rest)
    cache: dict = {}
    for q in sh_q:
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
                a: ShAnalysis = analyse_sh(files[name].decode("utf-8", "replace"))
                plan = ep.parse_pipe_descriptor(a.descriptor)
                if plan.stage_count is not None and plan.stage_count != len(plan.steps):
                    a.notes.append(f"stage_count={plan.stage_count} != len(steps)={len(plan.steps)}")
                cache[key] = (a, plan)
                log[f"{di}:{name}"] = {"kind": "bash", "descriptor": a.descriptor, "codec": a.codec,
                                       "representation": a.representation, "transform": a.transform, "key": a.key,
                                       "steps": plan.steps, "notes": a.notes}
            _a, plan = cache[key]
            answers[qid] = answer_bash(qid, plan)
        except (ep.Unsupported, ShUnsupported, KeyError, ValueError, IndexError) as e:
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
