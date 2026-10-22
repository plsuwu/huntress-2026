#!/usr/bin/env python3
"""Day-release orchestrator: one agent per lane (in tmux) plus an automated first pass. Everything event-specific
comes from config.toml (tools/orchcfg.py); only registered lanes are ever touched.

  rush.py all     [--day N] [--rehearse]   prep + launch + watch (start it ~15 min before the release)
  rush.py prep    [--day N]                reset the lane worktrees to the main branch, build every target warm
  rush.py launch  [--day N]                tmux session: one primed agent per lane
  rush.py watch   [--day N] [--rehearse]   wait for the release, run the first pass per lane, dispatch briefs
  rush.py status  [--day N]                first-pass results, lane done markers, stalled agents
  rush.py takeover --lanes a[,b] [--day N]  coordinator: finish a stalled lane from its worktree (untimed/allowed only)
Options: --lanes a,b (subset), --no-agents (watch: first pass + briefs only, no tmux), --branch B (default: main or
master, whichever exists).

First pass (no model): per lane, single-solve; once it is CORRECT, the timed modes - together, or one after another
for modes a lane lists as `sequential`. Dual-pool routes get an untimed regular attempt; timed-by-default routes a
timed start; one-shots (ranked pool 1) need the one-shot flag (policy: solves beat leaderboard entries - remove that
branch in mode_args() if your policy differs). A timed-by-default/one-shot failure gets an untimed --practice
continuation; untimed failures get the grouped oracle. Each lane's brief is written to <runs>/rush/brief-<lane>.md and
the agent is told to read it as soon as that lane's first pass ends.
Every solver call here and in the agents' panes runs with the no-ranked guard set: dual-pool leaderboard runs stay
with the coordinator's campaign.
`--rehearse`: skip the wait and replay the flow on an already-released day (solved routes only reconnect).
"""

import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import orchcfg  # noqa: E402

C = orchcfg.load()
TZ = timezone(timedelta(hours=C["event"]["tz_offset_hours"]))
REL_H, REL_M = map(int, C["event"]["release"].split(":"))
A = C["agents"]
TMUX = ["tmux", "-L", A["tmux_socket"]]
SESSION = A["session"]
LANES = {l["name"]: l for l in C.lanes}
NO_AGENTS = False
BRANCH = None
SINGLE, TIMED = C.single_mode, C.timed_modes
F = C["solver"]["flags"]


def log(msg):
    print(f"[{datetime.now(TZ):%H:%M:%S}] {msg}", flush=True)


def sh(args, cwd=None, check=True, **kw):
    r = subprocess.run(args, cwd=cwd, text=True, capture_output=True, **kw)
    if check and r.returncode != 0:
        raise RuntimeError(f"{' '.join(args)}: {r.stderr.strip()[-400:]}")
    return r


def main_branch():
    for b in ("main", "master"):
        if sh(["git", "rev-parse", "-q", "--verify", b], cwd=C.repo, check=False).returncode == 0:
            return b
    sys.exit("no main/master branch; pass --branch")


def state_dir(day):
    d = os.path.join(C.runs_dir(day), "rush")
    os.makedirs(d, exist_ok=True)
    return d


def lane_solver_dir(lane):
    return os.path.join(C.worktree(lane), C["solver"]["dir"])


def placeholders(day, lane):
    l = LANES[lane]
    return dict(
        day=day, prev=day - 1, lane=lane, prefix=l["prefix"], src=l["source"], fix=l["fixtures"],
        wt=C.worktree(lane), repo=C.repo, state=state_dir(day), solver=C["solver"]["dir"],
        bin=C["solver"]["bin"], runs=C["solver"]["runs_dir"].format(day=day),
        single=SINGLE, modes=", ".join(TIMED), flags=F, no_ranked=f"{C['solver']['no_ranked_env']}={C['solver']['no_ranked_value']}",
    )


# ---------------------------------------------------------------- prep


def prep(day):
    """Persistent worktrees (agent folder trust is accepted once), reset to the main branch, built warm."""
    base = BRANCH or main_branch()
    for lane in LANES:
        wt = C.worktree(lane)
        os.makedirs(os.path.dirname(wt), exist_ok=True)
        branch = C.fmt(A["branch"], day=day, lane=lane)
        if not os.path.isdir(wt):
            sh(["git", "worktree", "add", "-f", wt, "-B", branch, base], cwd=C.repo)
        else:
            if sh(["git", "status", "--porcelain"], cwd=wt).stdout.strip():
                log(f"{lane}: worktree dirty - stashing before reset")
                sh(["git", "stash", "push", "-u", "-m", f"rush day {day} prep"], cwd=wt)
            sh(["git", "checkout", "-B", branch, base], cwd=wt)
        # Git-ignored, shared with the main checkout (fixture manifests point at main's artifacts, the env file).
        for rel in (C.fmt(p) for p in A["shared_links"]):
            link, src = os.path.join(wt, rel), os.path.join(C.repo, rel)
            if not os.path.lexists(link) and os.path.lexists(src):
                os.symlink(src, link)
            # Keep the symlink out of the lane's commits (worktrees read excludes from the common git dir).
            common = os.path.join(wt, sh(["git", "rev-parse", "--git-common-dir"], cwd=wt).stdout.strip())
            exclude = os.path.join(common, "info", "exclude")
            os.makedirs(os.path.dirname(exclude), exist_ok=True)
            have = open(exclude).read().splitlines() if os.path.exists(exclude) else []
            if f"/{rel}" not in have:
                with open(exclude, "a") as f:
                    f.write(f"/{rel}\n")
        log(f"{lane}: worktree {wt} on {branch}")
    for where in [C.solver_dir] + [lane_solver_dir(l) for l in LANES]:
        t = time.time()
        sh(C["solver"]["build"], cwd=where)
        log(f"built {where} in {time.time() - t:.0f} s")


# ---------------------------------------------------------------- launch


def prime_prompt(day, lane):
    return open(os.path.join(HERE, "prompts", "prime.md")).read().format(**placeholders(day, lane))


def allowed_tools(lane):
    """Allow rules for a lane agent: its own worktree's solver binary, relative and absolute. Permission classifiers
    have refused plain untimed starts without an explicit rule. `VAR=... ` or `cd <wt> && ` prefixes don't match."""
    return [f"Bash(./{C['solver']['bin']} *)", f"Bash({C.solver_bin(lane_solver_dir(lane))} *)"]


def agent_settings(day, lane):
    """Stop touches stopped-<lane>; every tool call and prompt touches active-<lane> (heartbeat). A stop after the
    brief, with no activity since and no done marker, is a stall."""
    sd = state_dir(day)
    touch = lambda name: {"type": "command", "command": f"touch {shlex.quote(os.path.join(sd, name))}"}
    return {
        "hooks": {
            "Stop": [{"hooks": [touch(f"stopped-{lane}")]}],
            "PostToolUse": [{"matcher": "*", "hooks": [touch(f"active-{lane}")]}],
            "UserPromptSubmit": [{"hooks": [touch(f"active-{lane}")]}],
        }
    }


def launcher_script(day, lane, prompt_file, settings_file):
    allow = " ".join(f'"{r}"' for r in allowed_tools(lane))
    # Read access to shared data only - never the repo root if it holds run-host credentials: the no-ranked guard only
    # protects this machine, so agents must not be able to reach a ranked run elsewhere.
    dirs = " ".join(f'"{os.path.join(C.repo, C.fmt(d))}"' for d in A["add_dirs"])
    # Model/effort pinned: worktrees carry no settings file and would inherit user-level defaults.
    # --add-dir and --allowedTools are variadic: -n must follow them so the prompt stays positional.
    return (
        "#!/bin/sh\n"
        f"exec {A['command']} --model {A['model']} --effort {A['effort']} --permission-mode {A['permission_mode']} "
        f"--add-dir {dirs} --allowedTools {allow} --settings \"{settings_file}\" -n \"d{day:02d}-{lane}\" "
        f"\"$(cat {prompt_file})\"\n"
    )


def launch(day):
    if sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0:
        log(f"tmux session exists - not relaunching ({' '.join(TMUX)} kill-session -t {SESSION} to reset)")
        return
    sd, e = state_dir(day), C.solver_env()
    keep = (C["event"]["token_env"], C["event"]["url_env"], C["solver"]["no_ranked_env"])
    envflags = sum((["-e", f"{k}={e[k]}"] for k in keep), [])
    sh(TMUX + ["new-session", "-d", "-s", SESSION, "-n", "control", "-c", C.solver_dir] + envflags)
    for lane in LANES:
        prompt_file = os.path.join(sd, f"prime-{lane}.md")
        with open(prompt_file, "w") as f:
            f.write(prime_prompt(day, lane))
        settings_file = os.path.join(sd, f"settings-{lane}.json")
        with open(settings_file, "w") as f:
            json.dump(agent_settings(day, lane), f, indent=1)
        launcher = os.path.join(sd, f"launch-{lane}.sh")
        with open(launcher, "w") as f:
            f.write(launcher_script(day, lane, prompt_file, settings_file))
        os.chmod(launcher, 0o755)
        sh(TMUX + ["new-window", "-t", SESSION, "-n", lane, "-c", C.worktree(lane)] + envflags + [launcher])
        log(f"{lane}: agent launched (window {SESSION}:{lane})")


def send(lane, text):
    """An agent can leave a picker open after a turn; typed text would answer it and the message is lost. Dismiss
    only an open picker: Escape at a busy agent would interrupt it."""
    for _ in range(3):
        pane = sh(TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{lane}"], check=False).stdout
        if "Esc to cancel" not in "\n".join(pane.rstrip().splitlines()[-12:]):
            break
        sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "Escape"])
        time.sleep(0.5)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "-l", text])
    time.sleep(0.3)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "Enter"])


# ---------------------------------------------------------------- watch / first pass


def at(h, m, s):
    return datetime.now(TZ).replace(hour=h, minute=m, second=s, microsecond=0)


def release_at(delta_s=0):
    return at(REL_H, REL_M, 0) + timedelta(seconds=delta_s)


def sleep_until(t):
    while datetime.now(TZ) < t:
        time.sleep(min(5, max(0.05, (t - datetime.now(TZ)).total_seconds())))


def probe_released(day):
    """op=status on a predictable day route: the configured "unreleased" error before the release; anything else
    (200, a 409 like start_required, another 404 body) counts as released. 401/403/429/5xx raise (keep polling)."""
    lane = next((l for l in C.lanes if l.get("release_probe")), None)
    if not lane:
        return False  # the list poll alone detects the release
    try:
        C.api_get("status", timeout=5, id=f"{lane['prefix']}{day:02d}")
        return True
    except urllib.error.HTTPError as e:
        if e.code == 409:
            return True
        if e.code != 404:
            raise
        try:
            err = json.loads(e.read() or b"{}").get("error")
        except ValueError:
            err = None
        return err != C["api"]["unreleased_error"]


class Release:
    """Two detectors: a status probe every 1 s from 10 s before the release (3 s after +60 s), and a day list poll
    every 3 s from 90 s before (slow on a released day, so lanes fetch their own rows; it is the fallback)."""

    def __init__(self, day, rehearse):
        self.day, self.rehearse = day, rehearse
        self.released, self.listed, self.rows = threading.Event(), threading.Event(), []

    def start(self):
        for fn in (self._status, self._list):
            threading.Thread(target=fn, daemon=True).start()
        return self

    def _status(self):
        if not self.rehearse:
            sleep_until(release_at(-10))
        while not self.released.is_set():
            t = time.time()
            try:
                if probe_released(self.day):
                    log(f"day {self.day} released (status probe)")
                    self.released.set()
                    return
            except Exception as e:  # noqa: BLE001
                log(f"status probe failed: {e!r}")
            every = 1 if self.rehearse or datetime.now(TZ) < release_at(60) else 3
            time.sleep(max(0.05, every - (time.time() - t)))

    def _list(self):
        if not self.rehearse:
            sleep_until(release_at(-90))
        while True:
            try:
                rows = [c for c in C.api_get("list_day", day=self.day).get("challenges", [])
                        if c.get("day") == self.day and C.in_scope(c.get("id", ""))]
                if rows:
                    if not self.released.is_set():
                        log(f"day {self.day} released (list: {len(rows)} in-scope routes)")
                    self.rows = rows
                    self.listed.set()
                    self.released.set()
                    return
            except Exception as e:  # noqa: BLE001
                log(f"list failed: {e!r}")
            time.sleep(3)


def lane_rows(day, lane, rel, tries_s=20):
    """The lane's own routes right after the release via a prefix search; retried for tries_s (the probe can turn
    before every lane is listable), then the global list. Filtered by id prefix: search matches substrings."""
    mine = lambda rows: [c for c in rows if c.get("day") == day and C.owns_day_route(lane, day, c["id"])]
    end = time.time() + tries_s
    while time.time() < end:
        try:
            rows = mine(C.api_get("search", timeout=15, day=day, q=C.day_prefix(lane, day)).get("challenges", []))
            if rows:
                return rows, "search"
        except Exception as e:  # noqa: BLE001
            log(f"{lane}: search failed: {e!r}")
        time.sleep(1)
    log(f"{lane}: search found nothing in {tries_s} s - waiting for the global list")
    rel.listed.wait()
    return mine(rel.rows), "list"


_CACHE_LOCK = threading.Lock()


def merge_routes(rows):
    """Merge rows into the solver's route cache. Lanes merge concurrently: the lock spans read-modify-write."""
    with _CACHE_LOCK:
        path = C.route_cache()
        os.makedirs(os.path.dirname(path), exist_ok=True)
        try:
            cache = json.load(open(path))
        except (OSError, ValueError):
            cache = {"challenges": []}
        ids = {c["id"] for c in rows}
        cache["challenges"] = [c for c in cache.get("challenges", []) if c.get("id") not in ids] + rows
        with open(path + ".tmp", "w") as f:
            json.dump(cache, f)
        os.replace(path + ".tmp", path)  # atomic: agents may read it concurrently


def solver_run(args, out_dir, where=None):
    """Run the solver from `where` (the main checkout, or a lane worktree for takeover) with the no-ranked guard."""
    where = where or C.solver_dir
    R = C["solver"]["result"]
    r = subprocess.run([C.solver_bin(where)] + args + [F["out"], out_dir], cwd=where, text=True,
                       capture_output=True, env=C.solver_env(), timeout=900)
    res = re.search(R["submit_re"], r.stderr)
    saved = re.search(R["saved_re"], r.stderr)
    errors = [l for l in r.stderr.splitlines()
              if any(m in l for m in R["error_contains"]) or l.startswith(tuple(R["error_prefixes"]))]
    result = res.group(1) if res else ("ERROR" if r.returncode else "?")
    if result == "already_solved":  # rehearsals: the attempt was solved before
        result = "CORRECT"
    return {"result": result, "dir": saved and saved.group(1), "errors": errors[:40],
            "tail": r.stderr.strip().splitlines()[-8:]}


def route_kind(cfg):
    if cfg.get("run_for_time_supported"):
        return "dual-pool"
    return "ONE-SHOT" if cfg.get("ranked_pool_size") == 1 else "timed by default"


def mode_args(single, mode, cfg):
    """Dual-pool: plain start = untimed regular attempt. Timed by default: plain start is timed. One-shot (ranked pool
    1, the route's only attempt): needs the one-shot flag."""
    args = [single, C.mode_flag(mode)]
    if route_kind(cfg) == "ONE-SHOT":
        args.append(F["one_shot_ok"])
    return args


def chain(lane, single, cfgs, run, solved=(), hold=None):
    """Single-solve first; timed modes only once it is CORRECT (until then they usually answer prerequisite_required,
    and a one-shot must not be spent on a solver that just failed). Then all timed modes at once, except modes the
    lane lists as `sequential` (each waits for the previous CORRECT). A mode that still answers
    prerequisite_required after an earlier mode is CORRECT is run once more. A timed-by-default / one-shot mode that
    is not CORRECT gets an untimed practice run at once (CORRECT there counts for gating; otherwise its run dir is the
    open attempt for the grouped oracle). `solved`: modes already CORRECT; `hold`: mode -> reason not to start it."""
    hold = hold or {}

    def go(mode, args):
        if mode in solved:
            return {"mode": mode, "result": "CORRECT", "note": "already CORRECT"}
        if mode in hold:
            return {"mode": mode, "result": f"HELD ({hold[mode]})"}
        return dict(run(args), mode=mode)

    def tm(mode):
        cfg = cfgs[f"{single}/{mode}"]
        r = dict(go(mode, mode_args(single, mode, cfg)), kind=route_kind(cfg))
        if r["kind"] != "dual-pool" and r["result"] != "CORRECT":
            p = run([single, C.mode_flag(mode), F["practice"]])
            r["practice"] = p
            if p.get("result") == "CORRECT":
                r.update(timed_result=r["result"], result="CORRECT")
        return r

    gated = lambda r: "prerequisite_required" in " ".join(r.get("errors", []) + r.get("tail", []))
    steps = [go(SINGLE, [single])]
    modes = [m for m in TIMED if f"{single}/{m}" in cfgs]
    if steps[0]["result"] != "CORRECT":
        return steps + [{"mode": m, "kind": route_kind(cfgs[f"{single}/{m}"]),
                         "result": "WAITING (single-solve not CORRECT - fix it, then run this mode)"} for m in modes]
    seq = [m for m in LANES[lane].get("sequential", []) if m in modes]
    par = [m for m in modes if m not in seq]
    with ThreadPoolExecutor(max(1, len(par))) as pool:
        ran = [f.result() for f in [pool.submit(tm, m) for m in par]]
    prev_ok = True
    for m in seq:
        if prev_ok:
            r = tm(m)
            prev_ok = r["result"] == "CORRECT"
        else:
            r = {"mode": m, "kind": route_kind(cfgs[f"{single}/{m}"]),
                 "result": "WAITING (the previous sequential mode is not CORRECT)"}
        ran.append(r)
    if any(r["result"] == "CORRECT" for r in ran):
        ran = [tm(r["mode"]) if gated(r) else r for r in ran]
    order = {m: i for i, m in enumerate(TIMED)}
    return steps + sorted(ran, key=lambda r: order.get(r["mode"], 99))


def first_pass(day, lane, rows, routes_cfg):
    mine = sorted(c["id"] for c in rows if C.owns_day_route(lane, day, c["id"]))
    single = next((r for r in mine if "/" not in r), None)
    report = {"lane": lane, "routes": mine, "steps": []}
    if not single:
        report["note"] = "no single-solve route for this lane"
        return report
    out = C.runs_dir(day)

    def run(args):  # one mode's crash (e.g. a timeout) must not discard the others' results
        try:
            return solver_run(args, out)
        except Exception as e:  # noqa: BLE001
            return {"result": f"CRASHED {e!r}", "dir": None, "errors": [], "tail": []}

    report["steps"] = chain(lane, single, routes_cfg, run)

    def oracle(s):
        r = subprocess.run(["python3", "-I", os.path.join(C.repo, "tools", "oracle_groups.py"), s["dir"]],
                           cwd=C.solver_dir, text=True, capture_output=True, env=C.solver_env(), timeout=1800)
        s["oracle"] = r.stdout.strip().splitlines()[-40:]

    # Partial submits only mean something on an open untimed attempt: single-solve, dual-pool regular play, and the
    # practice continuation of a failed timed / one-shot attempt.
    wrong = [s for s in report["steps"] if s.get("result") == "incorrect" and s.get("dir")
             and s.get("kind") in (None, "dual-pool")]
    wrong += [s["practice"] for s in report["steps"]
              if (s.get("practice") or {}).get("result") == "incorrect" and s["practice"].get("dir")]
    if wrong:
        with ThreadPoolExecutor(len(wrong)) as pool:
            list(pool.map(oracle, wrong))
    return report


def brief(day, report):
    lane = report["lane"]
    done = all(s.get("result") == "CORRECT" for s in report["steps"]) and len(report["steps"]) > 1
    sd = C.solver_dir
    lines = [f"# Day {day} - {lane} lane: first-pass brief", "", f"Routes: {', '.join(report['routes']) or '(none)'}", ""]
    for s in report["steps"]:
        kind = f" ({s['kind']})" if s.get("kind") else ""
        via = f" via practice (timed attempt: {s['timed_result']})" if s.get("timed_result") else ""
        lines.append(f"## {s.get('mode')}{kind}: {s.get('result')}{via}")
        if s.get("dir"):
            lines.append(f"run dir: `{os.path.join(sd, s['dir'])}`")
        if s.get("errors"):
            lines += ["errors:", "```", *s["errors"], "```"]
        if s.get("oracle"):
            lines += ["grouped oracle (untimed partial submits):", "```", *s["oracle"], "```"]
        if s.get("result") != "CORRECT" and s.get("tail") and not s.get("errors"):
            lines += ["tail:", "```", *s["tail"], "```"]
        p = s.get("practice")
        if p:
            lines.append(f"practice (untimed continuation of that attempt): {p.get('result')}"
                         + (f" - run dir `{os.path.join(sd, p['dir'])}`" if p.get("dir") else ""))
            if p.get("errors"):
                lines += ["```", *p["errors"], "```"]
            if p.get("oracle"):
                lines += ["grouped oracle on the practice attempt:", "```", *p["oracle"], "```"]
            if p.get("result") != "CORRECT" and p.get("tail") and not p.get("errors") and not p.get("oracle"):
                lines += ["```", *p["tail"], "```"]
        lines.append("")
    tpl = open(os.path.join(HERE, "prompts", "task-done.md" if done else "task.md")).read()
    lines.append(tpl.format(**placeholders(day, lane)))
    path = os.path.join(state_dir(day), f"brief-{lane}.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    with open(os.path.join(state_dir(day), f"firstpass-{lane}.json"), "w") as f:
        json.dump(report, f, indent=1)
    return path, done


def watch(day, rehearse):
    rel = Release(day, rehearse).start()
    wait = 45 * 60 + (0 if rehearse else max(0, (release_at(-90) - datetime.now(TZ)).total_seconds()))
    if not rel.released.wait(wait):
        sys.exit("no release within 45 minutes")
    sd = state_dir(day)

    def lane_job(lane):
        t = time.time()
        try:
            rows, how = lane_rows(day, lane, rel)
            merge_routes(rows)
            log(f"{lane}: {len(rows)} routes via {how} in {time.time() - t:.1f} s")
            rep = first_pass(day, lane, rows, {c["id"]: c for c in rows})
        except Exception as e:  # noqa: BLE001
            rep = {"lane": lane, "routes": [], "steps": [{"mode": "first pass", "result": f"CRASHED {e!r}"}]}
        path, done = brief(day, rep)
        log(f"{lane}: first pass {'COMPLETE' if done else 'needs work'} in {time.time() - t:.0f} s -> {path}")
        if NO_AGENTS:
            return
        try:
            open(os.path.join(sd, f"sent-{lane}"), "w").close()  # stall clock: stops before this are the prime turn
            send(lane, f"Day {day} is live. Read {path} and follow it.")
        except Exception as e:  # noqa: BLE001
            log(f"{lane}: could not reach the agent pane ({e!r}) - brief is at {path}")

    threads = [threading.Thread(target=lane_job, args=(l,)) for l in LANES]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    log("first pass finished for every lane; agents are working - `rush.py status` to follow")
    if not NO_AGENTS:
        monitor(day)


# ---------------------------------------------------------------- stalls / takeover


def mtime(path):
    try:
        return os.path.getmtime(path)
    except OSError:
        return None


def stall_since(sd, lane, now=None):
    """Seconds since the agent stopped without finishing, or None: its last Stop came after the brief was sent, no
    tool call / prompt since, and no done marker."""
    now = now or time.time()
    sent, stop, act = (mtime(os.path.join(sd, f"{k}-{lane}")) for k in ("sent", "stopped", "active"))
    if os.path.exists(os.path.join(sd, f"done-{lane}")) or sent is None or stop is None or stop < sent:
        return None
    if act is not None and act > stop:
        return None
    return now - stop


def monitor(day, every_s=10):
    """Escalate stalled agents to the coordinator (stalled-<lane>.md + log). Never acts on a worktree itself: a false
    positive must not race a live agent."""
    sd, end, flagged = state_dir(day), time.time() + A["monitor_minutes"] * 60, {}
    while time.time() < end:
        pending = [l for l in LANES if not os.path.exists(os.path.join(sd, f"done-{l}"))]
        if not pending:
            log("every lane agent is done")
            return
        for lane in pending:
            idle = stall_since(sd, lane)
            stop = mtime(os.path.join(sd, f"stopped-{lane}"))
            if idle is not None and idle >= A["stall_grace_s"] and flagged.get(lane) != stop:
                flagged[lane] = stop
                pane = sh(TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{lane}", "-S", "-60"], check=False).stdout
                path = os.path.join(sd, f"stalled-{lane}.md")
                with open(path, "w") as f:
                    f.write(f"# {lane}: agent stopped {idle:.0f} s ago without done-{lane}\n\n"
                            f"Take over: `python3 tools/rush/rush.py takeover --day {day} --lanes {lane}` "
                            f"(or message the agent: {' '.join(TMUX)} send-keys -t {SESSION}:{lane} ...)\n\n"
                            "Pane tail:\n```\n" + "\n".join(pane.rstrip().splitlines()[-40:]) + "\n```\n")
                log(f"{lane}: STALLED (stopped {idle:.0f} s ago, no done marker) -> {path}")
        time.sleep(every_s)
    log("monitor window over")


def slugify(title):
    return re.sub(r"[^a-z0-9]+", "-", (title or "").lower()).strip("-")


def verdict(run_dir):
    """The graded result of a saved run: result.json is a list of submit records ({label, ms, response}) - the last
    one is the verdict - or (older layouts) one object."""
    res = json.load(open(os.path.join(run_dir, "result.json")))
    if isinstance(res, list):
        res = (res[-1].get("response") or {}) if res else {}
    return res.get("result")


def latest_correct(run_root, mode):
    """Newest graded-CORRECT run dir of one mode under <runs>/<single id>/ (first pass, agent or takeover)."""
    best = None
    for d in os.listdir(run_root) if os.path.isdir(run_root) else []:
        if not d.startswith(mode + "-"):
            continue
        try:
            ok = verdict(os.path.join(run_root, d)) == "CORRECT"
        except (OSError, ValueError, AttributeError):
            continue
        if ok and (best is None or d.rsplit("-", 1)[-1] > best.rsplit("-", 1)[-1]):
            best = d
    return best and os.path.join(run_root, best)


def takeover(day, lane):
    """Coordinator tool for a stalled lane, run against the lane worktree's build: every mode not CORRECT yet,
    chained like the first pass - except that a timed-by-default / one-shot route that already has an attempt gets no
    new timed start, only the untimed practice continuation. Then fixtures for modes without one, and the lane replay.
    Reports only: the coordinator reviews, commits on the lane branch and touches done."""
    wt = lane_solver_dir(lane)
    out = C.runs_dir(day)
    lines = [f"# takeover: day {day} {lane} ({wt})", ""]
    b = sh(C["solver"]["build"], cwd=wt, check=False)
    if b.returncode:
        lines += ["build FAILED:", "```", *b.stderr.strip().splitlines()[-25:], "```"]
        return finish_takeover(day, lane, lines)
    cache = json.load(open(C.route_cache())).get("challenges", [])
    rows = {c["id"]: c for c in cache if C.owns_day_route(lane, day, c["id"])}
    single = next((r for r in sorted(rows) if "/" not in r), None)
    if not single:
        lines.append(f"no day-{day} routes for {lane} in the route cache")
        return finish_takeover(day, lane, lines)
    modes = [(SINGLE, single)] + [(m, f"{single}/{m}") for m in TIMED if f"{single}/{m}" in rows]
    solved, hold = set(), {}
    for mode, rid in modes:
        try:
            st = C.api_get("status", timeout=120, id=rid)
        except urllib.error.HTTPError as e:  # 409 start_required / prerequisite_required: no attempt yet
            if e.code != 409:
                raise
            st = {}
        if st.get("result") == "CORRECT":
            solved.add(mode)
        elif mode != SINGLE and route_kind(rows[rid]) != "dual-pool" and st.get("attempt_id"):
            hold[mode] = f"{route_kind(rows[rid])} route already has an attempt (result {st.get('result')}): no new timed start"

    for r in chain(lane, single, rows, lambda args: solver_run(args, out, wt), solved, hold):
        lines.append(f"- {r['mode']}" + (f" ({r['kind']})" if r.get("kind") else "") + f": {r['result']}"
                     + (f" ({r['dir']})" if r.get("dir") else ""))
        lines += [f"    {e}" for e in r.get("errors", [])[:10]]
        if r.get("practice"):
            pr = r["practice"]
            lines.append(f"    practice: {pr.get('result')}" + (f" ({pr['dir']})" if pr.get("dir") else "")
                         + ("" if pr.get("result") == "CORRECT" else f" - grouped oracle: python3 -I tools/oracle_groups.py {pr.get('dir')}"))
    slug = slugify(rows[single].get("title") or rows[single].get("name")) if LANES[lane].get("slug") == "title" else None
    run_root = os.path.join(out, single)
    for mode, _ in modes:
        if os.path.exists(os.path.join(C.fixtures_dir(lane), f"d{day:02d}-{mode}.json")):
            continue
        d = latest_correct(run_root, mode)
        if not d:
            lines.append(f"- fixture d{day:02d}-{mode}: no CORRECT run yet")
            continue
        args = ["python3", "-I", os.path.join(C.repo, "tools", "fixtures_from_runs.py"), d] + (["--slug", slug] if slug else [])
        r = sh(args, cwd=C.solver_dir, check=False)
        lines.append(f"- fixture d{day:02d}-{mode}: " + ((r.stdout or r.stderr).strip().splitlines() or ["?"])[-1])
    rp = sh([C.solver_bin(wt), "replay", C.fixtures_dir(lane, wt)], cwd=wt, check=False)
    lines.append(f"- replay {LANES[lane]['fixtures']}: " + ((rp.stderr or rp.stdout).strip().splitlines() or ["?"])[-1].strip())
    lines += ["", f"Next (coordinator): review `git -C {C.worktree(lane)} status`, commit on "
                  f"{C.fmt(A['branch'], day=day, lane=lane)}, `touch {os.path.join(state_dir(day), 'done-' + lane)}`, "
                  f"then tools/merge.sh {day}."]
    return finish_takeover(day, lane, lines)


def finish_takeover(day, lane, lines):
    path = os.path.join(state_dir(day), f"takeover-{lane}.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    log(f"{lane}: takeover report -> {path}")
    return path


def status(day):
    sd = state_dir(day)
    for lane in LANES:
        fp = os.path.join(sd, f"firstpass-{lane}.json")
        steps = json.load(open(fp))["steps"] if os.path.exists(fp) else []
        done = os.path.exists(os.path.join(sd, f"done-{lane}"))
        idle = stall_since(sd, lane)
        agent = "done" if done else (f"STOPPED {idle:.0f} s ago (no done marker)" if idle is not None else "working")
        if not done and os.path.exists(os.path.join(sd, f"takeover-{lane}.md")):
            agent += " | takeover report written"
        summary = " | ".join(f"{s.get('mode')}={s.get('result')}" for s in steps) or "-"
        print(f"{lane:12} first pass: {summary}   agent: {agent}")


def main():
    global NO_AGENTS, BRANCH
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    cmd = a[0]
    day = int(a[a.index("--day") + 1]) if "--day" in a else datetime.now(TZ).day
    rehearse = "--rehearse" in a
    if "--lanes" in a:
        keep = a[a.index("--lanes") + 1].split(",")
        unknown = [l for l in keep if l not in LANES]
        if unknown:
            sys.exit(f"unknown lanes {unknown} (registered: {list(LANES)})")
        for lane in list(LANES):
            if lane not in keep:
                del LANES[lane]
    NO_AGENTS = "--no-agents" in a
    BRANCH = a[a.index("--branch") + 1] if "--branch" in a else None  # resolved by prep() when needed
    if not (C["event"]["first_day"] <= day <= C["event"]["last_day"]):
        sys.exit(f"day {day} outside the event")
    log(f"rush {cmd}: day {day}{' (rehearsal)' if rehearse else ''}, lanes {', '.join(LANES)}")
    if cmd == "all":
        # A fresh day must relaunch fresh agents: a previous session would make launch() a no-op and prep resets the
        # worktrees under its agents.
        if sh(TMUX + ["kill-session", "-t", SESSION], check=False).returncode == 0:
            log("killed the previous tmux session")
    if cmd in ("prep", "all"):
        prep(day)
    if cmd in ("launch", "all"):
        launch(day)
    if cmd in ("watch", "all"):
        watch(day, rehearse)
    if cmd == "status":
        status(day)
    if cmd == "takeover":
        if "--lanes" not in a:
            sys.exit("takeover needs --lanes")
        for lane in LANES:
            takeover(day, lane)


if __name__ == "__main__":
    main()
