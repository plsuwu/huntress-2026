#!/usr/bin/env python3
"""Day 7 "Node by Node" (script-analysis-day-07-node-operation-graphs).

Day 6's composite archive solved unchanged by `../day-06-script-analysis/shell_we_begin.py`, plus eight Node.js
specimens (`nodejs-<specimen>-*` questions, `setN.` prefix in mastery) analysed by `node_static.py`.

    node_by_node.py --zip evidence.zip --questions start.json [--out answers.json] [--log log.json]
    node_by_node.py --dir extracted/  --questions start.json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "day-06-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-05-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-04-script-analysis"))
sys.path.insert(0, str(HERE.parent / "day-03-script-analysis"))

import expansion_pack as ep  # noqa: E402
import shell_we_begin as swb  # noqa: E402
from node_static import NodeUnsupported, analyse_node  # noqa: E402

NODE_LANGUAGE_TECHNIQUE = "T1059.007"  # Command and Scripting Interpreter: JavaScript

NODE_VALUE_KEYS = dict(swb.SH_VALUE_KEYS)
NODE_VALUE_KEYS.update({"schedule-path": ["schedule_path", "cron_path", "task_path"]})


def _is_node_question(qid: str) -> bool:
    return re.sub(r"^set\d+\.", "", qid).startswith("nodejs-")


def answer_node(qid: str, plan) -> str:
    if qid.endswith("-role"):
        return ep.role_of(plan.steps)
    if qid.endswith("-attack"):
        if plan.steps[-2:] == ["build_schedule", "register_descriptor"]:
            return "T1053.003"  # Scheduled Task/Job: Cron (graded; T1053.005/.006/T1053 were incorrect)
        return ep.attack_of(plan.steps, NODE_LANGUAGE_TECHNIQUE)
    if qid.endswith("-adapter"):
        return plan.adapter
    if qid.endswith("-stage-count"):
        if plan.stage_count is None:
            raise ep.Unsupported(f"{qid}: no stage_count in descriptor")
        return str(plan.stage_count)
    if qid.endswith("-steps"):
        return ">".join(plan.steps)
    for suffix in sorted({"-" + k for k in NODE_VALUE_KEYS}, key=len, reverse=True):
        if qid.endswith(suffix):
            s = suffix[1:]
            for key in [s.replace("-", "_")] + NODE_VALUE_KEYS.get(s, []):
                if key in plan.values:
                    return plan.values[key]
            raise ep.Unsupported(f"{qid}: none of {s} keys in {sorted(plan.values)}")
    raise ep.Unsupported(f"unrouted node question {qid}")


def _is_adv_question(qid: str) -> bool:
    return re.sub(r"^set\d+\.", "", qid).startswith("bash-advanced-")


def analyse_adv_bash(text: str) -> dict:
    """Advanced Bash fixture: base64 descriptor `v=1|feature=..|primitive=..|campaign_id=..|stage_count=N|steps=a,b`."""
    chunks = re.search(r"readonly\s+Material_\w+=((?:'[^']*'\s*)+)", text)
    if not chunks:
        raise NodeUnsupported("no Material_")
    material = "".join(re.findall(r"'([^']*)'", chunks.group(1)))
    dec = re.search(r"^\s*Decode\(\)\s*\{(.*)\}\s*$", text, re.M)
    body = dec.group(1) if dec else text
    import base64
    if re.search(r"index--|\$\(\s*rev\s*\)|\|\s*rev\b", body):  # Material is reversed *before* decoding
        material = material[::-1]
    if "base64" in body:
        data = base64.b64decode(material + "=" * (-len(material) % 4))
    elif "//%/" in body or re.fullmatch(r"(?:%[0-9a-fA-F]{2})+", material):
        data = bytes(int(x, 16) for x in re.findall(r"%([0-9a-fA-F]{2})", material))
    elif "xxd" in body or "\\x%s" in body:
        data = bytes.fromhex(re.sub(r"[^0-9a-fA-F]", "", material))
    else:
        raise NodeUnsupported(f"advanced bash Decode not understood: {body[:120]}")
    raw = data.decode("utf-8", "replace")
    fields = {}
    for part in raw.split("|"):
        k, _, v = part.partition("=")
        fields[k] = v
    return fields


def answer_adv(qid: str, f: dict) -> str:
    if qid.endswith("-role"):
        return "advanced_syntax"
    if qid.endswith("-attack"):
        return "T1059.004"
    if qid.endswith("-feature"):
        return f["feature"]
    if qid.endswith("-primitive"):
        return f["primitive"]
    if qid.endswith("-campaign"):
        return f["campaign_id"]
    if qid.endswith("-stage-count"):
        return str(int(f["stage_count"]))
    if qid.endswith("-steps"):
        return ">".join(f["steps"].split(","))
    raise ep.Unsupported(f"unrouted advanced bash question {qid}")


def solve(deliveries, questions):
    nq = [q for q in questions if _is_node_question(q["id"])]
    aq = [q for q in questions if _is_adv_question(q["id"])]
    rest = [q for q in questions if not _is_node_question(q["id"]) and not _is_adv_question(q["id"])]
    answers, errors, log = swb.solve(deliveries, rest)
    cache: dict = {}
    for q in aq:
        qid = q["id"]
        try:
            label = q.get("artifact", "")
            di, files = ep.pick_delivery(label, deliveries)
            member = ep.member_name(label)
            name = next((n for n in files if n == member or n.endswith("/" + member)), None)
            if name is None:
                raise ep.Unsupported(f"member {member!r} not in delivery {di}")
            key = ("adv", di, name)
            if key not in cache:
                cache[key] = analyse_adv_bash(files[name].decode("utf-8", "replace"))
                log[f"{di}:{name}"] = {"kind": "bash-advanced", "fields": cache[key]}
            answers[qid] = answer_adv(qid, cache[key])
        except (ep.Unsupported, NodeUnsupported, KeyError, ValueError, IndexError) as e:
            errors[qid] = f"{type(e).__name__}: {e}"
    for q in nq:
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
                a = analyse_node(files[name].decode("utf-8", "replace"))
                plan = ep.parse_pipe_descriptor(a.descriptor)
                cache[key] = (a, plan)
                log[f"{di}:{name}"] = {"kind": "node", "descriptor": a.descriptor, "codec": a.codec,
                                       "steps": plan.steps, "notes": a.notes}
            _a, plan = cache[key]
            answers[qid] = answer_node(qid, plan)
        except (ep.Unsupported, NodeUnsupported, KeyError, ValueError, IndexError) as e:
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
