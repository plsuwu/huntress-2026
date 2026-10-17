"""Network probe burst, run ON THE JUMPBOX (stdlib only, piped over ssh by collect.py - nothing is deployed).

Prints exactly one JSON object on stdout: per-request timings for a short sequential burst against each target, plus
jumpbox context (loadavg, CPU steal over the burst). Every request is on a fresh connection, like a ranked run's
first calls, and none of them changes game state. Targets (round-robin, in this order):
  proxy_status   GET  proxy ?op=status&challenge=C  - proxy -> Game Master backend
  proxy_download GET  proxy ?op=download&capability= - C's (solved) artifact; the download leg of a timed run
  proxy_submit   POST proxy {"op":"submit", C, junk} - C is solved, so the GM answers "already_solved" ("Further
                 submissions are not scored"). Only the result keyword is kept; any other result stops the submits.
  proxy_edge     GET  proxy ?op=openapi              - answered at CourseStack/Vercel, no Game Master work (~70 ms)
  direct_status  GET  https://2026.huntress.ctf.games/?op=status (bearer ~/.huntress_token) - backend, no proxy
  ref_cloudflare GET  https://1.1.1.1/cdn-cgi/trace  - control: the jumpbox's own internet path, no Huntress at all
Skips the burst entirely (emits {"skipped": ...}) while a ranked campaign or ctf-run is running on the box.

  python3 -I - < remote_probe.py [BURST] [GAP_S] [CHALLENGE] [ANSWER_ID]
"""
import http.client, json, os, re, socket, subprocess, sys, time

BURST = int(sys.argv[1]) if len(sys.argv) > 1 else 5
GAP = float(sys.argv[2]) if len(sys.argv) > 2 else 0.25
CHALLENGE = sys.argv[3] if len(sys.argv) > 3 else "warmups-day-01"
ANSWER_ID = sys.argv[4] if len(sys.argv) > 4 else "message"
TIMEOUT = 20
PROXY_HOST, PROXY = "10.0.0.219", "/huntress-ctf-proxy"
SAFE_SUBMIT = {"already_solved"}
BUSY_SCRIPTS = ("tools/hof_campaign", "tools/campaign.py")


def token():
    try:
        with open(os.path.expanduser("~/.huntress_token")) as f:
            return f.read().strip()
    except OSError:
        return None


class Target:
    def __init__(self, name, host, port, path, tls=False, method="GET", body=None, hdrs=None):
        self.name, self.host, self.port, self.path, self.tls = name, host, port, path, tls
        self.method, self.body, self.hdrs, self.off = method, body, hdrs or {}, None


def targets():
    # proxy_status goes first: its i=0 request is the one that sees a cold proxy, like a ranked run's start.
    junk = json.dumps({"op": "submit", "challenge": CHALLENGE, "answers": {ANSWER_ID: "netprobe-junk"}}).encode()
    t = [Target("proxy_status", PROXY_HOST, 80, f"{PROXY}?op=status&challenge={CHALLENGE}"),
         Target("proxy_download", PROXY_HOST, 80, None),  # path filled from the first status response
         Target("proxy_submit", PROXY_HOST, 80, PROXY, method="POST", body=junk,
                hdrs={"Content-Type": "application/json"}),
         Target("proxy_edge", PROXY_HOST, 80, f"{PROXY}?op=openapi")]
    tok = token()
    if tok:
        t.append(Target("direct_status", "2026.huntress.ctf.games", 443, f"/?op=status&challenge={CHALLENGE}",
                        tls=True, hdrs={"Authorization": f"Bearer {tok}"}))
    t.append(Target("ref_cloudflare", "1.1.1.1", 443, "/cdn-cgi/trace", tls=True))
    return t


def busy():
    """Ranked/timed work in progress: a campaign driver, or any ctf-run binary (however it was invoked)."""
    out = subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout.splitlines()[1:]
    return [l[:120] for l in out if l.split() and (os.path.basename(l.split()[0]) == "ctf-run"
                                                   or any(s in l for s in BUSY_SCRIPTS))]


def cpu_ticks():
    with open("/proc/stat") as f:
        v = [int(x) for x in f.readline().split()[1:]]
    return sum(v), v[7] if len(v) > 7 else 0  # total, steal


def one(t, idx):
    """One request on a fresh connection; times are seconds from request start. Returns (record, body)."""
    rec, body = {"i": idx}, b""
    t0 = time.perf_counter()
    try:
        cls = http.client.HTTPSConnection if t.tls else http.client.HTTPConnection
        c = cls(t.host, t.port, timeout=TIMEOUT)
        c.connect()  # TCP (+TLS for https targets)
        rec["connect"] = round(time.perf_counter() - t0, 4)
        c.request(t.method, t.path, body=t.body, headers={"Accept-Encoding": "identity", **t.hdrs})
        r = c.getresponse()
        rec["ttfb"] = round(time.perf_counter() - t0, 4)
        body = r.read()
        rec["total"] = round(time.perf_counter() - t0, 4)
        rec["status"], rec["bytes"] = r.status, len(body)
        ray, vid = r.getheader("Cf-Ray", ""), r.getheader("X-Vercel-Id", "")
        rec["colo"] = ray.rsplit("-", 1)[-1] if "-" in ray else None
        rec["vercel"] = "::".join(vid.split("::")[:-1]) or None  # region chain, minus the per-request id
        c.close()
    except (OSError, http.client.HTTPException, socket.timeout) as e:
        rec["total"] = round(time.perf_counter() - t0, 4)
        rec["error"] = f"{type(e).__name__}: {e}"[:200]
    return rec, body


def capability(status_body):
    m = re.search(rb'/download/([0-9a-f]{64})', status_body)
    return m.group(1).decode() if m else None


def main():
    out = {"v": 2, "host": socket.gethostname(), "remote_ts": round(time.time(), 3), "burst": BURST, "gap": GAP,
           "challenge": CHALLENGE, "loadavg": os.getloadavg()}
    b = busy()
    if b:
        out["skipped"], out["busy"] = "campaign", b
        print(json.dumps(out))
        return
    tot0, steal0 = cpu_ticks()
    t_start = time.perf_counter()
    tl = targets()
    res = {t.name: [] for t in tl}
    # Interleave targets round-robin so a transient spike hits every series, not just one.
    for i in range(BURST):
        for t in tl:
            if t.off:
                continue
            if t.path is None:  # proxy_download, until a status response names the capability
                continue
            rec, body = one(t, i)
            if t.name == "proxy_status" and "proxy_download" in res:
                cap = capability(body)
                for d in tl:
                    if d.name == "proxy_download" and cap and d.path is None:
                        d.path = f"{PROXY}?op=download&capability={cap}"
            if t.name == "proxy_submit" and "error" not in rec:
                try:
                    rec["result"] = json.loads(body).get("result")
                except ValueError:
                    rec["result"] = None
                if rec["result"] not in SAFE_SUBMIT:  # not provably unscored - never submit again this burst
                    t.off = f"unexpected submit result {rec['result']!r} (status {rec.get('status')})"
            res[t.name].append(rec)
            time.sleep(GAP)
    tot1, steal1 = cpu_ticks()
    out["elapsed"] = round(time.perf_counter() - t_start, 3)
    out["steal_pct"] = round(100 * (steal1 - steal0) / max(1, tot1 - tot0), 2)
    out["results"] = res
    off = {t.name: t.off or "no capability in any status response" for t in tl if t.off or t.path is None}
    if off:
        out["disabled"] = off
    print(json.dumps(out))


if __name__ == "__main__":
    main()
