"""Phase 0 protocol probe for the CourseStack proxy (run on the jumpbox).

Measures a fresh start/retry, the download shape(s), per-download timings, a partial submit and the final submit,
and saves every raw response under OUT/. Answers come from the proven Python solvers copied next to this file as
mobile_solve.py / warmups_solve.py.

  python3 -I phase0_probe.py mobile  OUT   # untimed regular retry, mobile-android-day-13-keystore-misuse/mastery
  python3 -I phase0_probe.py warmups OUT   # TIMED retry (run_for_time), warmups-day-13/mastery
"""
import gzip, http.client, io, json, os, re, sys, time, zipfile
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
HOST, BASE = "10.0.0.219", "/huntress-ctf-proxy"
T0 = time.perf_counter()
LOG = []


def now():
    return time.perf_counter() - T0


def call(method, query="", body=None, label=""):
    """One request on a fresh connection; returns (status, headers, body, ttfb, total)."""
    c = http.client.HTTPConnection(HOST, 80, timeout=60)
    path = BASE + (("?" + query) if query else "")
    hdrs = {"Accept-Encoding": "gzip"}
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        hdrs["Content-Type"] = "application/json"
    t = time.perf_counter()
    c.request(method, path, body=data, headers=hdrs)
    r = c.getresponse()
    ttfb = time.perf_counter() - t
    raw = r.read()
    total = time.perf_counter() - t
    if r.getheader("Content-Encoding", "") == "gzip":
        raw = gzip.decompress(raw)
    c.close()
    rec = {"label": label, "method": method, "query": query[:80], "status": r.status, "ttfb": round(ttfb, 3),
           "total": round(total, 3), "bytes": len(raw), "at": round(now(), 3),
           "headers": {k: v for k, v in r.getheaders() if k.lower() in
                       ("content-type", "content-length", "content-encoding", "content-disposition", "transfer-encoding")}}
    LOG.append(rec)
    print(json.dumps(rec), flush=True)
    return r.status, dict(r.getheaders()), raw, ttfb, total


def cap(link):
    m = re.search(r"[0-9a-f]{64}", link)
    return m.group(0) if m else None


def shape(st):
    """Summarise every place the start response offers downloads."""
    s = {"keys": list(st), "top_download": "download" in st, "downloads": len(st.get("downloads") or []),
         "artifacts": len(st.get("artifacts") or [])}
    s["artifact_kinds"] = sorted({a.get("kind", "-") for a in st.get("artifacts") or [] if isinstance(a, dict)})
    s["artifact_sample"] = (st.get("artifacts") or [None])[:2]
    s["question_artifacts"] = [q.get("artifact") for q in st.get("questions", [])[:3]]
    s["n_questions"] = len(st.get("questions", []))
    for k in ("deadline", "started_at", "run_for_time", "hall_of_fame_eligible", "mode", "variant_fresh",
              "unique_attempts_remaining", "ranked_attempts_remaining", "regular_variants_remaining"):
        s[k] = st.get(k)
    return s


def fetch_parallel(links, label):
    t = time.perf_counter()
    with ThreadPoolExecutor(max(1, len(links))) as ex:
        res = list(ex.map(lambda l: call("GET", "op=download&capability=" + cap(l), label=label)[2], links))
    print(f"[{label}] {len(links)} downloads in {time.perf_counter() - t:.3f}s", flush=True)
    return res


def submit(cid, answers, label):
    st, _, raw, *_ = call("POST", body={"op": "submit", "challenge": cid, "answers": answers}, label=label)
    res = json.loads(raw)
    res.pop("guidance", None), res.pop("questions", None)
    print(f"[{label}] -> {json.dumps(res)[:600]}", flush=True)
    return res


def save(out, name, data):
    with open(os.path.join(out, name), "wb" if isinstance(data, bytes) else "w") as f:
        f.write(data)


def run_mobile(out):
    import mobile_solve as ms
    cid = "mobile-android-day-13-keystore-misuse/mastery"
    _, _, raw, *_ = call("POST", body={"op": "retry", "challenge": cid}, label="retry(regular)")
    st = json.loads(raw)
    save(out, "start.json", raw)
    print("[shape]", json.dumps(shape(st)), flush=True)
    links = st.get("downloads") or ([st["download"]] if "download" in st else [])
    blobs = fetch_parallel(links, "download")
    for i, b in enumerate(blobs):
        save(out, f"delivery{i + 1}.apk", b)
    answers = {}
    for q in st["questions"]:
        n = int(re.match(r"delivery (\d+):", q["artifact"]).group(1))
        with zipfile.ZipFile(io.BytesIO(blobs[n - 1])) as z:
            puzzle = json.loads(z.read("assets/huntress_puzzle.json"))
        v = ms.decode(puzzle)
        assert ms.ANS_RE.fullmatch(v), v
        answers[q["id"]] = v.decode()
    ids = list(answers)
    r1 = submit(cid, {ids[0]: answers[ids[0]]}, "submit-partial(1)")
    r2 = submit(cid, {k: answers[k] for k in ids[1:]}, "submit-rest")
    save(out, "answers.json", json.dumps({"answers": answers, "partial": r1, "final": r2}, indent=1))


def run_warmups(out):
    import warmups_solve as ws
    cid = "warmups-day-13/mastery"
    _, _, raw, *_ = call("POST", body={"op": "retry", "challenge": cid, "run_for_time": True}, label="retry(timed)")
    st = json.loads(raw)
    save(out, "start.json", raw)
    print("[shape]", json.dumps(shape(st)), flush=True)
    qs = st["questions"]
    texts = {}
    bundle_links = [a["download"] for a in st.get("artifacts") or [] if a.get("kind") == "bundle"]
    if "download" in st and st["download"] not in bundle_links:
        bundle_links.append(st["download"])
    for i, l in enumerate(bundle_links):  # bundle(s) first, timed on their own
        b = fetch_parallel([l], f"bundle{i + 1}")[0]
        save(out, f"bundle{i + 1}.bin", b)
        if b[:2] == b"PK":
            with zipfile.ZipFile(io.BytesIO(b)) as z:
                print(f"[bundle{i + 1}] members: {z.namelist()[:5]} ... ({len(z.namelist())})", flush=True)
                for n in z.namelist():
                    texts.setdefault(os.path.basename(n), z.read(n).decode())
    per_q = st.get("downloads") or []
    if per_q:
        blobs = fetch_parallel(per_q, "per-question")
        for q, b in zip(qs, blobs):
            texts[q["artifact"]] = b.decode()
    for d in (0, 1):  # op=artifact&delivery=N
        call("GET", f"op=artifact&challenge={cid}&delivery={d}", label=f"artifact-delivery{d}")
    answers = {}
    for q in qs:
        key = q["artifact"] if q["artifact"] in texts else os.path.basename(q["artifact"].split(":")[-1].strip())
        answers[q["id"]] = ws.solve(texts[key])
    ids = list(answers)
    half = len(ids) // 2
    r1 = submit(cid, {k: answers[k] for k in ids[:half]}, "submit-partial(half)")
    r2 = submit(cid, {k: answers[k] for k in ids[half:]}, "submit-rest")
    save(out, "answers.json", json.dumps({"answers": answers, "partial": r1, "final": r2}, indent=1))


if __name__ == "__main__":
    which, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    try:
        {"mobile": run_mobile, "warmups": run_warmups}[which](out)
    finally:
        save(out, "timings.json", json.dumps(LOG, indent=1))
