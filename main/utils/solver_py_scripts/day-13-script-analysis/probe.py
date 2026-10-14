#!/usr/bin/env python3
"""Per-field oracle: submit a subset of answers.json to a regular (untimed) attempt; print accepted_ids/remaining."""
import json, os, sys, time, http.client
chal, answers_path = sys.argv[1], sys.argv[2]
ids = json.load(open(sys.argv[3])) if len(sys.argv) > 3 else None
ans = json.load(open(answers_path))
sub = {k: ans[k] for k in (ids or ans)}
c = http.client.HTTPSConnection("2026.huntress.ctf.games", timeout=60)
c.request("POST", "/", json.dumps({"op": "submit", "challenge": chal, "answers": sub}),
          {"Authorization": "Bearer " + os.environ["HUNTRESS_GM_TOKEN"], "Content-Type": "application/json"})
r = json.loads(c.getresponse().read())
for k in ("guidance", "questions", "prompt"): r.pop(k, None)
r["n_accepted"] = len(r.get("accepted_ids", [])); r.pop("accepted_ids", None); print(json.dumps(r)[:600])
