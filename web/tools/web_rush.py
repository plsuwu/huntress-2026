#!/usr/bin/env python3
"""Day-release orchestrator for the web lane: one agent per challenge (Claude Code in tmux) plus an automated first pass.

  web_rush.py all     [--day N] [--rehearse] [--workers N]  prep + launch + watch (start it ~22:45 AEST; see docs/runbook.md)
  web_rush.py prep    [--day N]                             worker dirs + shared-tree symlinks, stale markers cleared
  web_rush.py launch  [--day N] [--workers N]               tmux session `web` (socket -L huntress-web): primed Claude per slot
  web_rush.py watch   [--day N] [--rehearse] [--no-agents]  wait for the release, first pass per challenge, dispatch briefs
  web_rush.py status  [--day N]                             first-pass results, challenge done-markers, stalled agents
  web_rush.py takeover --challenges a[,b] [--day N]         coordinator: fresh worker window + brief for a stalled challenge
Options: --workers N (pre-warmed slots, default 3), --no-agents (watch: first pass + briefs only, no tmux dispatch),
         --model NAME (worker Claude model, default claude-opus-4-8).

Release: 23:00 Australia/Brisbane; Day N = that date's day of the month. Work items are the day's web-exploitation
challenges, discovered at release (op=list&day=N&lane=web-exploitation, grouped into base challenges). Pre-warmed slot i
gets challenge i; more challenges than slots -> extra windows are primed on the spot, fewer -> spare slots stand down.
The first pass is pure automation, no model: per challenge, single_solve -> time_trial -> mastery, each mode started
only once its prerequisite is CORRECT, every start a plain untimed one. Each challenge's brief (routes, per-mode results,
recon evidence, submit feedback, instructions) is written to .rush-dNN/brief-<chal>.md and its agent is told to read it -
as soon as that challenge's first pass ends, not when all end. The first pass never sends run_for_time/retry/practice/
reset/switch_target either: the only GM ops are list (route discovery), status, start and submit, and the worker's only
GM door (tools/wk.py) has no timed code path at all. GM calls are direct - no lock, no proactive pacing (2026-10-24
decision: solution speed over gating; both this driver and wk.py back off reactively on 429/5xx).
Release detection: op=list&day=N&lane=web-exploitation every 1 s from 22:59:50 (the lane-filtered list answers in ~1 s;
a full all=true list takes 4-10 s - avoid it), released once two consecutive polls agree. After the briefs, stalled agents
(Stop hook after the brief, no tool activity since, no done marker, 180 s) are escalated to .rush-dNN/stalled-<chal>.md
for the coordinator, who runs `takeover` (or nudges the agent). `all` kills the previous day's tmux session before
relaunching. `--rehearse` skips the wait and replays the flow on an already-released day (solved routes are recognised
from GM status; no agent is dispatched).
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
WORKER_MODEL = "claude-opus-4-8"
NO_AGENTS = False
STALL_GRACE_S = 180  # an agent waiting on a background recovery also ends its turn with no tool activity

sys.path.insert(0, HERE)
import wk  # noqa: E402  the worker's GM helper: one shared GM-call implementation
sys.path.insert(0, os.path.join(ROOT, "solver_py_scripts"))
import recovery_lib  # noqa: E402  the first pass's classifiers/recoverers

SHARED_TREES = ("AGENTS.md", "docs", "solver_py_scripts", "tools")


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
    # code path at all. The token rides the tmux environment only; it is never sent to a target.
    return dict(
        os.environ, HUNTRESS_GM_TOKEN=token(), HUNTRESS_GM_URL=API, WEB_UNTIMED_ONLY="1"
    )


def envflags():
    e = env()
    return sum(
        (["-e", f"{k}={e[k]}"]
         for k in ("HUNTRESS_GM_TOKEN", "HUNTRESS_GM_URL", "WEB_UNTIMED_ONLY")),
        [],
    )


def redact(text):
    """Never let the GM token reach a log line: the tmux argv carries it (-e), and a failed
    tmux call's error message would otherwise print the whole command."""
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


def sh(args, cwd=None, check=True, **kw):
    r = subprocess.run(args, cwd=cwd, text=True, capture_output=True, **kw)
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
    root is never a worker's cwd or an --add-dir: it holds the jumpbox SSH key and .env."""
    wt = os.path.join(WORKERS, base)
    os.makedirs(wt, exist_ok=True)
    for rel in SHARED_TREES:
        link = os.path.join(wt, rel)
        src = os.path.join(ROOT, rel)
        if not os.path.lexists(link) and os.path.lexists(src):
            os.symlink(src, link)
    with open(os.path.join(wt, "family.json"), "w") as f:
        json.dump({"base": base, "release_day": day, "routes": rows}, f, indent=2)
        f.write("\n")
    return wt


def prep(day):
    """Worker directories for every challenge already known for the day (the route cache), the
    shared-tree symlinks, and a clean marker set. On release night the cache is empty - watch
    creates the worker dirs at the release."""
    os.makedirs(WORKERS, exist_ok=True)
    sd = state_dir(day)
    if os.path.isdir(sd):
        shutil.rmtree(sd)  # a same-day re-run must not inherit stale markers
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
    for rel in SHARED_TREES:
        src = os.path.join(ROOT, rel)
        if os.path.lexists(src):
            os.symlink(src, os.path.join(d, rel))
    return d


def prime_prompt(day, slot, n, wt):
    tpl = open(os.path.join(HERE, "prompts", "prime.md")).read()
    return tpl.format(i=slot, n=n, day=day, wt=wt)


def agent_settings(sd, slot):
    """Hooks that let the watcher see a worker stop: Stop touches stopped-<chal>; every tool call
    and prompt touches active-<chal> (heartbeat). The marker name resolves through slot-<slot>.chal,
    which holds "slot-<slot>" until the release assignment rewrites it with the challenge's short
    name - so the prime turn's stops can never start a stall clock."""
    chal = os.path.join(sd, f"slot-{slot}.chal")

    def touch(kind):
        return {
            "type": "command",
            "command": (f"touch {shlex.quote(os.path.join(sd, kind + '-'))}"
                        f'"$(cat {shlex.quote(chal)} 2>/dev/null || echo slot-{slot})"'),
        }

    return {
        "hooks": {
            "Stop": [{"hooks": [touch("stopped")]}],
            "PostToolUse": [{"matcher": "*", "hooks": [touch("active")]}],
            "UserPromptSubmit": [{"hooks": [touch("active")]}],
        }
    }


def agent_settings_direct(sd, name):
    """The same hooks for a window whose challenge is already known (extra release windows,
    takeover): the marker name is baked in."""

    def touch(kind):
        return {
            "type": "command",
            "command": f"touch {shlex.quote(os.path.join(sd, f'{kind}-{name}'))}",
        }

    return {
        "hooks": {
            "Stop": [{"hooks": [touch("stopped")]}],
            "PostToolUse": [{"matcher": "*", "hooks": [touch("active")]}],
            "UserPromptSubmit": [{"hooks": [touch("active")]}],
        }
    }


def launcher_script(day, name, prompt_file, settings_file):
    allow = f'"Bash(python3 {os.path.join(ROOT, "tools", "wk.py")} *)"'
    adddir = "--add-dir " + " ".join(f'"{p}"' for p in (os.path.join(ROOT, "docs"),
                                         os.path.join(ROOT, "solver_py_scripts"),
                                         os.path.join(ROOT, "tools")))
    return (
        "#!/bin/sh\n"
        # --add-dir: docs, the shared solver tree and tools/ - never the repo root (that holds
        # the jumpbox SSH key and .env). Model/effort pinned here: the worker's cwd carries no
        # settings file, so it would otherwise inherit whatever ~/.claude/settings.json says.
        # --add-dir and --allowedTools are variadic: -n must follow them so the prompt stays
        # positional. The wk.py rule covers the worker's only GM path so the auto-mode
        # classifier cannot stall a submit (the Day 16 lesson).
        f"exec claude --model {WORKER_MODEL} --effort high --permission-mode auto {adddir} "
        f"--allowedTools {allow} --settings {shlex.quote(settings_file)} "
        f'-n "web-d{day:02d}-{name}" "$(cat {shlex.quote(prompt_file)})"\n'
    )


def launch(day, n_workers=WORKER_SLOTS):
    have = sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0
    if have:
        log("tmux session already exists - not relaunching "
            "(tmux -L huntress-web kill-session -t web to reset)")
        return
    sd = state_dir(day)
    sh(TMUX + ["new-session", "-d", "-s", SESSION, "-n", "control", "-c", WORKERS]
       + envflags())
    for slot in range(1, n_workers + 1):
        cwd = ensure_slot_dir(slot)
        with open(os.path.join(sd, f"slot-{slot}.chal"), "w") as f:
            f.write(f"slot-{slot}\n")  # pre-release marker name; the assignment rewrites it
        prompt_file = os.path.join(sd, f"prime-{slot}.md")
        with open(prompt_file, "w") as f:
            f.write(prime_prompt(day, slot, n_workers, cwd))
        settings_file = os.path.join(sd, f"settings-{slot}.json")
        with open(settings_file, "w") as f:
            json.dump(agent_settings(sd, slot), f, indent=1)
        launcher = os.path.join(sd, f"launch-{slot}.sh")
        with open(launcher, "w") as f:
            f.write(launcher_script(day, f"slot-{slot}", prompt_file, settings_file))
        os.chmod(launcher, 0o755)
        sh(TMUX + ["new-window", "-t", SESSION, "-n", f"slot-{slot}", "-c", cwd]
           + envflags() + [launcher])
        log(f"slot {slot}: worker launched (window {SESSION}:slot-{slot}, cwd {cwd})")


def send(window, text):
    # Claude Code can leave a picker open after the prime turn ("Teach auto mode about your
    # environment?" - seen in the main pipeline's Day 17 smoke test). Typed text would answer it
    # and Enter confirms, so the brief is lost. Dismiss only an open picker: Escape at a busy
    # agent would interrupt it.
    for _ in range(3):
        pane = sh(TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{window}"],
                  check=False).stdout
        if "Esc to cancel" not in "\n".join(pane.rstrip().splitlines()[-12:]):
            break
        sh(TMUX + ["send-keys", "-t", f"{SESSION}:{window}", "Escape"])
        time.sleep(0.5)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{window}", "-l", text])
    time.sleep(0.3)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{window}", "Enter"])


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
    Seeds the worker even when auto-recovery fails. Target traffic only - never the GM."""
    slug = re.sub(r"[^a-z0-9]+", "-", str(tid).lower()).strip("-") or "target"
    out = {"target": tid, "url": url}
    base = url.rstrip("/")
    try:
        req = urllib.request.Request(base + "/", headers={"User-Agent": recovery_lib.UA})
        with urllib.request.urlopen(req, timeout=30) as r:
            out["root_status"] = r.status
            (mdir / f"{slug}.root.html").write_bytes(r.read())
            (mdir / f"{slug}.root.headers.json").write_text(
                json.dumps(dict(r.headers.items()), indent=2) + "\n")
    except Exception as e:  # noqa: BLE001
        out["root_status"] = f"error {e!r}"
    try:
        req = urllib.request.Request(base + "/api/clues",
                                     headers={"User-Agent": recovery_lib.UA})
        with urllib.request.urlopen(req, timeout=30) as r:
            out["clues_status"] = r.status
            (mdir / f"{slug}.clues.json").write_bytes(r.read())
    except Exception as e:  # noqa: BLE001
        out["clues_status"] = f"error {e!r}"
    return out


def recover_answers(mdir, sj):
    """classify_and_recover per target, joining questions[].artifact to targets[].id (never array
    order - mastery is cumulative and mixes earlier bug classes). Returns
    ({answer_id: proof}, {answer_id: evidence}, missing answer ids)."""
    mdir = Path(mdir)
    by_id = {t.get("id"): t for t in (sj.get("targets") or []) if isinstance(t, dict)}
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
            proof = recovery_lib.classify_and_recover(t, ev, sj.get("prompt", ""))
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
        if code != 200 or not isinstance(raw, dict):
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
        step["recon"] = [recon_target(t.get("url"), mdir, t.get("id"))
                         for t in targets if t.get("url")]
        answers, evidence, missing = recover_answers(mdir, sj)
        step["evidence"] = evidence
        answer_ids = sj.get("answer_ids") or []
        if answer_ids and not missing and set(answers) == set(answer_ids):
            wk.save_answers(mdir, answers)
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


# ---------------------------------------------------------------- watch


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
            log(f"slot {slot}: standing down (fewer challenges than slots)")
            try:
                send(f"slot-{slot}",
                     f"Day {day} is live, but there are fewer web challenges than workers - "
                     f"you are not needed. Stand down: reply 'stood down' and stop.")
                time.sleep(2)
                sh(TMUX + ["kill-window", "-t", f"{SESSION}:slot-{slot}"], check=False)
            except Exception as e:  # noqa: BLE001
                log(f"slot {slot}: stand-down failed ({e!r})")

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
        try:
            if idx + 1 <= n_workers:
                assign_window(day, idx + 1, short, base, frows, path)
            else:
                launch_extra(day, short, base, frows, path)
        except Exception as e:  # noqa: BLE001
            log(f"{short}: could not reach the agent pane ({e!r}) - brief is at {path}")

    threads = [threading.Thread(target=challenge_job, args=(i, b, r))
               for i, (b, r) in enumerate(fams)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    if dispatch:
        log("first pass finished for every challenge; agents are working - "
            "`web_rush.py status` to follow")
        monitor(day)
    else:
        log("first pass finished for every challenge; no agents dispatched")


def assign_window(day, slot, short, base, rows, brief_path):
    """Point a pre-warmed slot at its challenge: rewrite the slot's marker name, rename the
    window, write the sent marker (the stall clock starts here), and send the assignment."""
    sd = state_dir(day)
    wt = os.path.join(WORKERS, base)
    title = rows[0].get("title") or rows[0].get("name") or base
    with open(os.path.join(sd, f"slot-{slot}.chal"), "w") as f:
        f.write(short + "\n")
    sh(TMUX + ["rename-window", "-t", f"{SESSION}:slot-{slot}", short], check=False)
    open(os.path.join(sd, f"sent-{short}"), "w").close()
    send(short, f"Day {day} is live. Your challenge is `{base}` ({title}). Your worker "
                f"directory is `{wt}` - cd into it and work only there. "
                f"Read {brief_path} and follow it.")
    log(f"{short}: assigned to slot {slot} (window {SESSION}:{short})")


def launch_extra(day, short, base, rows, brief_path):
    """A day released more challenges than pre-warmed slots: prime an extra worker window on the
    spot (no pre-warm - the assignment prompt carries the challenge id)."""
    sd = state_dir(day)
    wt = os.path.join(WORKERS, base)
    title = rows[0].get("title") or rows[0].get("name") or base
    settings_file = os.path.join(sd, f"settings-{short}.json")
    with open(settings_file, "w") as f:
        json.dump(agent_settings_direct(sd, short), f, indent=1)
    prompt_file = os.path.join(sd, f"assign-{short}.md")
    with open(prompt_file, "w") as f:
        f.write(f"Day {day} is live. Your challenge is `{base}` ({title}). Your worker "
                f"directory is `{wt}` (your current directory - you are already in it). "
                f"Read {brief_path} and follow it.\n")
    launcher = os.path.join(sd, f"launch-{short}.sh")
    with open(launcher, "w") as f:
        f.write(launcher_script(day, short, prompt_file, settings_file))
    os.chmod(launcher, 0o755)
    sh(TMUX + ["new-window", "-t", SESSION, "-n", short, "-c", wt]
       + envflags() + [launcher])
    open(os.path.join(sd, f"sent-{short}"), "w").close()
    log(f"{short}: extra worker launched (window {SESSION}:{short})")


# ---------------------------------------------------------------- stalls / takeover


def mtime(path):
    try:
        return os.path.getmtime(path)
    except OSError:
        return None


def stall_since(sd, short, now=None):
    """Seconds since the challenge's agent stopped without finishing, or None. Stalled = its last
    Stop came after the brief was sent, no tool call / prompt since (active-<short>), and no done
    marker."""
    now = now or time.time()
    sent, stop, act = (mtime(os.path.join(sd, f"{k}-{short}"))
                       for k in ("sent", "stopped", "active"))
    if (os.path.exists(os.path.join(sd, f"done-{short}"))
            or sent is None or stop is None or stop < sent):
        return None
    if act is not None and act > stop:
        return None
    return now - stop


def day_challenges(day):
    """The day's challenge short names, from the route cache (assignment order = sorted base ids)."""
    return [short_name(b) for b, _ in group_challenges(cached_rows(day))]


def monitor(day, until_s=90 * 60, every_s=10):
    """Escalate stalled challenge agents to the coordinator (stalled-<chal>.md + log); never acts
    on the worker dir itself: a false positive must not race a live agent. The coordinator runs
    `web_rush.py takeover` (or nudges the agent)."""
    sd, end, flagged = state_dir(day), time.time() + until_s, {}
    while time.time() < end:
        pending = [s for s in day_challenges(day)
                   if not os.path.exists(os.path.join(sd, f"done-{s}"))]
        if not pending:
            log("every web challenge agent is done")
            return
        for short in pending:
            idle = stall_since(sd, short)
            stop = mtime(os.path.join(sd, f"stopped-{short}"))
            if idle is not None and idle >= STALL_GRACE_S and flagged.get(short) != stop:
                flagged[short] = stop
                pane = sh(TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{short}",
                                  "-S", "-60"], check=False).stdout
                path = os.path.join(sd, f"stalled-{short}.md")
                with open(path, "w") as f:
                    f.write(f"# {short}: agent stopped {idle:.0f} s ago without "
                            f"done-{short}\n\n"
                            f"Take over: `python3 tools/web_rush.py takeover --day {day} "
                            f"--challenges {short}` (or message the agent: "
                            f"tmux -L huntress-web send-keys -t web:{short} ...)\n\n"
                            "Pane tail:\n```\n"
                            + "\n".join(pane.rstrip().splitlines()[-40:]) + "\n```\n")
                log(f"{short}: STALLED (stopped {idle:.0f} s ago, no done marker) -> {path}")
        time.sleep(every_s)
    log("monitor window over")


def takeover(day, challenges):
    """Coordinator tool for a stalled challenge: capture the pane tail + GM status per route + a
    listing of the worker dir, write a takeover brief, kill the stalled window, and launch a fresh
    worker window whose prompt is the brief. wk.py begin is idempotent (a plain start reconnects),
    so the replacement resumes the same attempt with no lost work."""
    sd = state_dir(day)
    tok = token()
    for short in challenges:
        base = LANE_PREFIX + short
        wt = os.path.join(WORKERS, base)
        lines = [f"# takeover: day {day} {short} ({wt})", ""]
        pane = sh(TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{short}", "-S", "-80"],
                  check=False).stdout
        lines += ["## Pane tail", "```", *pane.rstrip().splitlines()[-40:], "```", ""]
        routes = []
        fam_path = os.path.join(wt, "family.json")
        if os.path.exists(fam_path):
            try:
                fam = json.load(open(fam_path))
                routes = [(r.get("mode"), r.get("id"))
                          for r in fam.get("routes", []) if isinstance(r, dict)]
            except ValueError:
                lines.append(f"family.json is not valid JSON - rebuild it with prep")
        for mode, route in routes:
            try:
                raw = wk.route_status(tok, route)
                extra = ""
                if isinstance(raw, dict):
                    bits = [f"{k}={raw[k]}" for k in ("attempt_id",
                                                      "ranked_attempts_remaining",
                                                      "regular_variants_remaining")
                            if k in raw]
                    extra = f" [{', '.join(bits)}]" if bits else ""
                lines.append(f"- {mode} `{route}`: gm state={wk.norm_state(raw)}{extra}")
            except SystemExit as e:
                lines.append(f"- {mode} `{route}`: wk.py exited {e.code} (see stderr)")
        dirs = [wt] + [os.path.join(wt, m) for m, _ in routes
                       if os.path.isdir(os.path.join(wt, m))]
        listing = sh(["ls", "-la", *dirs], check=False)
        lines += ["", "## Worker dir", "```",
                  *((listing.stdout or listing.stderr).splitlines()[-40:]), "```", ""]
        fp = os.path.join(sd, f"brief-{short}.md")
        if os.path.exists(fp):
            lines += [f"First-pass brief: `{fp}` - read it as well.", ""]
        tpl = open(os.path.join(HERE, "prompts", "task.md")).read()
        lines.append(tpl.format(chal=base, wt=wt, day=day, slug=bare_slug(base),
                                state=sd, short=short))
        path = os.path.join(sd, f"takeover-{short}.md")
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
        log(f"{short}: takeover brief -> {path}")
        settings_file = os.path.join(sd, f"settings-{short}.json")
        with open(settings_file, "w") as f:
            json.dump(agent_settings_direct(sd, short), f, indent=1)
        launcher = os.path.join(sd, f"launch-{short}.sh")
        with open(launcher, "w") as f:
            f.write(launcher_script(day, short, path, settings_file))
        os.chmod(launcher, 0o755)
        try:
            sh(TMUX + ["kill-window", "-t", f"{SESSION}:{short}"], check=False)
            sh(TMUX + ["new-window", "-t", SESSION, "-n", short, "-c", wt]
               + envflags() + [launcher])
            log(f"{short}: fresh worker launched (window {SESSION}:{short})")
        except Exception as e:  # noqa: BLE001
            log(f"{short}: could not relaunch a window ({e!r}) - the brief is at {path}")


# ---------------------------------------------------------------- status


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
        done = os.path.exists(os.path.join(sd, f"done-{short}"))
        idle = stall_since(sd, short)
        if done:
            agent = "done"
        elif idle is not None:
            agent = f"STOPPED {idle:.0f} s ago (no done marker)"
        elif mtime(os.path.join(sd, f"sent-{short}")) is None:
            agent = "not dispatched"
        else:
            agent = "working"
        if not done and os.path.exists(os.path.join(sd, f"takeover-{short}.md")):
            agent += " | takeover report written"
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
    global NO_AGENTS, WORKER_MODEL
    NO_AGENTS = "--no-agents" in a
    if "--model" in a:
        WORKER_MODEL = a[a.index("--model") + 1]
    if not (1 <= day <= 31):
        sys.exit(f"day {day} outside the event")
    log(f"web_rush {cmd}: day {day}{' (rehearsal)' if rehearse else ''}")
    if cmd == "all" and not rehearse:
        # The 22:45 run must relaunch fresh agents (new flags/hooks, new day): yesterday's
        # session would otherwise make launch() a no-op, and prep clears its state dir.
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
