#!/usr/bin/env python3
"""Download-latency probe on UNTIMED regular attempts (never on a live timed attempt).

  download_probe.py CHALLENGE[/mode] COPIES [alt|cap]   (alt: alternate capability/op=artifact; cap: capability only)

Pre-opens TLS connections, sends a regular `retry` (no run_for_time), then fetches every delivery COPIES
times in parallel, alternating the capability link and `op=artifact&delivery=N`, and prints each
request's time to first byte relative to the retry response - i.e. what hedged downloads would buy.
The caller completes the attempt afterwards (e.g. `ctf-run <challenge> -m`) so the next retry advances.
Token: $HUNTRESS_GM_TOKEN or ~/.huntress_token.
"""
import http.client, json, os, ssl, sys, threading, time

HOST = "2026.huntress.ctf.games"
TOKEN = os.environ.get("HUNTRESS_GM_TOKEN") or open(os.path.expanduser("~/.huntress_token")).read().strip()
HDR = {"Authorization": f"Bearer {TOKEN}", "Accept-Encoding": "identity"}


def conn():
    c = http.client.HTTPSConnection(HOST, timeout=30, context=ssl.create_default_context())
    c.connect()
    return c


def main():
    challenge, copies = sys.argv[1], int(sys.argv[2])
    only_cap = len(sys.argv) > 3 and sys.argv[3] == "cap"
    pool = [conn() for _ in range(12)]
    c = pool.pop()
    t0 = time.time()
    c.request("POST", "/", json.dumps({"op": "retry", "challenge": challenge}), {**HDR, "Content-Type": "application/json"})
    r = c.getresponse()
    start = json.loads(r.read())
    t_start = time.time()
    if start.get("deadline"):
        sys.exit(f"refusing: attempt is timed (deadline {start['deadline']})")
    links = [a["download"] for a in start.get("artifacts") or []] or start.get("downloads") or [start["download"]]
    jobs = []
    for i, link in enumerate(links):
        for k in range(copies):
            art = k % 2 == 1 and not only_cap
            path = f"/?op=artifact&challenge={challenge}&delivery={i}" if art else link
            jobs.append((i, k, "art" if art else "cap", path))
    results = []

    def fetch(job, cn):
        i, k, kind, path = job
        cn.request("GET", path, headers=HDR)
        resp = cn.getresponse()
        first = time.time()
        body = resp.read()
        results.append((i, k, kind, resp.status, len(body), (first - t_start) * 1000))

    th = [threading.Thread(target=fetch, args=(j, pool[n])) for n, j in enumerate(jobs)]
    for t in th:
        t.start()
    for t in th:
        t.join()
    print(f"retry {(t_start - t0) * 1000:.0f} ms, attempt {start.get('attempt_id', '')[:8]}, {len(links)} deliveries")
    by, first = {}, {}
    for i, k, kind, st, n, ms in sorted(results):
        by.setdefault(i, []).append(ms)
        if k == 0:
            first[i] = ms  # copy 0 = the single capability request an unhedged client sends
        print(f"  delivery {i} copy {k}: {kind} HTTP {st} {n} B first byte +{ms:.0f} ms")
    worst_single = max(first.values())
    worst_hedged = max(min(v) for v in by.values())
    print(f"last delivery: capability only +{worst_single:.0f} ms, hedged (min of {copies}) +{worst_hedged:.0f} ms")


if __name__ == "__main__":
    main()
