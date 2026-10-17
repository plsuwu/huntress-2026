"""Run one network probe burst on the jumpbox and append the result to a local JSONL log.

Pipes remote_probe.py to the jumpbox's python3 over ssh (nothing is deployed there), stamps the record with local
time and the ssh wall time, and appends one line to OUT. ssh/VPN failures are logged as {"error": ...} records, not
dropped - an outage window is data too. Driven every 10 min by the systemd user timer from install-timer.sh.

Before the ssh, a local baseline goes under "local": ping (loss/rtt/jitter) to public anycast resolvers and to the
jumpbox over the VPN, plus fresh-connection HTTPS timings to Cloudflare and Google. With the jumpbox's own
ref_cloudflare control this separates "our network is bad" from "the Game Master is slow".

  python3 collect.py [--out ctf-run/runs/netprobe/probes.jsonl] [--burst 5] [--gap 0.25] [--challenge ID]
"""
import argparse, fcntl, http.client, json, os, re, socket, ssl, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
KEY = os.path.join(REPO, "huntress-id_ed25519")
HOST = "HuntressCTF@10.1.181.90"
DEFAULT_OUT = os.path.join(REPO, "ctf-run", "runs", "netprobe", "probes.jsonl")
PING_TARGETS = {"cloudflare": "1.1.1.1", "google": "8.8.8.8", "jumpbox": HOST.split("@")[1]}
HTTPS_TARGETS = {"cloudflare": ("1.1.1.1", "/cdn-cgi/trace"), "google": ("www.google.com", "/generate_204")}
PING_LOSS = re.compile(r"(\d+) packets transmitted, (\d+) received")
PING_RTT = re.compile(r"= ([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+) ms")  # may be followed by ", pipe N"


def ping(host, count):
    """ping summary: loss %, rtt min/avg/max/mdev in ms (rtt absent when every packet was lost)."""
    try:
        p = subprocess.run(["ping", "-n", "-q", "-c", str(count), "-i", "0.2", "-W", "2", host],
                           capture_output=True, text=True, timeout=count * 2 + 5)
    except (OSError, subprocess.TimeoutExpired) as e:
        return {"error": f"{type(e).__name__}: {e}"[:200]}
    m, rtt = PING_LOSS.search(p.stdout), PING_RTT.search(p.stdout)
    if not m:
        return {"error": (p.stderr or p.stdout).strip()[-200:]}
    sent, recv = int(m.group(1)), int(m.group(2))
    r = {"loss": round(100 * (sent - recv) / max(1, sent), 1)}
    if rtt:
        r.update(zip(("min", "avg", "max", "mdev"), map(float, rtt.groups())))
    return r


def https_get(host, path, idx):
    """One HTTPS GET on a fresh connection, split into TCP connect / TLS / TTFB (seconds from start)."""
    rec = {"i": idx}
    t0 = time.perf_counter()
    try:
        sock = socket.create_connection((host, 443), timeout=10)
        rec["tcp"] = round(time.perf_counter() - t0, 4)
        sock = ssl.create_default_context().wrap_socket(sock, server_hostname=host)
        rec["tls"] = round(time.perf_counter() - t0, 4)
        c = http.client.HTTPSConnection(host, 443, timeout=10)
        c.sock = sock
        c.request("GET", path, headers={"Accept-Encoding": "identity"})
        r = c.getresponse()
        rec["ttfb"] = round(time.perf_counter() - t0, 4)
        r.read()
        rec["status"] = r.status
        c.close()
    except (OSError, http.client.HTTPException) as e:
        rec["error"] = f"{type(e).__name__}: {e}"[:200]
    rec["total"] = round(time.perf_counter() - t0, 4)
    return rec


def local_baseline(n):
    with ThreadPoolExecutor(len(PING_TARGETS)) as ex:  # pings run side by side; HTTPS after, one at a time
        pings = dict(zip(PING_TARGETS, ex.map(lambda h: ping(h, n * 2), PING_TARGETS.values())))
    https = {name: [https_get(h, path, i) for i in range(n)] for name, (h, path) in HTTPS_TARGETS.items()}
    return {"ping": pings, "https": https}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--burst", type=int, default=5)
    ap.add_argument("--gap", type=float, default=0.25)
    ap.add_argument("--challenge", default="warmups-day-01", help="a SOLVED challenge (junk submits must be unscored)")
    ap.add_argument("--answer-id", default="message", help="an answer id of --challenge, for the junk submit")
    ap.add_argument("--local-n", type=int, default=3, help="local HTTPS requests per target (pings: 2x this)")
    ap.add_argument("--timeout", type=int, default=90, help="hard cap on the whole ssh run (seconds)")
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)

    lock = open(a.out + ".lock", "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print("previous probe still running; skipping", file=sys.stderr)
        return

    rec = {"ts": round(time.time(), 3)}
    try:
        rec["local"] = local_baseline(a.local_n)
    except Exception as e:  # the baseline must never cost us the remote burst
        rec["local"] = {"error": f"{type(e).__name__}: {e}"[:200]}
    cmd = ["ssh", "-i", KEY, "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10",
           HOST, "python3", "-I", "-", str(a.burst), str(a.gap), a.challenge, a.answer_id]
    t0 = time.perf_counter()
    try:
        with open(os.path.join(HERE, "remote_probe.py"), "rb") as src:
            p = subprocess.run(cmd, stdin=src, capture_output=True, timeout=a.timeout)
        rec["ssh_wall"] = round(time.perf_counter() - t0, 3)
        line = p.stdout.decode(errors="replace").strip().splitlines()
        if p.returncode == 0 and line:
            rec.update(json.loads(line[-1]))
        else:
            rec["error"] = f"ssh rc={p.returncode}: {p.stderr.decode(errors='replace').strip()[-300:]}"
    except subprocess.TimeoutExpired:
        rec["ssh_wall"] = round(time.perf_counter() - t0, 3)
        rec["error"] = f"ssh timeout after {a.timeout}s"
    except (OSError, ValueError) as e:
        rec["ssh_wall"] = round(time.perf_counter() - t0, 3)
        rec["error"] = f"{type(e).__name__}: {e}"[:300]

    with open(a.out, "a") as f:
        f.write(json.dumps(rec, separators=(",", ":")) + "\n")
    print(rec.get("error") or rec.get("skipped") or f"ok ({rec['ssh_wall']}s)")


if __name__ == "__main__":
    main()
