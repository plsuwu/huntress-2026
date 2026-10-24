#!/usr/bin/env python3
"""wk.py - a web worker's only door to the Game Master.

Guards first: this script is the security boundary between a worker agent and
the team's shared GM identity. It is deliberately tiny - no config, no planner,
no registry beyond a family.json in the worker directory.

Commands (run from the worker directory, workers/web/<base-id>/):

  begin [mode]    plain untimed `start` (reconnect-safe); bounded wait for
                  preparing -> active targets and for target-capacity 409s;
                  saves start.json / targets.json / provisioning.json into
                  the mode directory.
  submit [mode]   submits <mode>/answers.json with exactly the answer_ids
                  from the current start.json, verified against the current
                  attempt_id; appends a round to results.json, writes
                  result.json, prints the GM verdict.
  status          local record: per-mode verdict, attempt_id, target count.
  status --live   the same, plus a live GM op=status per route (raw response
                  saved to <mode>/status-gm.json).
  next            the first mode (single_solve, time_trial, mastery) whose
                  results.json has no CORRECT round.

Hard guards:

  * Untimed only. The only operations this script can ever send are
    op=status (GET) for the worker's own routes and op=start / op=submit
    (POST) with fixed untimed payloads. There is no code path for
    run_for_time, retry, practice, reset, switch_target, artifact, download
    or list, and WEB_UNTIMED_ONLY=1 must be set in the environment.
  * Own family only. The challenge id never comes from the command line; it
    comes from family.json in the worker directory and must be a
    web-exploitation route of that family.
  * Token discipline. HUNTRESS_GM_TOKEN is read from the environment only,
    never printed, and only ever sent to the pinned GM host as a Bearer
    header. Redirects are refused outright - a signed artifact URL is a
    redirect, and following one would carry the header off the GM host.
  * No GM lock. Calls are direct: workers are trusted not to flood the GM
    (2026-10-24 decision - solution speed over gating). wk.py only backs off
    reactively on 429/5xx; reintroduce a shared lock if rate limits appear.

Exit codes: 0 ok, 1 blocked (message on stderr - do not work around),
75 still waiting (run the command again), 2 usage.

The GM machinery and the evidence-file shapes are importable (tools/
web_rush.py does `import wk`, so the first pass and the workers share one
GM-call implementation and one results format):

  answers.json    {"answers": {"<answer_id>": "<value>", ...}}
  results.json    [{"round": N, "t": iso, "answers": {...}, "response": {...}}]
  result.json     the latest submit response
"""

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]      # the 2026-web repo root
WORKERS = ROOT / "workers" / "web"             # worker root (shared locks live here)
GM_HOST = "2026.huntress.ctf.games"            # pinned host - the token never leaves it
GM_URL = f"https://{GM_HOST}/"
GM_TIMEOUT_S = 30
GM_ATTEMPTS = 3
MODES = ("single_solve", "time_trial", "mastery")
MODE_SUFFIX = {"single_solve": "", "time_trial": "/time_trial", "mastery": "/mastery"}
BEGIN_WAIT_S = int(os.environ.get("WK_BEGIN_WAIT_S", "300"))
BEGIN_POLL_S = 5
LIMIT_POLL_S = 30                              # wait between web_target_limit_reached retries
SUBMIT_CAP = 100                               # rounds per mode - a runaway-loop guard

EXIT_OK, EXIT_BLOCKED, EXIT_WAITING, EXIT_USAGE = 0, 1, 75, 2


def die(msg, code=EXIT_BLOCKED):
    print(f"wk.py: {msg}", file=sys.stderr)
    sys.exit(code)


def iso_now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def short(raw, limit=300):
    """One-line, truncated rendering of a GM response (never contains the token)."""
    try:
        s = raw if isinstance(raw, str) else json.dumps(raw)
    except (TypeError, ValueError):
        s = repr(raw)
    s = " ".join(s.split())
    return s if len(s) <= limit else s[: limit - 3] + "..."


# --------------------------------------------------------------------------
# GM machinery (importable - web_rush.py reuses all of it)
# --------------------------------------------------------------------------

class _RefuseRedirects(urllib.request.HTTPRedirectHandler):
    """Artifact/download URLs answer with redirects; following one would carry
    the Authorization header off the GM host. Refuse them all."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code,
                                     "redirect refused (artifact URLs are never followed)",
                                     headers, fp)


_OPENER = urllib.request.build_opener(_RefuseRedirects)


def _parse(raw):
    try:
        return json.loads(raw)
    except (ValueError, TypeError):
        return raw.decode("utf-8", "replace") if isinstance(raw, bytes) else raw


def _one_gm_call(token, method, op, challenge, payload):
    """Build and fire exactly one GM request. Returns (http_status, parsed)."""
    headers = {"Authorization": f"Bearer {token}"}
    if method == "GET":
        q = urllib.parse.urlencode({"op": op, "challenge": challenge},
                                   quote_via=urllib.parse.quote)
        url, data = GM_URL + "?" + q, None
    else:
        url = GM_URL
        body = {"op": op, "challenge": challenge}
        if payload:
            body.update(payload)
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with _OPENER.open(req, timeout=GM_TIMEOUT_S) as r:
            return r.status, _parse(r.read())
    except urllib.error.HTTPError as e:
        return e.code, _parse(e.read())


def gm_request(token, method="GET", op=None, challenge=None, payload=None):
    """One direct GM call - no lock, no proactive pacing.

    Workers are trusted not to flood the GM (2026-10-24 decision: solution
    speed over gating; reintroduce a shared lock if rate limits appear).
    Transient failures (429/5xx/network) are retried a bounded number of
    times with backoff. Redirects are fatal. Returns (http_status, parsed).
    """
    err = None
    for attempt in range(GM_ATTEMPTS):
        status = None
        try:
            status, body = _one_gm_call(token, method, op, challenge, payload)
            if 300 <= status < 400:
                die(f"GM answered HTTP {status} with a redirect - refusing to follow it")
        except urllib.error.URLError as e:
            err = f"network error: {e.reason}"
        if status is not None and status != 429 and status < 500:
            return status, body
        err = err or f"HTTP {status}"
        if attempt + 1 < GM_ATTEMPTS:
            time.sleep(2 * (attempt + 1))
    die(f"GM call {method} op={op} failed after {GM_ATTEMPTS} attempts: {err}")


def route_status(token, route):
    """Live op=status for one route (any HTTP code; the body decides)."""
    status, body = gm_request(token, "GET", op="status", challenge=route)
    return body if isinstance(body, dict) else {"http_status": status, "body": body}


def norm_state(raw):
    """Best-effort normalized route state from an op=status response."""
    if not isinstance(raw, dict):
        return "unknown"
    if raw.get("error") == "start_required":
        return "not_started"
    for key in ("state", "result", "status"):
        v = raw.get(key)
        if isinstance(v, str) and v:
            return v.lower()
    for key in ("correct", "is_correct", "solved"):
        if raw.get(key) is True:
            return "correct"
    if raw.get("error"):
        return f"error:{raw['error']}"
    return "unknown"


def is_correct(raw):
    return norm_state(raw) == "correct"


def looks_solved(start_response):
    """True if a start/status response describes an already-solved route."""
    if not isinstance(start_response, dict):
        return False
    for key in ("state", "result"):
        if str(start_response.get(key, "")).lower() == "correct":
            return True
    if "correct" in str(start_response.get("error", "")).lower():
        return True
    answer_ids = start_response.get("answer_ids") or []
    accepted = start_response.get("accepted_ids") or []
    return bool(answer_ids) and all(a in accepted for a in answer_ids)


# --------------------------------------------------------------------------
# Worker directory, family, guards
# --------------------------------------------------------------------------

def check_guards():
    """The security boundary. Returns the GM token (never printed)."""
    if os.environ.get("WEB_UNTIMED_ONLY") != "1":
        die("WEB_UNTIMED_ONLY is not '1' - this worker may only make untimed attempts")
    token = os.environ.get("HUNTRESS_GM_TOKEN")
    if not token:
        die("HUNTRESS_GM_TOKEN is not set in the environment")
    url = os.environ.get("HUNTRESS_GM_URL", GM_URL)
    host = urllib.parse.urlparse(url).netloc
    if host != GM_HOST:
        die(f"HUNTRESS_GM_URL host {host!r} is not the pinned GM host {GM_HOST!r} - "
            f"the token is only ever sent to the GM")
    return token


def worker_dir():
    cwd = Path.cwd().resolve()
    try:
        cwd.relative_to(WORKERS.resolve())
    except ValueError:
        die(f"cwd {cwd} is not a worker directory under {WORKERS} "
            f"(run wk.py from your worker directory)")
    if not (cwd / "family.json").is_file():
        die(f"no family.json in {cwd} - not a worker directory")
    return cwd


def load_family(wt):
    """Return (base, {mode: route_id}) from the worker's family.json."""
    try:
        fam = json.loads((wt / "family.json").read_text())
    except (OSError, ValueError) as e:
        die(f"cannot read family.json: {e}")
    if not isinstance(fam, dict):
        die("family.json must be a JSON object")
    base = None
    for key in ("base", "family", "challenge", "id"):
        v = fam.get(key)
        if isinstance(v, str) and v:
            base = v
            break
    if not base:
        die("family.json has no base challenge id")
    if not base.startswith("web-exploitation-"):
        die(f"family base {base!r} is not a web-exploitation challenge - "
            f"this worker may only touch web-exploitation routes")
    routes = {}
    rows = fam.get("routes")
    if isinstance(rows, list):
        for row in rows:
            if (isinstance(row, dict) and row.get("mode") in MODES
                    and isinstance(row.get("id"), str)):
                routes[row["mode"]] = row["id"]
    for mode, suffix in MODE_SUFFIX.items():
        routes.setdefault(mode, base + suffix)
    for mode, route in routes.items():
        if not route.startswith(base):
            die(f"family route for {mode} ({route!r}) is not under the family base {base!r}")
    return base, routes


# --------------------------------------------------------------------------
# Evidence-file shapes (shared with the driver's first pass)
# --------------------------------------------------------------------------

def load_answers(mdir):
    """Read <mode>/answers.json - {"answers": {...}} (a bare mapping is tolerated)."""
    p = mdir / "answers.json"
    if not p.is_file():
        return None
    try:
        raw = json.loads(p.read_text())
    except ValueError as e:
        die(f"answers.json is not valid JSON: {e}")
    if isinstance(raw, dict) and isinstance(raw.get("answers"), dict):
        return raw["answers"]
    if isinstance(raw, dict):
        return raw
    die("answers.json must be a JSON object of answer ids to values")


def save_answers(mdir, answers):
    (mdir / "answers.json").write_text(json.dumps({"answers": answers}, indent=2) + "\n")


def load_rounds(mdir):
    p = mdir / "results.json"
    if not p.is_file():
        return []
    try:
        rounds = json.loads(p.read_text())
    except ValueError:
        return []
    return rounds if isinstance(rounds, list) else []


def write_result(mdir, response):
    (mdir / "result.json").write_text(json.dumps(response, indent=2) + "\n")


def save_round(mdir, answers, response):
    """Append one submit round to results.json and refresh result.json."""
    rounds = load_rounds(mdir)
    rounds.append({"round": len(rounds) + 1, "t": iso_now(),
                   "answers": answers, "response": response})
    (mdir / "results.json").write_text(json.dumps(rounds, indent=2) + "\n")
    write_result(mdir, response)


def mode_correct(mdir):
    """True if results.json already holds a CORRECT round (CORRECT is terminal)."""
    for r in load_rounds(mdir):
        resp = r.get("response")
        if isinstance(resp, dict) and str(resp.get("result", "")).upper() == "CORRECT":
            return True
    return False


def append_event(line):
    """Append one line to the shared workers/web/.events log (route completions)."""
    try:
        with open(WORKERS / ".events", "a") as f:
            f.write(line.rstrip("\n") + "\n")
    except OSError:
        pass


# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------

def _save_provisioning(mdir, route, mode, polls):
    (mdir / "provisioning.json").write_text(json.dumps(
        {"route": route, "mode": mode, "polls": polls}, indent=2) + "\n")


def cmd_begin(wt, routes, mode, token):
    route = routes[mode]
    mdir = wt / mode
    mdir.mkdir(parents=True, exist_ok=True)

    # Live check first: never start an attempt on a route that is already CORRECT.
    raw = route_status(token, route)
    if is_correct(raw):
        print(f"{route}: already CORRECT - nothing to begin")
        return
    state = norm_state(raw)
    if state not in ("not_started", "active", "unknown"):
        print(f"wk.py: note: op=status reports {state!r} for {route} - starting anyway")

    prev_attempt = None
    try:
        prev = json.loads((mdir / "start.json").read_text())
        if isinstance(prev, dict):
            prev_attempt = prev.get("attempt_id")
    except (OSError, ValueError):
        pass

    deadline = time.time() + BEGIN_WAIT_S
    polls = []
    sj = None
    targets = []
    while True:
        status, body = gm_request(token, "POST", op="start", challenge=route)
        if status == 409 and isinstance(body, dict) and "web_target_limit_reached" in short(body):
            polls.append({"t": iso_now(), "note": "team web target limit reached"})
            left = max(0, int(deadline - time.time()))
            print(f"wk.py: team web target limit reached - waiting for a slot "
                  f"({left}s left in budget)", flush=True)
            if left <= 0:
                _save_provisioning(mdir, route, mode, polls)
                die("still waiting for a web target slot - run begin again", EXIT_WAITING)
            time.sleep(min(LIMIT_POLL_S, max(1.0, deadline - time.time())))
            continue
        if status != 200:
            die(f"op=start for {route} -> HTTP {status}: {short(body)}")
        if not isinstance(body, dict):
            die(f"op=start for {route} returned a non-JSON response: {short(body)}")
        sj = body
        targets = [t for t in (sj.get("targets") or []) if isinstance(t, dict)]
        polls.append({"t": iso_now(), "result": sj.get("result"),
                      "attempt_id": sj.get("attempt_id"),
                      "targets": [{"id": t.get("id"), "url": t.get("url")} for t in targets]})
        if looks_solved(sj):
            print(f"{route}: already CORRECT - nothing to begin")
            return
        if sj.get("result") == "active" and targets and all(t.get("url") for t in targets):
            break
        left = deadline - time.time()
        if left <= 0:
            _save_provisioning(mdir, route, mode, polls)
            die(f"targets not ready after {BEGIN_WAIT_S}s "
                f"(result={sj.get('result')!r}, targets={len(targets)}) - run begin again",
                EXIT_WAITING)
        time.sleep(min(BEGIN_POLL_S, left))

    (mdir / "start.json").write_text(json.dumps(sj, indent=2) + "\n")
    (mdir / "targets.json").write_text(json.dumps(targets, indent=2) + "\n")
    _save_provisioning(mdir, route, mode, polls)

    if prev_attempt and prev_attempt != sj.get("attempt_id"):
        print(f"wk.py: note: attempt changed {prev_attempt} -> {sj.get('attempt_id')} "
              f"(the old attempt expired); answers derived from it are stale")

    print(f"attempt {sj.get('attempt_id')} active on {route}")
    for t in targets:
        print(f"  target {t.get('id')}: {t.get('url')}")
    answer_ids = sj.get("answer_ids") or []
    print(f"answer_ids: {', '.join(answer_ids) if answer_ids else '(none)'}")
    print(f"questions: {len(sj.get('questions') or [])} "
          f"(join questions[].artifact to targets[].id)")
    print(f"wrote {mode}/start.json, {mode}/targets.json, {mode}/provisioning.json")
    if answer_ids:
        print(f"next: write {mode}/answers.json mapping each answer_id to its value, "
              f"then: python3 tools/wk.py submit {mode}")


def cmd_submit(wt, routes, mode, token):
    route = routes[mode]
    mdir = wt / mode

    # Local fast path: CORRECT is terminal and already recorded.
    if mode_correct(mdir):
        print(f"{route}: already CORRECT (round {len(load_rounds(mdir))}) - nothing to submit")
        return

    try:
        sj = json.loads((mdir / "start.json").read_text())
    except (OSError, ValueError):
        die(f"no readable {mode}/start.json - run: python3 tools/wk.py begin {mode}")
    if not isinstance(sj, dict):
        die(f"{mode}/start.json must be a JSON object - run: python3 tools/wk.py begin {mode}")
    answer_ids = sj.get("answer_ids") or []
    if not answer_ids:
        die(f"{mode}/start.json has no answer_ids - run: python3 tools/wk.py begin {mode}")

    answers = load_answers(mdir)
    if answers is None:
        die(f'no {mode}/answers.json - write it as {{"answers": {{"<answer_id>": "<value>"}}}} '
            f"for exactly: {', '.join(answer_ids)}")
    bad = sorted(k for k, v in answers.items() if not isinstance(v, str))
    if bad:
        die(f"answers.json values must be strings (non-string for: {', '.join(bad)})")
    extra = sorted(set(answers) - set(answer_ids))
    missing = sorted(set(answer_ids) - set(answers))
    if extra or missing:
        die(f"answers.json keys must be exactly the answer_ids from start.json "
            f"(extra: {', '.join(extra) or 'none'}; missing: {', '.join(missing) or 'none'})")

    rounds = load_rounds(mdir)
    if len(rounds) >= SUBMIT_CAP:
        die(f"submit cap ({SUBMIT_CAP} rounds) reached for {route} - "
            f"stop and reconsider; do not work around this")

    # Verify the attempt is still the one begin saved (idle expiry turns it over).
    status, body = gm_request(token, "POST", op="start", challenge=route)
    if status == 409 and isinstance(body, dict) and "web_target_limit_reached" in short(body):
        die("team web target limit reached - run begin again, then submit", EXIT_WAITING)
    if status == 409 and isinstance(body, dict) and body.get("error") == "start_required":
        die(f"no attempt on {route} - run: python3 tools/wk.py begin {mode}")
    if status != 200:
        die(f"op=start (attempt check) for {route} -> HTTP {status}: {short(body)}")
    if not isinstance(body, dict):
        die(f"op=start for {route} returned a non-JSON response: {short(body)}")
    if looks_solved(body):
        save_round(mdir, answers, body)
        append_event(f"{route} correct")
        print(f"{route}: already CORRECT - recorded from GM status")
        return
    if body.get("result") != "active":
        die(f"attempt on {route} is not active (result={body.get('result')!r}) - "
            f"run: python3 tools/wk.py begin {mode}", EXIT_WAITING)
    if body.get("attempt_id") != sj.get("attempt_id"):
        die(f"attempt changed since begin ({sj.get('attempt_id')} -> {body.get('attempt_id')}); "
            f"the saved answers are stale - run: python3 tools/wk.py begin {mode} "
            f"and re-derive them")

    # Submit - exactly the answer_ids, nothing else.
    status, body = gm_request(token, "POST", op="submit", challenge=route,
                              payload={"answers": answers})
    if status == 409 and isinstance(body, dict) and "web_target_limit_reached" in short(body):
        die("team web target limit reached - run again", EXIT_WAITING)
    if status != 200:
        die(f"op=submit for {route} -> HTTP {status}: {short(body)}")
    if not isinstance(body, dict):
        die(f"op=submit for {route} returned a non-JSON response: {short(body)}")

    verdict = str(body.get("result", "")).upper()
    save_round(mdir, answers, body)
    if verdict == "CORRECT":
        append_event(f"{route} correct")
        print(f"verdict: CORRECT - {route} is solved")
        if mode != MODES[-1]:
            print(f"next: python3 tools/wk.py begin {MODES[MODES.index(mode) + 1]}")
        else:
            print("next: every mode of this family is CORRECT - write NOTES.md, commit, "
                  "touch the done marker")
    elif verdict == "ACCEPTED":
        print(f"verdict: accepted - accepted_ids={body.get('accepted_ids')} "
              f"remaining={body.get('remaining')}")
        print("next: fix the remaining answers in answers.json and submit again "
              "(already-accepted answers are free to resend)")
    else:
        print(f"verdict: {verdict or 'unknown'} - re-derive the wrong answers from the "
              f"evidence in {mode}/ and submit again")


def cmd_status(wt, routes, live, token):
    print(f"family routes (worker dir {wt.name}):")
    for mode in MODES:
        route = routes[mode]
        mdir = wt / mode
        rounds = load_rounds(mdir)
        verdict = str(rounds[-1]["response"].get("result", "?")) if rounds else "no submits"
        line = f"  {mode:12s} {route}  [{verdict}]"
        if mode_correct(mdir):
            line = f"  {mode:12s} {route}  [CORRECT]"
        try:
            attempt = json.loads((mdir / "start.json").read_text()).get("attempt_id")
            if attempt:
                line += f" attempt={attempt}"
        except (OSError, ValueError, AttributeError):
            pass
        if (mdir / "targets.json").is_file():
            try:
                line += f" targets={len(json.loads((mdir / 'targets.json').read_text()))}"
            except (ValueError, TypeError):
                pass
        if live:
            raw = route_status(token, route)
            mdir.mkdir(parents=True, exist_ok=True)
            (mdir / "status-gm.json").write_text(json.dumps(raw, indent=2) + "\n")
            bits = []
            if isinstance(raw, dict):
                for k in ("run_for_time_supported", "ranked_attempts_remaining",
                          "regular_attempts_remaining", "regular_variants_remaining",
                          "hall_of_fame_eligible", "deadline"):
                    if k in raw:
                        bits.append(f"{k}={raw[k]}")
            line += f" | gm state={norm_state(raw)}"
            if bits:
                line += f" [{', '.join(bits)}]"
        print(line)
    if not live:
        print("hint: status --live queries the GM (op=status per route) and saves "
              "<mode>/status-gm.json")


def cmd_next(wt, routes):
    for mode in MODES:
        if not mode_correct(wt / mode):
            print(f"{mode} {routes[mode]}")
            return
    print("all routes CORRECT")


def main(argv=None):
    ap = argparse.ArgumentParser(
        prog="wk.py",
        description="A web worker's only door to the Game Master "
                    "(untimed, own-family routes only).")
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("begin", "submit"):
        p = sub.add_parser(name)
        p.add_argument("mode", nargs="?", default="single_solve", choices=MODES)
    p = sub.add_parser("status")
    p.add_argument("--live", action="store_true")
    sub.add_parser("next")
    args = ap.parse_args(argv)

    token = check_guards()
    wt = worker_dir()
    base, routes = load_family(wt)
    if args.cmd == "begin":
        cmd_begin(wt, routes, args.mode, token)
    elif args.cmd == "submit":
        cmd_submit(wt, routes, args.mode, token)
    elif args.cmd == "status":
        cmd_status(wt, routes, args.live, token)
    elif args.cmd == "next":
        cmd_next(wt, routes)
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
