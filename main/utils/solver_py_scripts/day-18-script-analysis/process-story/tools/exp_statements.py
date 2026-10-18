"""Print each batch-*-expansion question's artifact `set` statements next to an oracle verdict.

usage: python3 -I exp_statements.py RUN_DIR EXTRACT_DIR VERDICTS.txt
EXTRACT_DIR holds one folder per evidence zip (7z x -pinfected); VERDICTS: "<qid> <verdict>" lines.
"""
import json, os, re, sys

run, ex, verdicts = sys.argv[1:4]
qs = {q["id"]: q for q in json.load(open(os.path.join(run, "start.json")))["questions"]}
for line in open(verdicts):
    qid, verdict = line.split()
    zname, _, member = (s.strip() for s in qs[qid]["artifact"].partition("->"))
    zname = zname.split(": ")[-1]
    path = os.path.join(ex, zname[:-4] if zname.endswith(".zip") else zname, member)
    text = open(path, encoding="utf-8", errors="replace").read()
    sets = [l.strip() for l in text.splitlines() if re.search(r"\bset\b", l, re.I) and re.search(r"[%!]", l)]
    print(f"## {qid}  [{verdict}]  {zname} -> {member}")
    for l in sets:
        print("   ", l[:180])
