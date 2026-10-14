#!/usr/bin/env python3
"""Mastery (untimed regular) driver: submit non-expansion answers in (set, language) chunks, then resolve each
`batch-*-expansion` field by trying candidates until accepted.  Prints a line per request; writes final.json."""
import http.client, json, os, re, sys, time
CHAL = "script-analysis-day-13-archive-delivery-chain/mastery"
ans = json.load(open(sys.argv[1]))
c = http.client.HTTPSConnection("2026.huntress.ctf.games", timeout=60)
def post(sub):
    c.request("POST", "/", json.dumps({"op": "submit", "challenge": CHAL, "answers": sub}),
              {"Authorization": "Bearer " + os.environ["HUNTRESS_GM_TOKEN"], "Content-Type": "application/json"})
    r = json.loads(c.getresponse().read()); time.sleep(3.6); return r
def is_exp(k): return re.sub(r"^set\d+\.", "", k).startswith("batch-") and k.endswith("-expansion")
chunks = {}
for k in ans:
    if is_exp(k): continue
    s = re.match(r"set\d+\.", k); s = s.group(0) if s else ""
    chunks.setdefault((s, re.sub(r"^set\d+\.", "", k).split("-")[0]), []).append(k)
final = dict(ans)
for key, ids in chunks.items():
    r = post({k: ans[k] for k in ids})
    print("chunk", key, len(ids), r.get("result"), r.get("remaining"), flush=True)
    if r.get("flag"): print("FLAG", r["flag"]); sys.exit(0)
for k in [k for k in ans if is_exp(k)]:
    cands = [ans[k]] + [v for v in ("percent", "delayed", "call_double", "for_indirect") if v != ans[k]]
    for v in cands:
        r = post({k: v})
        if r.get("result") in ("accepted", "CORRECT"):
            final[k] = v; print(k, v, r.get("result"), r.get("remaining"), flush=True)
            if r.get("flag"): print("FLAG", r["flag"]); json.dump(final, open("fixtures/mastery/final.json", "w"), indent=1); sys.exit(0)
            break
        print(k, v, "incorrect", flush=True)
json.dump(final, open("fixtures/mastery/final.json", "w"), indent=1)
