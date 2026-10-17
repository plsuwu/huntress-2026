"""Summarise the netprobe log by AEST hour of day: which hours have the fastest, steadiest Game Master?

Per hour bucket and target: n runs, warm TTFB median/p90/max (burst requests i>=1), cold TTFB median (i=0, the first
request after ~10 idle minutes - closest to a ranked run's start), and error rate (non-2xx, timeouts, refused).
Medians/percentiles, not means: the cold-start tail would swamp an average. Ends with the hours ranked by
--rank-target warm median (default proxy_download: downloads are the long pole of a timed run's window).

Controls: ref_cloudflare (jumpbox -> 1.1.1.1) is in the target table; the local section shows this machine's ping/HTTPS
baseline. --exclude-degraded drops bursts where either control was bad (local ping loss, or local/jumpbox Cloudflare
TTFB over --degraded-x times its own overall median), so slow buckets can't be blamed on our own network.

  python3 report.py [LOG] [--by hour|weekday-hour|day] [--target T] [--since 2026-10-18] [--min-n 3] [--exclude-degraded]
"""
import argparse, collections, datetime as dt, json, os, statistics, sys
from zoneinfo import ZoneInfo

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_LOG = os.path.abspath(os.path.join(HERE, "..", "..", "runs", "netprobe", "probes.jsonl"))
TZ = ZoneInfo("Australia/Brisbane")  # AEST, no DST; the 23:00 day release is in this zone


def pct(xs, q):
    if not xs:
        return None
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]


def ms(x):
    return "-" if x is None else f"{x * 1000:.0f}"


def bucket(ts, by):
    t = dt.datetime.fromtimestamp(ts, TZ)
    if by == "hour":
        return f"{t.hour:02d}:00"
    if by == "weekday-hour":
        return f"{t:%a} {t.hour:02d}:00"
    return f"{t:%Y-%m-%d}"


def warm_ttfb(reqs):
    return [q["ttfb"] for q in reqs if q["i"] > 0 and "ttfb" in q and "error" not in q]


def degraded_reason(r, ref_med, x):
    """Why this burst's own network looked bad, or None. ref_med: overall medians of the two Cloudflare controls."""
    loc = r.get("local") or {}
    for name, pg in (loc.get("ping") or {}).items():
        if pg.get("loss", 0) > 0 or "error" in pg:
            return f"local ping {name} loss {pg.get('loss', '?')}%"
    for key, xs in (("local", [q["ttfb"] for q in (loc.get("https") or {}).get("cloudflare", []) if "ttfb" in q]),
                    ("jumpbox", warm_ttfb(r.get("results", {}).get("ref_cloudflare", [])))):
        if xs and ref_med.get(key) and statistics.median(xs) > x * ref_med[key]:
            return f"{key} cloudflare ttfb {statistics.median(xs) * 1000:.0f} ms"
    return None


def load(path, since):
    recs = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            if since and dt.datetime.fromtimestamp(r["ts"], TZ).date() < since:
                continue
            recs.append(r)
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log", nargs="?", default=DEFAULT_LOG)
    ap.add_argument("--by", choices=("hour", "weekday-hour", "day"), default="hour")
    ap.add_argument("--target", default=None, help="only this target (proxy_status, proxy_edge, direct_status)")
    ap.add_argument("--rank-target", default="proxy_download")
    ap.add_argument("--exclude-degraded", action="store_true", help="drop bursts taken while our own network was bad")
    ap.add_argument("--degraded-x", type=float, default=2.0, help="control TTFB multiple that counts as degraded")
    ap.add_argument("--since", type=dt.date.fromisoformat, default=None, help="AEST date, inclusive")
    ap.add_argument("--min-n", type=int, default=3, help="min runs in a bucket to be ranked")
    a = ap.parse_args()

    recs = load(a.log, a.since)
    if not recs:
        sys.exit(f"no records in {a.log}")
    first, last = (dt.datetime.fromtimestamp(recs[i]["ts"], TZ) for i in (0, -1))
    runs = [r for r in recs if "results" in r]
    print(f"{a.log}: {len(recs)} records ({first:%Y-%m-%d %H:%M} .. {last:%Y-%m-%d %H:%M} AEST), "
          f"{len(runs)} bursts, {sum('error' in r for r in recs)} ssh/collector errors, "
          f"{sum('skipped' in r for r in recs)} skipped (campaign running)")

    ref_all = {"local": [q["ttfb"] for r in runs for q in (r.get("local") or {}).get("https", {}).get("cloudflare", [])
                         if "ttfb" in q],
               "jumpbox": [t for r in runs for t in warm_ttfb(r["results"].get("ref_cloudflare", []))]}
    ref_med = {k: statistics.median(v) for k, v in ref_all.items() if v}
    bad = {id(r): why for r in recs if (why := degraded_reason(r, ref_med, a.degraded_x))}
    print(f"degraded bursts (own network bad): {len(bad)}" + (" - excluded" if a.exclude_degraded and bad else ""))
    for r in recs:
        if id(r) in bad:
            print(f"  {dt.datetime.fromtimestamp(r['ts'], TZ):%Y-%m-%d %H:%M}  {bad[id(r)]}")
    if a.exclude_degraded:
        recs = [r for r in recs if id(r) not in bad]
    unscored = collections.Counter(q.get("result") for r in recs for q in r.get("results", {}).get("proxy_submit", []))
    if set(unscored) - {"already_solved"}:
        print(f"!! proxy_submit results other than already_solved: {dict(unscored)} - check --challenge is solved")

    # (bucket, target) -> lists
    warm, cold, conn = (collections.defaultdict(list) for _ in range(3))
    n_req, n_err, n_runs = collections.Counter(), collections.Counter(), collections.Counter()
    colos, ssh_wall, steal = collections.defaultdict(collections.Counter), collections.defaultdict(list), \
        collections.defaultdict(list)
    lping, lhttps = collections.defaultdict(lambda: collections.defaultdict(list)), \
        collections.defaultdict(lambda: collections.defaultdict(list))
    for r in recs:
        b = bucket(r["ts"], a.by)
        loc = r.get("local") or {}
        for name, pg in (loc.get("ping") or {}).items():
            lping[b][name].append(pg)
        for name, reqs in (loc.get("https") or {}).items():
            lhttps[b][name] += [q["ttfb"] for q in reqs if "ttfb" in q]
        if "ssh_wall" in r and "results" in r:
            ssh_wall[b].append(r["ssh_wall"] - r.get("elapsed", 0))
            steal[b].append(r.get("steal_pct", 0))
        for tgt, reqs in r.get("results", {}).items():
            if a.target and tgt != a.target:
                continue
            k = (b, tgt)
            n_runs[k] += 1
            for q in reqs:
                n_req[k] += 1
                ok = "error" not in q and 200 <= q.get("status", 0) < 300
                if not ok:
                    n_err[k] += 1
                    continue
                (cold if q["i"] == 0 else warm)[k].append(q["ttfb"])
                conn[k].append(q["connect"])
                colos[k][f"{q.get('colo')}/{q.get('vercel')}"] += 1

    keys = sorted(n_runs)
    hdr = f"{'bucket':<14}{'target':<15}{'runs':>5}{'warm p50':>10}{'p90':>7}{'max':>7}{'cold p50':>10}" \
          f"{'cold max':>9}{'conn p50':>9}{'err%':>6}  route"
    print("\nTTFB in ms (fresh connection per request)\n" + hdr + "\n" + "-" * len(hdr))
    prev = None
    for k in keys:
        b, tgt = k
        if prev is not None and b != prev:
            print()
        prev = b
        w, c = warm[k], cold[k]
        route = ",".join(x for x, _ in colos[k].most_common(2))
        print(f"{b if b else '':<14}{tgt:<15}{n_runs[k]:>5}{ms(pct(w, .5)):>10}{ms(pct(w, .9)):>7}{ms(max(w, default=None)):>7}"
              f"{ms(pct(c, .5)):>10}{ms(max(c, default=None)):>9}{ms(pct(conn[k], .5)):>9}"
              f"{100 * n_err[k] / max(1, n_req[k]):>6.1f}  {route}")

    print("\nper bucket: ssh overhead (local<->jumpbox, s) p50 / jumpbox CPU steal % p50")
    for b in sorted(ssh_wall):
        print(f"  {b:<14} ssh {statistics.median(ssh_wall[b]):6.2f}   steal {statistics.median(steal[b]):5.2f}")

    if lping:
        print("\nlocal baseline (this machine): ping avg p50 / mdev p50 / mean loss %, HTTPS TTFB p50 (ms)")
        for b in sorted(lping):
            cells = []
            for name, pgs in sorted(lping[b].items()):
                avg = [p["avg"] for p in pgs if "avg" in p]
                dev = [p["mdev"] for p in pgs if "mdev" in p]
                loss = statistics.mean(p.get("loss", 100) for p in pgs)
                cells.append(f"{name} {statistics.median(avg) if avg else float('nan'):.0f}/"
                             f"{statistics.median(dev) if dev else float('nan'):.1f}/{loss:.0f}%")
            cells += [f"https {n} {ms(pct(v, .5))}" for n, v in sorted(lhttps[b].items())]
            print(f"  {b:<14} " + "   ".join(cells))

    ranked = [(pct(warm[k], .5), pct(cold[k], .5), 100 * n_err[k] / max(1, n_req[k]), k[0])
              for k in keys if k[1] == a.rank_target and n_runs[k] >= a.min_n and warm[k]]
    if ranked:
        ranked.sort(key=lambda x: (x[2] > 0, x[0]))
        print(f"\n{a.rank_target} buckets with >= {a.min_n} runs, best first (error-free first, then warm p50):")
        for w, c, e, b in ranked[:10]:
            print(f"  {b:<14} warm p50 {ms(w):>5} ms   cold p50 {ms(c):>5} ms   err {e:.1f}%")
    else:
        print(f"\n(no {a.rank_target} bucket has >= {a.min_n} runs yet - let the timer run for a day or two)")


if __name__ == "__main__":
    main()
