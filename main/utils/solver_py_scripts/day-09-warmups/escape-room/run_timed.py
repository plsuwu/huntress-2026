#!/usr/bin/env python3
"""Full-automation runner for the timed modes (deadline is only ~6-7s, so
start -> download -> solve -> submit must happen in ONE process).

usage: run_timed.py <time_trial|mastery> [--retry] [--untimed] [--challenge warmups-day-08]
Token is read from HUNTRESS_GM_TOKEN (source ../../../../.env first). Run with `python3 -I`.
Every fetched variant is dumped to artifacts/<mode>-<attempt>/ for offline debugging.
"""
import io, json, os, sys, time, urllib.request, zipfile
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
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


def fetch_blobs(st):
    """Return {artifact_name: text}; handles zip / single file / per-question `downloads` list."""
    if "downloads" in st:
        with ThreadPoolExecutor(len(st["downloads"])) as ex:
            raws = list(ex.map(req, st["downloads"]))
        return {q["artifact"]: r.decode() for q, r in zip(st["questions"], raws)}
    raw = req(st["download"])
    if raw[:2] == b"PK":
        z = zipfile.ZipFile(io.BytesIO(raw))
        return {n: z.read(n).decode() for n in z.namelist()}
    return {st["questions"][0]["artifact"]: raw.decode()}


def main():
    mode = sys.argv[1]
    base_id = sys.argv[sys.argv.index("--challenge") + 1] if "--challenge" in sys.argv else "warmups-day-08"
    cid = f"{base_id}/{mode}"
    op = "retry" if "--retry" in sys.argv else "start"
    t0 = time.time()
    body = {"op": op, "challenge": cid}
    if "--untimed" not in sys.argv:
        body["run_for_time"] = True  # run_for_time_supported routes: plain start is a regular untimed attempt
    st = json.loads(req("/", body))
    if "download" not in st and "downloads" not in st:
        print(json.dumps({k: v for k, v in st.items() if k != "guidance"}, indent=1)); return
    blobs = fetch_blobs(st)
    print("downloaded", len(blobs), "files", round(time.time() - t0, 2), "s", st.get("deadline"))
    answers = {}
    for q in st["questions"]:
        try:
            answers[q["id"]] = solve(blobs[q["artifact"]])
        except Exception as e:  # never crash mid-clock; partial answers still bank (accepted/remaining)
            print("solve failed", q["id"], q["artifact"], repr(e)); answers[q["id"]] = ""
    res = json.loads(req("/", {"op": "submit", "challenge": cid, "answers": answers}))
    print(round(time.time() - t0, 2), "s")
    res.pop("guidance", None); res.pop("questions", None)
    print(json.dumps(res, indent=1))
    d = os.path.join(HERE, "artifacts", f"{mode}-{st.get('attempt_id', 'x')[:8]}")  # after submit: don't burn the clock
    os.makedirs(d, exist_ok=True)
    for i, (n, t) in enumerate(blobs.items()):
        open(os.path.join(d, f"{i:02d}-{n}"), "w").write(t)
    json.dump({"answers": answers, "result": res}, open(os.path.join(d, "answers.json"), "w"), indent=1)

main()
