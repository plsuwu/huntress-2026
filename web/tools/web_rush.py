#!/usr/bin/env python3
"""Day-release orchestrator for the web lane: one opencode agent per challenge (a shared
opencode server, tmux for human visibility) plus an automated first pass.

  web_rush.py all     [--day N] [--rehearse] [--workers N]  prep + launch + watch (start it ~22:45 AEST; see docs/runbook.md)
  web_rush.py prep    [--day N]                             worker dirs + shared-tree symlinks, stale markers cleared
  web_rush.py launch  [--day N] [--workers N]                tmux session `web` (socket -L huntress-web) + the opencode server: primed agent per slot
  web_rush.py watch   [--day N] [--rehearse] [--no-agents]  wait for the release, first pass per challenge, dispatch briefs
  web_rush.py status  [--day N]                             first-pass results, challenge done-markers, agent state
  web_rush.py takeover --challenges a[,b] [--day N]         coordinator: nudge a challenge's existing session
Options: --workers N (pre-warmed slots, default 3), --no-agents (watch: first pass + briefs only, no dispatch),
         --model NAME (worker opencode model, default unset - tracks opencode's own configured default),
         --effort LEVEL (worker reasoning effort / opencode "variant", default unset).

Release: 23:00 Australia/Brisbane; Day N = that date's day of the month. Work items are the day's web-exploitation
challenges, discovered at release (op=list&day=N&lane=web-exploitation, grouped into base challenges). Pre-warmed slot i
gets challenge i; more challenges than slots -> extra sessions are created on the spot, fewer -> spare slots stand down
(their tmux window is just closed - nothing to tell an idle agent). The first pass is pure automation, no model: per
challenge, single_solve -> time_trial -> mastery, each mode started only once its prerequisite is CORRECT, every start a
plain untimed one. Each challenge's brief (routes, per-mode results, recon evidence, submit feedback, instructions) is
written to .rush-dNN/brief-<chal>.md and its agent is told to read it - as soon as that challenge's first pass ends, not
when all end. The first pass never sends run_for_time/retry/practice/reset/switch_target either: the only GM ops are
list (route discovery), status, start and submit, and the worker's only GM door (tools/wk.py) has no timed code path at
all. GM calls are direct - no lock, no proactive pacing (2026-10-24 decision: solution speed over gating; both this
driver and wk.py back off reactively on 429/5xx).
Release detection: op=list&day=N&lane=web-exploitation every 1 s from 22:59:50 (the lane-filtered list answers in ~1 s;
a full all=true list takes 4-10 s - avoid it), released once two consecutive polls agree. `all` kills the previous day's
tmux session before relaunching. `--rehearse` skips the wait and replays the flow on an already-released day (solved
routes are recognised from GM status; no agent is dispatched).

Agent architecture (2026-10-28, opencode): one shared `opencode serve` process hosts every session for the day (started
by `launch`/`all`; its own environment carries HUNTRESS_GM_TOKEN/HUNTRESS_GM_URL/WEB_UNTIMED_ONLY, since tool calls run
server-side - every session's Bash tool inherits it from there, not from a per-window flag). A tmux window per
challenge runs `opencode attach <url> --session <id>` purely for human visibility/intervention - the coordinator can
watch or type directly into the pane. The driver's own turns go over `opencode run --attach <url> --session <id>
--format json "<message>"`, which blocks until the agent's turn ends: that block IS the "is it done" signal, so there
is no hook/marker-based stall detection here (opencode has no Stop/PostToolUse hook equivalent anyway, and a shared
`opencode serve` process runs multiple sessions' turns genuinely in parallel - confirmed live before this was written).
No permission flag or config is passed either: plain bash tool calls run with zero permission friction under the
environment's default policy, and `--dangerously-skip-permissions` is both unneeded and a blanket bypass this pipeline
already rejects on principle (docs/policies.md). The real guards stay where they always lived: inside tools/wk.py.
Attach: tmux -L huntress-web attach -t web
"""

import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)                     # the 2026-web repo root
WORKERS = os.path.join(ROOT, "workers", "web")   # worker root + driver state
API = "https://2026.huntress.ctf.games"
AEST = timezone(timedelta(hours=10))  # Brisbane: no daylight saving
TMUX = ["tmux", "-L", "huntress-web"]
SESSION = "web"
LANE = "web-exploitation"
LANE_PREFIX = "web-exploitation-"
WORKER_SLOTS = 3
OC_PORT = 41099                        # fixed, distinct from opencode's own default port
OC_URL = f"http://127.0.0.1:{OC_PORT}"
WORKER_MODEL = None                    # unset: opencode's own configured default (openai/gpt-6.1-sol as of writing)
WORKER_VARIANT = None                  # unset: opencode's own configured default reasoning effort
NO_AGENTS = False
PRIME_TIMEOUT_S = 300                  # the prime turn is short: read docs, reply "ready", wait
CHALLENGE_TIMEOUT_S = 60 * 60          # generous ceiling per challenge turn - not a stall detector
SERVER_UP_WAIT_S = 30

sys.path.insert(0, HERE)
import wk  # noqa: E402  the worker's GM helper: one shared GM-call implementation
sys.path.insert(0, os.path.join(ROOT, "solver_py_scripts"))
import recovery_lib  # noqa: E402  the first pass's classifiers/recoverers

SHARED_TREES = ("AGENTS.md", "HOMEBASE.md", "docs", "solver_py_scripts", "tools")
# Symlinked from WORKERS (workers/web/), not ROOT - the consolidated, tagged reference library
# (past solve.py scripts + per-day notes; see workers/web/solution-notes/TAGGING-PROMPT.md).
WORKER_SHARED_TREES = ("past-solutions", "solution-notes")


def symlink_shared_trees(dest):
    """Symlink both the repo-root shared trees and the workers/web/ reference library into dest -
    used for both a real worker dir and a pre-warmed slot dir."""
    for root, rels in ((ROOT, SHARED_TREES), (WORKERS, WORKER_SHARED_TREES)):
        for rel in rels:
            link = os.path.join(dest, rel)
            src = os.path.join(root, rel)
            if not os.path.lexists(link) and os.path.lexists(src):
                os.symlink(src, link)


def log(msg):
    print(f"[{datetime.now(AEST):%H:%M:%S}] {msg}", flush=True)


def token():
    tok = os.environ.get("HUNTRESS_GM_TOKEN")
    if tok:
        return tok
    for line in open(os.path.join(ROOT, ".env")):
        if line.startswith("HUNTRESS_GM_TOKEN="):
            return line.split("=", 1)[1].strip()
    sys.exit("no HUNTRESS_GM_TOKEN (env or .env)")


def env():
    # WEB_UNTIMED_ONLY=1: wk.py refuses to run without it, and it has no timed/retry/practice
    # code path at all. This is the opencode server's own environment - every session's Bash
    # tool calls inherit it from there (tool execution happens server-side), not from a per-pane
    # flag. The token is never sent to a target.
    return dict(
        os.environ, HUNTRESS_GM_TOKEN=token(), HUNTRESS_GM_URL=API, WEB_UNTIMED_ONLY="1"
    )


def redact(text):
    """Never let the GM token reach a log line."""
    tok = os.environ.get("HUNTRESS_GM_TOKEN")
    if not tok:
        try:
            for line in open(os.path.join(ROOT, ".env")):
                if line.startswith("HUNTRESS_GM_TOKEN="):
                    tok = line.split("=", 1)[1].strip()
                    break
        except OSError:
            pass
    return text.replace(tok, "<token>") if tok else text


def sh(args, cwd=None, check=True, timeout=None, **kw):
    try:
        r = subprocess.run(args, cwd=cwd, text=True, capture_output=True, timeout=timeout, **kw)
    except subprocess.TimeoutExpired:
        return None
    if check and r.returncode != 0:
        raise RuntimeError(f"{redact(' '.join(args))}: {redact(r.stderr.strip()[-400:])}")
    return r


def state_dir(day):
    d = os.path.join(WORKERS, f".rush-d{day:02d}")
    os.makedirs(d, exist_ok=True)
    return d


def routes_cache(day):
    return os.path.join(WORKERS, f"routes-d{day:02d}.json")


# ---------------------------------------------------------------- routes


def get(url, timeout=60):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token()}"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def web_rows_of(rows, day):
    """Re-filter op=list rows by day and the web-exploitation- prefix, exactly like rush's
    lane_rows filters by lane prefix - search matches substrings, so the prefix is the guard."""
    return [c for c in rows
            if c.get("day") == day and str(c.get("id", "")).startswith(LANE_PREFIX)]


def cached_rows(day):
    try:
        rows = json.load(open(routes_cache(day))).get("challenges", [])
    except (OSError, ValueError):
        return []
    return web_rows_of(rows, day)


def group_challenges(rows):
    """Group route rows into base challenges (strip /time_trial, /mastery), sorted by base id;
    each family's rows in mode order (single_solve, time_trial, mastery)."""
    fams = {}
    for c in rows:
        fams.setdefault(str(c.get("id", "")).split("/")[0], []).append(c)
    order = {"single_solve": 0, "time_trial": 1, "mastery": 2}
    out = []
    for base in sorted(fams):
        fams[base].sort(key=lambda c: order.get(c.get("mode"), 9))
        out.append((base, fams[base]))
    return out


def short_name(base):
    """web-exploitation-day-24-slug -> day-24-slug (the window/marker name)."""
    return base[len(LANE_PREFIX):] if base.startswith(LANE_PREFIX) else base


def bare_slug(base):
    """web-exploitation-day-24-slug -> slug (the day-directory name)."""
    m = re.match(r"day-\d+-(.*)", short_name(base))
    return m.group(1) if m else short_name(base)


class Release:
    """op=list&day=N&lane=web-exploitation every 1 s from 22:59:50 (3 s after 23:01, in case the
    release is late). The lane-filtered list answers in ~1 s and is empty until the release.
    Released once two consecutive polls agree, so a partial listing cannot drop a challenge."""

    def __init__(self, day, rehearse):
        self.day, self.rehearse = day, rehearse
        self.released, self.rows = threading.Event(), []

    def start(self):
        threading.Thread(target=self._poll, daemon=True).start()
        return self

    def _poll(self):
        if not self.rehearse:
            sleep_until(at(22, 59, 50))
        seen, stable = None, 0
        while not self.released.is_set():
            t = time.time()
            try:
                rows = web_rows_of(
                    get(f"{API}/?op=list&day={self.day}&lane={LANE}", timeout=15).get(
                        "challenges", []),
                    self.day,
                )
                if rows:
                    ids = sorted(c["id"] for c in rows)
                    if ids == seen:
                        stable += 1
                    else:
                        stable, seen = 0, ids
                        log(f"day {self.day}: {len(rows)} web routes so far")
                    if stable >= 1:
                        log(f"day {self.day} released (op=list: {len(rows)} web routes, stable)")
                        self.rows = rows
                        self.released.set()
                        return
            except Exception as e:  # noqa: BLE001
                log(f"release probe failed: {e!r}")
            every = 1 if self.rehearse or datetime.now(AEST) < at(23, 1, 0) else 3
            time.sleep(max(0.05, every - (time.time() - t)))


def at(h, m, s):
    return datetime.now(AEST).replace(hour=h, minute=m, second=s, microsecond=0)


def sleep_until(t):
    while datetime.now(AEST) < t:
        time.sleep(min(5, max(0.05, (t - datetime.now(AEST)).total_seconds())))


# ---------------------------------------------------------------- prep


def ensure_worker_dir(day, base, rows):
    """Create workers/web/<base>/ with the read-only shared trees symlinked in and a family.json
    (wk.py's registry: the challenge id comes from here, never from the command line). The repo
    root is never a worker's cwd or an --add-dir equivalent: it holds the jumpbox SSH key and
    .env."""
    wt = os.path.join(WORKERS, base)
    os.makedirs(wt, exist_ok=True)
    symlink_shared_trees(wt)
    with open(os.path.join(wt, "family.json"), "w") as f:
        json.dump({"base": base, "release_day": day, "routes": rows}, f, indent=2)
        f.write("\n")
    starter = os.path.join(ROOT, "solver_py_scripts", "web_templates", "starter_solve.py")
    destination = os.path.join(wt, "solve.py")
    if not os.path.exists(destination):
        shutil.copyfile(starter, destination)
    return wt


def prep(day):
    """Worker directories for every challenge already known for the day (the route cache), the
    shared-tree symlinks, and a clean marker set. On release night the cache is empty - watch
    creates the worker dirs at the release."""
    os.makedirs(WORKERS, exist_ok=True)
    sd = state_dir(day)
    if os.path.isdir(sd):
        shutil.rmtree(sd)  # a same-day re-run must not inherit stale markers/sessions
        os.makedirs(sd)
    for name in os.listdir(WORKERS):
        if re.fullmatch(r"\.slot-\d+", name):
            shutil.rmtree(os.path.join(WORKERS, name))  # stale pre-warm slots
    log(f"state dir {sd} cleared")
    made = 0
    for base, rows in group_challenges(cached_rows(day)):
        ensure_worker_dir(day, base, rows)
        made += 1
    if made:
        log(f"{made} worker dir(s) prepared from the route cache")


# ---------------------------------------------------------------- opencode server + sessions


def ensure_server(day):
    """Start the shared opencode server if it isn't already answering. Its own environment is
    what every session's Bash tool calls inherit - tool execution happens server-side - so the
    GM token / WEB_UNTIMED_ONLY ride here once, rather than per tmux window."""
    try:
        urllib.request.urlopen(OC_URL + "/doc", timeout=2)
        return
    except Exception:  # noqa: BLE001
        pass
    logf = open(os.path.join(state_dir(day), "opencode-serve.log"), "a")
    subprocess.Popen(
        ["opencode", "serve", "--port", str(OC_PORT), "--hostname", "127.0.0.1"],
        cwd=ROOT, env=env(), stdout=logf, stderr=subprocess.STDOUT, start_new_session=True,
    )
    deadline = time.time() + SERVER_UP_WAIT_S
    while time.time() < deadline:
        try:
            urllib.request.urlopen(OC_URL + "/doc", timeout=2)
            log(f"opencode serve up on {OC_URL}")
            return
        except Exception:  # noqa: BLE001
            time.sleep(0.5)
    sys.exit(f"opencode serve did not come up on {OC_URL} within {SERVER_UP_WAIT_S}s - "
             f"see {os.path.join(state_dir(day), 'opencode-serve.log')}")


def oc_cmd(session, cwd, message, extra=()):
    args = ["opencode", "run", "--attach", OC_URL, "--format", "json"]
    if session:
        args += ["--session", session]
    if cwd:
        args += ["--dir", str(cwd)]
    if WORKER_MODEL:
        args += ["--model", WORKER_MODEL]
    if WORKER_VARIANT:
        args += ["--variant", WORKER_VARIANT]
    args += list(extra)
    args.append(message)
    return args


def oc_parse(raw):
    """Parse `opencode run --format json`'s one-JSON-object-per-line stream. Returns
    (session_id, events) - the session id comes off the first event that carries one."""
    events, sid = [], None
    for line in raw.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        events.append(ev)
        if sid is None and isinstance(ev, dict) and ev.get("sessionID"):
            sid = ev["sessionID"]
    return sid, events


def oc_run(session, message, cwd, timeout_s=CHALLENGE_TIMEOUT_S, extra=()):
    """Send one turn to an existing session (or, with session=None plus a --title in extra,
    create a fresh one); blocks until the agent's turn ends - that block is the only "is it done"
    signal this pipeline needs. Returns (session_id, events); raises RuntimeError on a non-zero
    exit or a timeout."""
    r = sh(oc_cmd(session, cwd, message, extra), check=False, timeout=timeout_s)
    if r is None:
        raise RuntimeError(f"opencode run timed out after {timeout_s}s (session {session})")
    if r.returncode != 0:
        raise RuntimeError(f"opencode run exited {r.returncode}: "
                            f"{redact(r.stderr.strip()[-400:])}")
    sid, events = oc_parse(r.stdout)
    return (sid or session), events


def oc_new_session(title, message, cwd, timeout_s=PRIME_TIMEOUT_S):
    sid, events = oc_run(None, message, cwd, timeout_s, extra=("--title", title))
    if not sid:
        raise RuntimeError(f"opencode run did not report a sessionID for title {title!r}")
    return sid, events


def attach_window(window_name, session_id, cwd):
    """Open a tmux window that just shows (and lets a human type into) the session's live TUI -
    the human-visibility half of the split; the driver's own turns go over oc_run, never
    tmux send-keys."""
    cmd = shlex.join(["opencode", "attach", OC_URL, "--session", session_id])
    sh(TMUX + ["new-window", "-t", SESSION, "-n", window_name, "-c", cwd, cmd])


# ---------------------------------------------------------------- launch


def slot_dir(slot):
    return os.path.join(WORKERS, f".slot-{slot}")


def ensure_slot_dir(slot):
    """A pre-warmed worker's cwd before the release: the shared trees symlinked in, so the prime
    prompt's reads work exactly as written. No family.json - wk.py rightly refuses to run here
    until the worker has a challenge."""
    d = slot_dir(slot)
    if os.path.isdir(d):
        shutil.rmtree(d)
    os.makedirs(d)
    symlink_shared_trees(d)
    return d


def prime_prompt(day, slot, n, wt):
    tpl = open(os.path.join(HERE, "prompts", "prime.md")).read()
    return tpl.format(i=slot, n=n, day=day, wt=wt)


def launch(day, n_workers=WORKER_SLOTS):
    have = sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0
    if have:
        log("tmux session already exists - not relaunching "
            "(tmux -L huntress-web kill-session -t web to reset)")
        return
    ensure_server(day)
    sd = state_dir(day)
    sh(TMUX + ["new-session", "-d", "-s", SESSION, "-n", "control", "-c", WORKERS])

    primed = {}

    def prime_slot(slot):
        cwd = ensure_slot_dir(slot)
        try:
            sid, _ = oc_new_session(f"web-d{day:02d}-slot-{slot}",
                                    prime_prompt(day, slot, n_workers, cwd), cwd,
                                    timeout_s=PRIME_TIMEOUT_S)
            primed[slot] = (sid, cwd)
        except Exception as e:  # noqa: BLE001  one slot's failure must not drop the others
            log(f"slot {slot}: prime failed ({e!r})")

    threads = [threading.Thread(target=prime_slot, args=(slot,))
               for slot in range(1, n_workers + 1)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    for slot, (sid, cwd) in sorted(primed.items()):
        with open(os.path.join(sd, f"slot-{slot}.session"), "w") as f:
            f.write(sid + "\n")
        attach_window(f"slot-{slot}", sid, cwd)
        log(f"slot {slot}: primed (session {sid}, window {SESSION}:slot-{slot})")


# ---------------------------------------------------------------- first pass


def gm(token_, method="GET", op=None, challenge=None, payload=None):
    """wk.gm_request with wk.py's die() (SystemExit) turned into an exception, so one
    challenge's first pass cannot kill the driver's other threads."""
    try:
        return wk.gm_request(token_, method, op=op, challenge=challenge, payload=payload)
    except SystemExit as e:
        raise RuntimeError(f"wk.py exited {e.code} (see the driver's stderr above)") from None


def begin_route(token_, route, mode, wt, wait_s=300):
    """Plain untimed start with a bounded preparing->active wait and bounded target-capacity waits
    (the team limit frees when a teammate's target expires or its worker finishes - the shared
    .events log records those). Saves start.json / targets.json / provisioning.json into the mode
    dir, exactly like wk.py begin. Returns (start_json, targets) or (None, reason)."""
    mdir = Path(wt) / mode
    mdir.mkdir(parents=True, exist_ok=True)
    deadline = time.time() + wait_s
    polls = []
    while True:
        status, body = gm(token_, "POST", op="start", challenge=route)
        if status == 409 and isinstance(body, dict) \
                and "web_target_limit_reached" in wk.short(body):
            polls.append({"t": wk.iso_now(), "note": "team web target limit reached"})
            left = max(0, int(deadline - time.time()))
            log(f"{route}: team web target limit reached - waiting for a slot "
                f"({left} s left in budget)")
            if left <= 0:
                return None, "still waiting for a web target slot"
            time.sleep(min(30, max(1.0, deadline - time.time())))
            continue
        if status != 200 or not isinstance(body, dict):
            return None, f"op=start -> HTTP {status}: {wk.short(body)}"
        targets = [t for t in (body.get("targets") or []) if isinstance(t, dict)]
        polls.append({"t": wk.iso_now(), "result": body.get("result"),
                      "attempt_id": body.get("attempt_id"),
                      "targets": [{"id": t.get("id"), "url": t.get("url")} for t in targets]})
        if wk.looks_solved(body):
            break  # already CORRECT: recognised without a worker; save the response below
        if body.get("result") == "active" and targets and all(t.get("url") for t in targets):
            break
        left = deadline - time.time()
        if left <= 0:
            return None, (f"targets not ready after {wait_s} s "
                          f"(result={body.get('result')!r}, targets={len(targets)})")
        time.sleep(min(5, left))
    (mdir / "start.json").write_text(json.dumps(body, indent=2) + "\n")
    (mdir / "targets.json").write_text(json.dumps(targets, indent=2) + "\n")
    (mdir / "provisioning.json").write_text(
        json.dumps({"route": route, "mode": mode, "polls": polls}, indent=2) + "\n")
    return body, targets


def recon_target(url, mdir, tid):
    """Cheap recon per target: GET / (root html + headers) and GET /api/clues (clues.json).
    Seeds the worker even when recovery fails. The private `_responses` entry lets the
    automated recoverer reuse these bytes instead of immediately repeating both requests;
    it is stripped before the report is persisted. Target traffic only - never the GM."""
    slug = re.sub(r"[^a-z0-9]+", "-", str(tid).lower()).strip("-") or "target"
    out = {"target": tid, "url": url, "_responses": {}}
    base = url.rstrip("/")
    try:
        req = urllib.request.Request(base + "/", headers={"User-Agent": recovery_lib.UA})
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read()
            out["root_status"] = r.status
            (mdir / f"{slug}.root.html").write_bytes(body)
            (mdir / f"{slug}.root.headers.json").write_text(
                json.dumps(dict(r.headers.items()), indent=2) + "\n")
            out["_responses"]["/"] = (r.status, body.decode("utf-8", "replace"))
    except Exception as e:  # noqa: BLE001
        out["root_status"] = f"error {e!r}"
    try:
        req = urllib.request.Request(base + "/api/clues",
                                     headers={"User-Agent": recovery_lib.UA})
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read()
            out["clues_status"] = r.status
            (mdir / f"{slug}.clues.json").write_bytes(body)
            out["_responses"]["/api/clues"] = (r.status, body.decode("utf-8", "replace"))
    except Exception as e:  # noqa: BLE001
        out["clues_status"] = f"error {e!r}"
    return out


def recover_answers(mdir, sj, recon=None):
    """classify_and_recover per target, joining questions[].artifact to targets[].id (never array
    order - mastery is cumulative and mixes earlier bug classes). Reuse recon's root/clues
    responses when available. Returns ({answer_id: proof}, {answer_id: evidence}, missing ids)."""
    mdir = Path(mdir)
    by_id = {t.get("id"): t for t in (sj.get("targets") or []) if isinstance(t, dict)}
    recon_by_id = {r.get("target"): r.get("_responses", {}) for r in (recon or [])}
    answers, evidence, missing = {}, {}, []
    for q in (sj.get("questions") or []):
        qid, tid = q.get("id"), q.get("artifact")
        tgt = by_id.get(tid)
        if not qid or not tgt or not tgt.get("url"):
            missing.append(qid or str(tid))
            continue
        ev = {}
        t = recovery_lib.Target(tgt["url"],
                                mdir / "evidence" / str(tid).replace(" ", "_"))
        try:
            proof = recovery_lib.classify_and_recover(
                t, ev, sj.get("prompt", ""), initial=recon_by_id.get(tid))
        except Exception as e:  # noqa: BLE001  one target's crash must not discard the others
            proof, ev["error"] = None, repr(e)
        evidence[qid] = {"target": tid, "url": tgt["url"], **ev}
        if proof:
            answers[qid] = proof
        else:
            missing.append(qid)
    return answers, evidence, missing


def first_pass(token_, base, rows, wt):
    """The automated first pass for one challenge family: per mode, serially, single_solve ->
    time_trial -> mastery, each mode started only once its prerequisite is CORRECT. Pure
    automation, no model. Untimed only: the only GM ops are status, start and submit."""
    routes = {}
    for c in rows:
        if c.get("mode") in wk.MODES:
            routes[c["mode"]] = c["id"]
    for mode, suffix in wk.MODE_SUFFIX.items():
        routes.setdefault(mode, base + suffix)
    report = {"challenge": base, "worker_dir": str(wt),
              "routes": [routes[m] for m in wk.MODES], "steps": []}
    correct = True
    for mode in wk.MODES:
        route = routes[mode]
        step = {"mode": mode, "route": route}
        if not correct:
            step["result"] = "WAITING (previous mode not CORRECT - fix it, then run this mode)"
            report["steps"].append(step)
            continue
        mdir = Path(wt) / mode
        # Live status first: a solved route is recognised without starting anything.
        try:
            code, raw = gm(token_, "GET", op="status", challenge=route)
        except RuntimeError as e:
            step["result"] = f"ERROR {e}"
            report["steps"].append(step)
            correct = False
            continue
        if code == 409 and isinstance(raw, dict) and raw.get("error") == "start_required":
            pass  # a fresh route has no attempt yet; begin_route performs the plain untimed start
        elif code != 200 or not isinstance(raw, dict):
            step["result"] = f"ERROR op=status -> HTTP {code}: {wk.short(raw)}"
            report["steps"].append(step)
            correct = False
            continue
        if wk.is_correct(raw) or wk.looks_solved(raw):
            step["result"] = "CORRECT (already solved - recognised from GM status)"
            step["via"] = "status"
            report["steps"].append(step)
            continue
        sj, note = begin_route(token_, route, mode, wt)
        if sj is None:
            step["result"] = f"WAITING ({note})"
            report["steps"].append(step)
            correct = False
            continue
        if wk.looks_solved(sj):
            step["result"] = "CORRECT (already solved - recognised from GM status)"
            step["via"] = "start"
            report["steps"].append(step)
            continue
        targets = [t for t in (sj.get("targets") or []) if isinstance(t, dict)]
        step["attempt_id"] = sj.get("attempt_id")
        step["targets"] = [t.get("id") for t in targets]
        recon = [recon_target(t.get("url"), mdir, t.get("id"))
                 for t in targets if t.get("url")]
        answers, evidence, missing = recover_answers(mdir, sj, recon)
        step["recon"] = [{k: v for k, v in row.items() if k != "_responses"} for row in recon]
        step["evidence"] = evidence
        answer_ids = sj.get("answer_ids") or []
        step["recovered_answer_ids"] = [qid for qid in answer_ids if qid in answers]
        if answers:
            # Partial recovery is valuable hand-off state too: wk.submit still enforces the
            # complete answer-id set, but the worker must not have to rediscover these proofs.
            wk.save_answers(mdir, answers)
            (mdir / "answers.meta.json").write_text(json.dumps({
                "attempt_id": sj.get("attempt_id"),
                "targets": {t.get("id"): t.get("url") for t in targets},
            }, indent=2) + "\n")
        if answer_ids and not missing and set(answers) == set(answer_ids):
            status, body = gm(token_, "POST", op="submit", challenge=route,
                              payload={"answers": answers})
            wk.save_round(mdir, answers, body)
            verdict = str(body.get("result", "")).upper() if isinstance(body, dict) else "?"
            step["result"] = "CORRECT (auto)" if verdict == "CORRECT" else verdict
            if isinstance(body, dict):
                step["submit"] = {k: body.get(k) for k in ("accepted_ids", "remaining")
                                  if k in body}
            if verdict == "CORRECT":
                wk.append_event(f"{route} correct")
            correct = verdict == "CORRECT"
        else:
            step["result"] = (f"needs work (auto-recovery found {len(answers)} of "
                              f"{len(answer_ids)} proofs; missing: "
                              f"{', '.join(missing) or '?'})")
            correct = False
        report["steps"].append(step)
    return report


def brief(day, report):
    base = report["challenge"]
    short, slug = short_name(base), bare_slug(base)
    sd, wt = state_dir(day), report["worker_dir"]
    done = bool(report["steps"]) and all(
        str(s.get("result", "")).startswith("CORRECT") for s in report["steps"])
    lines = [
        f"# Day {day} - {base}: first-pass brief",
        "",
        f"Worker directory: `{wt}` (cd there; `family.json` is the registry - wk.py takes the "
        f"challenge id from it, never from the command line).",
        f"Routes: {', '.join(report['routes']) or '(none)'}",
        "",
    ]
    for s in report["steps"]:
        lines.append(f"## {s.get('mode')}: {s.get('result')}")
        lines.append(f"mode dir: `{os.path.join(wt, str(s.get('mode')))}`")
        if s.get("attempt_id"):
            lines.append(f"attempt: `{s['attempt_id']}`")
        if s.get("targets"):
            lines.append(f"targets: {', '.join(str(t) for t in s['targets'])}")
        if s.get("recon"):
            bits = [f"{r.get('target')}: root={r.get('root_status')} "
                    f"clues={r.get('clues_status')}" for r in s["recon"]]
            lines.append("recon: " + "; ".join(bits))
        if s.get("recovered_answer_ids"):
            lines.append("recovered answer ids saved in answers.json: "
                         + ", ".join(s["recovered_answer_ids"]))
        for qid, ev in (s.get("evidence") or {}).items():
            cls = ev.get("classification") or ev.get("error") or "unclassified"
            lines.append(f"- {qid} [{ev.get('target')}] class={cls}")
        if s.get("submit"):
            lines.append(f"submit feedback: {json.dumps(s['submit'])}")
        lines.append("")
    tpl = open(os.path.join(HERE, "prompts",
                            "task-done.md" if done else "task.md")).read()
    lines.append(tpl.format(chal=base, wt=wt, day=day, slug=slug, state=sd, short=short))
    path = os.path.join(sd, f"brief-{short}.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    with open(os.path.join(sd, f"firstpass-{short}.json"), "w") as f:
        json.dump(report, f, indent=1)
    return path, done


# ---------------------------------------------------------------- watch / dispatch


def run_turn(sd, short, session_id, message, wt, timeout_s=CHALLENGE_TIMEOUT_S):
    """Block on one opencode turn for a challenge's session, marking running/done/timedout.
    This marker trio replaces the old Stop-hook stall detection entirely: "done" means the call
    returned, nothing more elaborate is tracked."""
    for marker in (f"done-{short}", f"timedout-{short}"):
        p = os.path.join(sd, marker)
        if os.path.exists(p):
            os.remove(p)
    open(os.path.join(sd, f"running-{short}"), "w").close()
    try:
        oc_run(session_id, message, wt, timeout_s=timeout_s)
        open(os.path.join(sd, f"done-{short}"), "w").close()
        log(f"{short}: agent turn finished -> done-{short}")
    except Exception as e:  # noqa: BLE001
        open(os.path.join(sd, f"timedout-{short}"), "w").close()
        log(f"{short}: turn failed/timed out ({e!r}) -> timedout-{short}")
    finally:
        try:
            os.remove(os.path.join(sd, f"running-{short}"))
        except OSError:
            pass


def dispatch_challenge(day, short, base, wt, brief_path, slot=None):
    """Point a session (a pre-warmed slot, or a fresh one for an extra challenge) at its
    challenge brief, then block on the whole challenge in one turn - single_solve through
    mastery, NOTES.md and the commit all happen inside this one call."""
    sd = state_dir(day)
    message = (f"Day {day} is live. Your challenge is `{base}`. Your worker directory is "
               f"`{wt}` - cd into it and work only there. Read {brief_path} and follow it.")
    if slot is not None:
        session_id = open(os.path.join(sd, f"slot-{slot}.session")).read().strip()
        sh(TMUX + ["rename-window", "-t", f"{SESSION}:slot-{slot}", short], check=False)
    else:
        try:
            session_id, _ = oc_new_session(
                f"web-d{day:02d}-{short}",
                "You are about to be assigned a web-exploitation challenge. Reply 'ready' and "
                "wait for the next message.",
                wt, timeout_s=60)
        except Exception as e:  # noqa: BLE001
            log(f"{short}: could not create a session ({e!r}) - brief is at {brief_path}")
            return
        attach_window(short, session_id, wt)
    with open(os.path.join(sd, f"{short}.session"), "w") as f:
        f.write(session_id + "\n")
    log(f"{short}: dispatched (window {SESSION}:{short}, session {session_id})")
    run_turn(sd, short, session_id, message, wt, timeout_s=CHALLENGE_TIMEOUT_S)


def watch(day, rehearse, n_workers=WORKER_SLOTS):
    rel = Release(day, rehearse).start()
    if not rel.released.wait(
            45 * 60 + (0 if rehearse
                       else max(0, (at(22, 59, 50) - datetime.now(AEST)).total_seconds()))):
        sys.exit("no release within 45 minutes")
    sd = state_dir(day)
    rows = rel.rows
    with open(routes_cache(day), "w") as f:
        json.dump({"challenges": rows}, f, indent=1)
        f.write("\n")
    fams = group_challenges(rows)
    log(f"{len(fams)} web challenge(s) for day {day}: {', '.join(b for b, _ in fams)}")

    have_session = sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0
    dispatch = have_session and not NO_AGENTS and not rehearse
    if not have_session and not NO_AGENTS and not rehearse:
        log("no tmux session - first pass and briefs only")
    if dispatch:
        for slot in range(len(fams) + 1, n_workers + 1):
            log(f"slot {slot}: standing down (fewer challenges than slots) - "
                f"closing its window (nothing to tell an idle session)")
            sh(TMUX + ["kill-window", "-t", f"{SESSION}:slot-{slot}"], check=False)

    tok = token()

    def challenge_job(idx, base, frows):
        t = time.time()
        short = short_name(base)
        wt = os.path.join(WORKERS, base)
        try:
            ensure_worker_dir(day, base, frows)
            rep = first_pass(tok, base, frows, wt)
        except Exception as e:  # noqa: BLE001  one challenge's crash must not discard the others
            rep = {"challenge": base, "worker_dir": wt, "routes": [],
                   "steps": [{"mode": "first pass", "result": f"CRASHED {e!r}"}]}
        path, done = brief(day, rep)
        log(f"{short}: first pass {'COMPLETE' if done else 'needs work'} "
            f"in {time.time() - t:.0f} s -> {path}")
        if not dispatch:
            return
        slot = idx + 1 if idx + 1 <= n_workers else None
        dispatch_challenge(day, short, base, wt, path, slot=slot)

    threads = [threading.Thread(target=challenge_job, args=(i, b, r))
               for i, (b, r) in enumerate(fams)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    if dispatch:
        log("every web challenge agent has finished its turn (or timed out) - "
            "`web_rush.py status` for the summary")
    else:
        log("first pass finished for every challenge; no agents dispatched")


# ---------------------------------------------------------------- takeover


def takeover(day, challenges):
    """Send one more message to a challenge's existing session - nothing is lost, since the
    tmux window is already attached to that same live session. A scripted alternative to just
    attaching tmux and typing into the pane directly."""
    sd = state_dir(day)
    threads = []
    for short in challenges:
        sess_path = os.path.join(sd, f"{short}.session")
        if not os.path.exists(sess_path):
            log(f"{short}: no recorded session (was it ever dispatched?) - nothing to take over")
            continue
        session_id = open(sess_path).read().strip()
        base = LANE_PREFIX + short
        wt = os.path.join(WORKERS, base)
        if os.path.exists(os.path.join(sd, f"running-{short}")):
            log(f"{short}: a turn looks like it may still be in flight - attaching tmux "
                f"(tmux -L huntress-web attach -t web) and typing into the pane is safer than "
                f"a concurrent nudge. Sending one anyway since takeover was requested.")
        fp = os.path.join(sd, f"brief-{short}.md")
        nudge = (f"Checking in on `{base}` in `{wt}`. "
                 + (f"Re-read {fp} and " if os.path.exists(fp) else "")
                 + "the mode directories, re-derive any wrong answers from the evidence, and "
                   "keep going - do not sit idle. When every mode is CORRECT, write NOTES.md "
                   "and commit your day directory. Report your status in a few lines.")
        log(f"{short}: takeover nudge -> session {session_id}")
        t = threading.Thread(target=run_turn,
                             args=(sd, short, session_id, nudge, wt, CHALLENGE_TIMEOUT_S))
        t.start()
        threads.append(t)
    for t in threads:
        t.join()


# ---------------------------------------------------------------- status


def day_challenges(day):
    """The day's challenge short names, from the route cache (assignment order = sorted base ids)."""
    return [short_name(b) for b, _ in group_challenges(cached_rows(day))]


def status(day):
    sd = state_dir(day)
    shorts = day_challenges(day)
    if not shorts:
        print(f"no routes cached for day {day} yet "
              f"(workers/web/routes-d{day:02d}.json)")
        return
    for short in shorts:
        fp = os.path.join(sd, f"firstpass-{short}.json")
        steps = json.load(open(fp))["steps"] if os.path.exists(fp) else []
        if os.path.exists(os.path.join(sd, f"done-{short}")):
            agent = "done"
        elif os.path.exists(os.path.join(sd, f"timedout-{short}")):
            agent = "TIMED OUT - see driver log, or run takeover"
        elif os.path.exists(os.path.join(sd, f"running-{short}")):
            agent = "working"
        else:
            agent = "not dispatched"
        summary = " | ".join("{}={}".format(s.get("mode"), s.get("result"))
                             for s in steps) or "-"
        print(f"{short:32} first pass: {summary}   agent: {agent}")


# ---------------------------------------------------------------- main


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    cmd = a[0]
    day = int(a[a.index("--day") + 1]) if "--day" in a else datetime.now(AEST).day
    rehearse = "--rehearse" in a
    n_workers = int(a[a.index("--workers") + 1]) if "--workers" in a else WORKER_SLOTS
    challenges = a[a.index("--challenges") + 1].split(",") if "--challenges" in a else None
    global NO_AGENTS, WORKER_MODEL, WORKER_VARIANT
    NO_AGENTS = "--no-agents" in a
    if "--model" in a:
        WORKER_MODEL = a[a.index("--model") + 1]
    if "--effort" in a:
        WORKER_VARIANT = a[a.index("--effort") + 1]
    if not (1 <= day <= 31):
        sys.exit(f"day {day} outside the event")
    log(f"web_rush {cmd}: day {day}{' (rehearsal)' if rehearse else ''}")
    if cmd == "all" and not rehearse:
        # The 22:45 run must relaunch fresh agents (new day): yesterday's session would otherwise
        # make launch() a no-op, and prep clears its state dir.
        if sh(TMUX + ["kill-session", "-t", SESSION], check=False).returncode == 0:
            log("killed the previous tmux session")
    if cmd in ("prep", "all"):
        prep(day)
    if cmd in ("launch", "all"):
        launch(day, n_workers)
    if cmd in ("watch", "all"):
        watch(day, rehearse, n_workers)
    if cmd == "status":
        status(day)
    if cmd == "takeover":
        if not challenges:
            sys.exit("takeover needs --challenges")
        takeover(day, challenges)


if __name__ == "__main__":
    main()
