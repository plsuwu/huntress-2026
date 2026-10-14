#!/usr/bin/env python3
"""Full-automation runner for the timed modes (deadline is only ~6s for time_trial,
so start -> download -> solve -> submit must happen in ONE process).

usage: run_timed.py <time_trial|mastery> [--retry]
Token is read from HUNTRESS_GM_TOKEN (source ../../../../.env first).
"""
import io, json, os, sys, time, urllib.request, zipfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from solve import solve

BASE = "https://2026.huntress.ctf.games"
TOK = os.environ["HUNTRESS_GM_TOKEN"]


def req(path, body=None):
    r = urllib.request.Request(BASE + path, headers={"Authorization": f"Bearer {TOK}"})
    if body is not None:
        r.data = json.dumps(body).encode()
        r.add_header("Content-Type", "application/json")
        r.method = "POST"
    with urllib.request.urlopen(r) as f:
        return f.read()


def main():
    mode = sys.argv[1]
    cid = f"warmups-day-05/{mode}"
    op = "retry" if "--retry" in sys.argv else "start"
    t0 = time.time()
    st = json.loads(req("/", {"op": op, "challenge": cid}))
    if "download" not in st and "downloads" not in st:
        print(json.dumps({k: v for k, v in st.items() if k != "guidance"}, indent=1)); return
    if "downloads" in st:  # retry-style response: one download per question, in question order
        from concurrent.futures import ThreadPoolExecutor
        with ThreadPoolExecutor(len(st["downloads"])) as ex:
            raws = list(ex.map(req, st["downloads"]))
        blobs = {q["artifact"]: r.decode() for q, r in zip(st["questions"], raws)}
        print("downloaded", len(raws), "files", round(time.time() - t0, 2), "s", st.get("deadline"))
    else:
        raw = req(st["download"])
        print("download done", round(time.time() - t0, 2), "s", len(raw), "bytes", st.get("deadline"))
        if raw[:2] == b"PK":  # mastery: zip of blob-NN.txt
            z = zipfile.ZipFile(io.BytesIO(raw))
            blobs = {n: z.read(n).decode() for n in z.namelist()}
        else:
            blobs = {st["questions"][0]["artifact"]: raw.decode()}
    answers = {q["id"]: solve(blobs[q["artifact"]]) for q in st["questions"]}
    res = json.loads(req("/", {"op": "submit", "challenge": cid, "answers": answers}))
    res.pop("guidance", None); res.pop("questions", None)
    print(round(time.time() - t0, 2), "s")
    print(json.dumps(res, indent=1))

main()
