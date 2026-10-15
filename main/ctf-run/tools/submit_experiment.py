#!/usr/bin/env python3
"""Submit-leg experiments on an UNTIMED attempt (never point this at a live timed attempt).

  submit_experiment.py gzip-probe CHALLENGE              # start reconnect with a gzip body vs plain
  submit_experiment.py full CHALLENGE ANSWERS [--gzip]   # one submit of every answer
  submit_experiment.py parallel CHALLENGE ANSWERS N      # N concurrent partial submits (equal chunks)

ANSWERS = a JSON object {answer_id: value} (ctf-run's stdout). Token: $HUNTRESS_GM_TOKEN or ~/.huntress_token.
"""
import gzip, json, os, sys, time, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

URL = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games")
TOKEN = os.environ.get("HUNTRESS_GM_TOKEN") or open(os.path.expanduser("~/.huntress_token")).read().strip()


def post(payload, gz=False):
    body = json.dumps(payload, separators=(",", ":")).encode()
    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {TOKEN}", "Accept-Encoding": "identity"}
    if gz:
        body = gzip.compress(body, 6)
        headers["Content-Encoding"] = "gzip"
    t = time.time()
    try:
        with urllib.request.urlopen(urllib.request.Request(URL + "/", body, headers), timeout=120) as r:
            status, data = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, data = e.code, e.read()
    ms = (time.time() - t) * 1000
    try:
        out = json.loads(data)
    except ValueError:
        out = {"raw": data[:200].decode(errors="replace")}
    return status, ms, len(body), out


def brief(o):
    return {k: o.get(k) for k in ("result", "remaining", "error", "message", "attempt_id", "raw") if k in o}


def main():
    cmd, challenge = sys.argv[1], sys.argv[2]
    if cmd == "gzip-probe":
        for gz in (False, True):
            s, ms, n, o = post({"op": "start", "challenge": challenge}, gz)
            print(f"gzip={gz} HTTP {s} {ms:.0f} ms body {n} B -> {brief(o)}")
        return
    answers = json.load(open(sys.argv[3]))
    answers = answers.get("answers", answers)
    if cmd == "full":
        gz = "--gzip" in sys.argv
        s, ms, n, o = post({"op": "submit", "challenge": challenge, "answers": answers}, gz)
        print(f"full gzip={gz}: {len(answers)} answers, body {n} B, HTTP {s} {ms:.0f} ms -> {brief(o)}")
    elif cmd == "parallel":
        n = int(sys.argv[4])
        items = list(answers.items())
        size = -(-len(items) // n)
        parts = [dict(items[i : i + size]) for i in range(0, len(items), size)]
        t = time.time()
        with ThreadPoolExecutor(n) as ex:
            res = list(ex.map(lambda p: post({"op": "submit", "challenge": challenge, "answers": p}), parts))
        for p, (s, ms, nb, o) in zip(parts, res):
            print(f"  part {len(p)} answers, HTTP {s} {ms:.0f} ms -> {brief(o)}")
        print(f"parallel x{n}: wall {(time.time() - t) * 1000:.0f} ms")


if __name__ == "__main__":
    main()
