#!/usr/bin/env python3
"""One-process mastery run (27 s ranked deadline, ~5 x 31 MB deliveries -> bandwidth is the limit).

start -> parallel streaming downloads -> as each delivery lands: 7z extract (nested zips unpacked recursively) ->
solve.py on every dir that holds a `d01--*` specimen -> one batched submit with `setN.<id>` keys.

usage:  mastery_run.py [start|retry|practice]            (needs HUNTRESS_GM_TOKEN; run with `python3 -I`)
        mastery_run.py --local <delivery.zip> [<delivery.zip> ...]   offline: extract+solve only, prints timings
Everything the server returns is written to artifacts/mastery/run_<ts>/ (start.json first, deliveries, answers,
result) so a burned attempt still leaves a fixture. Never prints the token.
"""
import http.client, json, os, re, subprocess, sys, tempfile, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
HOST = "2026.huntress.ctf.games"
CH = "binary-reverse-engineering-day-08-swift-network-configuration/mastery"
T0 = time.perf_counter()


def lap(msg):
    print(f"[{time.perf_counter() - T0:6.2f}s] {msg}", file=sys.stderr, flush=True)


def extract(z, dest):
    subprocess.run(["7z", "x", "-y", "-pinfected", f"-o{dest}", str(z)], capture_output=True, check=True, stdin=subprocess.DEVNULL)
    for inner in sorted(dest.rglob("*.zip")):
        extract(inner, inner.with_suffix(""))


def natkey(p):
    return [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", str(p))]


def solve_delivery(zpath, work):
    """-> list of answer dicts, one per question set found in this delivery (in natural path order)."""
    d = Path(work) / (Path(zpath).stem + "-x")
    extract(zpath, d)
    dirs = sorted({p.parent for p in d.rglob("d01--*")}, key=natkey)
    out = []
    for sd in dirs:
        r = subprocess.run([sys.executable, "-I", str(HERE / "solve.py"), str(sd)], capture_output=True, text=True,
                           check=True, stdin=subprocess.DEVNULL)
        out.append(json.loads(r.stdout))
    lap(f"{Path(zpath).name}: {len(out)} set(s) solved")
    return out


class Client:
    def __init__(self, tok):
        self.tok = tok

    def req(self, method, path, body=None, sink=None):
        c = http.client.HTTPSConnection(HOST, timeout=60)
        h = {"Authorization": f"Bearer {self.tok}"}
        data = None
        if body is not None:
            data = json.dumps(body, separators=(",", ":")).encode()
            h["Content-Type"] = "application/json"
        c.request(method, path, body=data, headers=h)
        r = c.getresponse()
        if sink is None:
            return r.status, r.read()
        n = 0
        with open(sink, "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
                n += len(chunk)
        return r.status, n


def main():
    if sys.argv[1:2] == ["--local"]:
        with tempfile.TemporaryDirectory() as w:
            for z in sys.argv[2:]:
                res = solve_delivery(z, w)
                print(len(res), "set(s);", [len(a) for a in res], "answers each")
        return 0
    op = sys.argv[1] if len(sys.argv) > 1 else "start"
    tok = os.environ["HUNTRESS_GM_TOKEN"]
    cli = Client(tok)
    run = HERE / "artifacts" / "mastery" / f"run_{int(time.time())}"
    run.mkdir(parents=True)
    st, raw = cli.req("POST", "/", {"op": op, "challenge": CH})
    (run / "start.json").write_bytes(raw)
    start = json.loads(raw)
    lap(f"{op} -> HTTP {st}; deadline={start.get('deadline')}")
    links = start.get("downloads") or [a["download"] for a in start.get("artifacts", [])] or \
        ([start["download"]] if start.get("download") else [])
    if not links:
        print(json.dumps({k: v for k, v in start.items() if k not in ("guidance", "questions")})[:800])
        return 1
    qids = [q["id"] for q in start["questions"]]
    work = tempfile.mkdtemp(prefix="mastery-", dir=run)

    def job(i):
        p = run / f"delivery{i + 1}.zip"
        code, n = cli.req("GET", links[i], sink=p)
        lap(f"download {i + 1}/{len(links)}: HTTP {code}, {n} bytes")
        if code != 200:
            raise RuntimeError(f"download {i + 1} HTTP {code}")
        return solve_delivery(p, Path(work) / str(i))

    with ThreadPoolExecutor(len(links)) as ex:
        per_delivery = list(ex.map(job, range(len(links))))
    sets = [a for res in per_delivery for a in res]  # delivery order, then natural path order within a delivery
    answers = {}
    for n, a in enumerate(sets, 1):
        for k, v in a.items():
            answers[f"set{n}.{k}"] = v
    missing = [q for q in qids if q not in answers]
    extra = [k for k in answers if k not in set(qids)]
    lap(f"{len(answers)} answers, {len(missing)} missing, {len(extra)} extra")
    (run / "answers.json").write_text(json.dumps(answers, indent=1))
    if missing or extra:
        print("id mismatch; submitting only the matching keys", missing[:3], extra[:3], file=sys.stderr)
        answers = {k: v for k, v in answers.items() if k in set(qids)}
    st, raw = cli.req("POST", "/", {"op": "submit", "challenge": CH, "answers": answers})
    res = json.loads(raw)
    res.pop("guidance", None)
    res.pop("questions", None)
    (run / "result.json").write_text(json.dumps(res, indent=1))
    lap(f"submit -> HTTP {st}")
    print(json.dumps(res)[:1500])
    return 0 if res.get("result") == "CORRECT" else 1


if __name__ == "__main__":
    sys.exit(main())
