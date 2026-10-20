#!/usr/bin/env python3
"""Day-release orchestrator: one agent per lane (Claude Code in tmux) plus an automated first pass.

  rush.py all     [--day N] [--rehearse]   prep + launch + watch (what the systemd timer runs at 22:45 AEST)
  rush.py prep    [--day N]                reset the lane worktrees to master, build every target (warm caches)
  rush.py launch  [--day N]                tmux session `rush` (socket -L huntress): one primed Claude per lane
  rush.py watch   [--day N] [--rehearse]   wait for the release, run the first pass per lane, dispatch briefs
  rush.py status  [--day N]                first-pass results, lane done-markers, stalled agents
  rush.py takeover --lanes a[,b] [--day N]  coordinator: finish a stalled lane from its worktree (untimed only)
Options: --lanes a,b (subset, e.g. for a rehearsal), --no-agents (watch: first pass + briefs only, no tmux).

Release: 23:00 Australia/Brisbane; Day N = that date's day of the month (Day 15 = 2026-10-15). The first pass is
pure ctf-run, no model: per lane, single-solve, then - once it is CORRECT - time trial and mastery together (binary-RE:
mastery after a CORRECT TT). Dual-pool routes get an untimed regular attempt; timed-by-default ones a timed start,
one-shots with --one-shot-ok (from Day 19: solves beat HoF entries). Untimed failures get the grouped oracle. Each
lane's brief (results, errors, oracle output, instructions) is written to runs-dNN/rush/brief-<lane>.md and the lane
agent is told to read it - as soon as that lane's first pass ends, not when all lanes end.
Every ctf-run here and in the agents' panes runs with CTF_NO_RANKED=hof: ctf-run refuses only --timed, so dual-pool
Hall of Fame runs stay with the coordinator's campaign.
Release detection: warmups-day-NN status every 1 s from 22:59:50 (404 -> 200), op=list as a fallback; each lane then
fetches its own routes (search=<prefix>NN) and starts at once. After the briefs, stalled agents (Stop hook after the
brief, no tool activity since, no done marker, 180 s) are escalated to runs-dNN/rush/stalled-<lane>.md for the
coordinator, who runs `takeover`. `all` kills the previous day's tmux session before relaunching.
`--rehearse` skips the wait and replays the flow on an already-released day (solved routes only reconnect).
Attach: tmux -L huntress attach -t rush
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
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
CTF = os.path.dirname(os.path.dirname(HERE))  # .../2026/ctf-run
REPO = os.path.dirname(CTF)  # .../2026
WT_ROOT = os.path.join(os.path.dirname(REPO), "wt")  # .../huntress-ctfs/wt/<lane>
API = "https://2026.huntress.ctf.games"
AEST = timezone(timedelta(hours=10))  # Brisbane: no daylight saving
TMUX = ["tmux", "-L", "huntress"]
SESSION = "rush"
NO_AGENTS = False
LANES = {  # lane -> (challenge prefix, lane source, fixtures dir)
    "warmups": ("warmups-day-", "warmups.rs", "warmups"),
    "metadata": ("metadata-forensics-day-", "metadata.rs", "metadata"),
    "script": ("script-analysis-day-", "script_analysis.rs", "script"),
    "binary": ("binary-reverse-engineering-day-", "binary_re.rs", "binary_re"),
    "mobile": ("mobile-android-day-", "mobile.rs", "mobile"),
}


def log(msg):
    print(f"[{datetime.now(AEST):%H:%M:%S}] {msg}", flush=True)


def token():
    tok = os.environ.get("HUNTRESS_GM_TOKEN")
    if tok:
        return tok
    for line in open(os.path.join(REPO, ".env")):
        if line.startswith("HUNTRESS_GM_TOKEN="):
            return line.split("=", 1)[1].strip()
    sys.exit("no HUNTRESS_GM_TOKEN (env or .env)")


def env():
    # hof: ctf-run refuses only --timed (dual-pool Hall of Fame runs); one-shot / timed-by-default solves are allowed.
    return dict(
        os.environ, HUNTRESS_GM_TOKEN=token(), HUNTRESS_GM_URL=API, CTF_NO_RANKED="hof"
    )


def sh(args, cwd=None, check=True, **kw):
    r = subprocess.run(args, cwd=cwd, text=True, capture_output=True, **kw)
    if check and r.returncode != 0:
        raise RuntimeError(f"{' '.join(args)}: {r.stderr.strip()[-400:]}")
    return r


def state_dir(day):
    d = os.path.join(CTF, f"runs-d{day:02d}", "rush")
    os.makedirs(d, exist_ok=True)
    return d


def agent_dir(day, lane):
    """Where a lane's agent-facing files (prime, settings, launcher, brief, markers) live."""
    return state_dir(day)


# ---------------------------------------------------------------- prep


def prep(day):
    """Persistent worktrees (Claude's folder trust is accepted once), reset to master, built warm."""
    os.makedirs(WT_ROOT, exist_ok=True)
    for lane in LANES:
        wt = os.path.join(WT_ROOT, lane)
        branch = f"day{day:02d}/{lane}"
        if not os.path.isdir(wt):
            sh(
                ["git", "worktree", "add", "-f", wt, "-B", branch, "master"],
                cwd=REPO,
            )
        else:
            dirty = sh(["git", "status", "--porcelain"], cwd=wt).stdout.strip()
            if dirty:
                log(f"{lane}: worktree dirty - stashing before reset")
                sh(
                    ["git", "stash", "push", "-u", "-m", f"rush day {day} prep"],
                    cwd=wt,
                )
            sh(["git", "checkout", "-B", branch, "master"], cwd=wt)
        # Gitignored, shared with the main checkout: fixtures (manifests point at main's artifacts) and .env.
        for rel in ("ctf-run/fixtures", ".env"):
            link = os.path.join(wt, rel)
            if not os.path.lexists(link) and os.path.lexists(os.path.join(REPO, rel)):
                os.symlink(os.path.join(REPO, rel), link)
        log(f"{lane}: worktree {wt} on {branch}")
    for where in [CTF] + [os.path.join(WT_ROOT, l, "ctf-run") for l in LANES]:
        t = time.time()
        sh(["cargo", "build", "--release", "--features", "tls"], cwd=where)
        log(f"built {where} in {time.time() - t:.0f} s")


# ---------------------------------------------------------------- launch


def prime_prompt(day, lane):
    prefix, src, fix = LANES[lane]
    tpl = open(os.path.join(HERE, "prompts", "prime.md")).read()
    return tpl.format(
        day=day,
        prev=day - 1,
        lane=lane,
        prefix=prefix,
        src=src,
        fix=fix,
        wt=os.path.join(WT_ROOT, lane),
        repo=REPO,
        state=agent_dir(day, lane),
    )


def allowed_tools(lane):
    """Allow rules for a lane agent: its own worktree's ctf-run, relative and absolute. The auto-mode classifier
    denied the brief's plain untimed `-t` start on Day 16 ("Auto-Mode Bypass") and the binary agent stalled; an
    explicit rule skips the classifier for that binary only. The pane exports CTF_NO_RANKED=hof (one-shot and
    timed-by-default solves allowed, --timed HoF runs refused) - and an `env -u ...` / `VAR=... ` prefix no longer
    matches the rule."""
    return [
        "Bash(./target/release/ctf-run *)",
        f"Bash({os.path.join(WT_ROOT, lane, 'ctf-run', 'target', 'release', 'ctf-run')} *)",
    ]


def agent_settings(day, lane):
    """Hooks that let the watcher see a lane agent stop: Stop touches stopped-<lane>; every tool call and prompt
    touches active-<lane> (heartbeat). A stop after the brief, with no activity since and no done marker, is a stall."""
    sd = agent_dir(day, lane)
    touch = lambda name: {
        "type": "command",
        "command": f"touch {shlex.quote(os.path.join(sd, name))}",
    }
    return {
        "hooks": {
            "Stop": [{"hooks": [touch(f"stopped-{lane}")]}],
            "PostToolUse": [{"matcher": "*", "hooks": [touch(f"active-{lane}")]}],
            "UserPromptSubmit": [{"hooks": [touch(f"active-{lane}")]}],
        }
    }


def launcher_script(day, lane, prompt_file, settings_file):
    allow = " ".join(f'"{r}"' for r in allowed_tools(lane))
    adddir = f'--add-dir "{CTF}" "{os.path.join(REPO, "utils")}" '
    return (
        "#!/bin/sh\n"
        # --add-dir: briefs, first-pass runs, done markers and the real fixtures/artifacts live
        # in the main checkout's ctf-run/ and utils/; without it every read there stops at a permission dialog.
        # Deliberately not the repo root: that holds the jumpbox SSH key, and CTF_NO_RANKED only guards
        # this machine - workers must not be able to reach a ranked run on the jumpbox.
        # Model/effort pinned here: worktrees carry no settings file, so they would otherwise inherit
        # whatever ~/.claude/settings.json says (including per-model effort overrides).
        # --add-dir and --allowedTools are variadic: -n must follow them so the prompt stays positional.
        f"exec claude --model claude-opus-5-5 --effort high --permission-mode auto {adddir}"
        f'--allowedTools {allow} --settings "{settings_file}" -n "d{day:02d}-{lane}" "$(cat {prompt_file})"\n'
    )


def launch(day):
    have = sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0
    if have:
        log(
            "tmux session already exists - not relaunching (tmux -L huntress kill-session -t rush to reset)"
        )
        return
    sd = state_dir(day)
    e = env()
    envflags = sum(
        (
            ["-e", f"{k}={e[k]}"]
            for k in ("HUNTRESS_GM_TOKEN", "HUNTRESS_GM_URL", "CTF_NO_RANKED")
        ),
        [],
    )
    sh(
        TMUX
        + ["new-session", "-d", "-s", SESSION, "-n", "control", "-c", CTF]
        + envflags
    )
    for lane in LANES:
        ad = agent_dir(day, lane)
        prompt_file = os.path.join(ad, f"prime-{lane}.md")
        with open(prompt_file, "w") as f:
            f.write(prime_prompt(day, lane))
        settings_file = os.path.join(ad, f"settings-{lane}.json")
        with open(settings_file, "w") as f:
            json.dump(agent_settings(day, lane), f, indent=1)
        launcher = os.path.join(ad, f"launch-{lane}.sh")
        with open(launcher, "w") as f:
            f.write(launcher_script(day, lane, prompt_file, settings_file))
        os.chmod(launcher, 0o755)
        sh(
            TMUX
            + [
                "new-window",
                "-t",
                SESSION,
                "-n",
                lane,
                "-c",
                os.path.join(WT_ROOT, lane),
            ]
            + envflags
            + [launcher]
        )
        log(f"{lane}: agent launched (window rush:{lane})")


def send(lane, text):
    # Claude Code can leave a picker open after the prime turn ("Teach auto mode about your environment?" - seen in
    # the Day 17 smoke test). Typed text would answer it (the "1" in "Day 1x" picks option 1) and Enter confirms, so
    # the brief is lost. Dismiss only an open picker: Escape at a busy agent would interrupt it.
    for _ in range(3):
        pane = sh(
            TMUX + ["capture-pane", "-p", "-t", f"{SESSION}:{lane}"], check=False
        ).stdout
        if "Esc to cancel" not in "\n".join(pane.rstrip().splitlines()[-12:]):
            break
        sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "Escape"])
        time.sleep(0.5)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "-l", text])
    time.sleep(0.3)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "Enter"])


# ---------------------------------------------------------------- watch / first pass


def get(url, timeout=60):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token()}"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def at(h, m, s):
    return datetime.now(AEST).replace(hour=h, minute=m, second=s, microsecond=0)


def sleep_until(t):
    while datetime.now(AEST) < t:
        time.sleep(min(5, max(0.05, (t - datetime.now(AEST)).total_seconds())))


def warmups_released(day):
    """op=status on the one predictable route: 404 {"error":"challenge_unavailable"} before the release (~0.5 s),
    200 after (seen only on routes we had started). Only challenge_unavailable means "not yet": any other 404 body
    (e.g. a never-started route answering "no attempt") counts as released, and so does 409 {"error":"start_required"}
    (Day 17: a released, never-started route; treating it as a failure cost 9 s); 401/403/429/5xx raise (keep polling)."""
    try:
        get(f"{API}/?op=status&challenge=warmups-day-{day:02d}", timeout=5)
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
        return err != "challenge_unavailable"


class Release:
    """Two detectors: warmups status every 1 s from 22:59:50 (3 s after 23:01, in case the release is late) and the
    old op=list poll every 3 s from 22:58:30, which keeps running until it has the day's rows - the fallback for a
    lane whose own search finds nothing (and for a renamed warmups route). A released day's op=list&day=N took
    4-10 s on Day 16 (that was the 11 s detection), so lanes fetch their own rows instead (lane_rows)."""

    def __init__(self, day, rehearse):
        self.day, self.rehearse = day, rehearse
        self.released, self.listed, self.rows = threading.Event(), threading.Event(), []

    def start(self):
        for fn in (self._status, self._list):
            threading.Thread(target=fn, daemon=True).start()
        return self

    def _status(self):
        if not self.rehearse:
            sleep_until(at(22, 59, 50))
        while not self.released.is_set():
            t = time.time()
            try:
                if warmups_released(self.day):
                    log(
                        f"day {self.day} released (warmups-day-{self.day:02d} status 200)"
                    )
                    self.released.set()
                    return
            except Exception as e:  # noqa: BLE001
                log(f"status probe failed: {e!r}")
            every = 1 if self.rehearse or datetime.now(AEST) < at(23, 1, 0) else 3
            time.sleep(
                max(0.05, every - (time.time() - t))
            )  # a steady 1 s rhythm: a 404 itself takes ~0.5 s

    def _list(self):
        if not self.rehearse:
            sleep_until(at(22, 58, 30))
        while True:
            try:
                rows = get(f"{API}/?op=list&day={self.day}&all=true").get(
                    "challenges", []
                )
                rows = [c for c in rows if c.get("day") == self.day]
                if rows:
                    if not self.released.is_set():
                        log(f"day {self.day} released (op=list: {len(rows)} routes)")
                    self.rows = rows
                    self.listed.set()
                    self.released.set()
                    return
            except Exception as e:  # noqa: BLE001
                log(f"list failed: {e!r}")
            time.sleep(3)


def lane_rows(day, lane, rel, tries_s=20):
    """The lane's own routes right after the release: op=list&day=N&search=<prefix>NN (~1.4 s for 3 routes; the
    lane= filter rejects binary-RE). Retried for tries_s (status can go 200 before every lane is listable), then the
    global list. Rows are filtered by id prefix: search matches substrings."""
    prefix = f"{LANES[lane][0]}{day:02d}"
    mine = lambda rows: [
        c
        for c in rows
        if c.get("day") == day
        and (
            c["id"] == prefix
            or c["id"].startswith(prefix + "-")
            or c["id"].startswith(prefix + "/")
        )
    ]
    end = time.time() + tries_s
    while time.time() < end:
        try:
            rows = mine(
                get(f"{API}/?op=list&day={day}&search={prefix}", timeout=15).get(
                    "challenges", []
                )
            )
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
    """Merge routes into ctf-run's cache (a full op=list refresh takes ~90 s). Lanes merge concurrently: the lock
    spans read-modify-write, or one lane's write drops another's rows."""
    with _CACHE_LOCK:
        _merge_routes(rows)


def _merge_routes(rows):
    path = os.path.expanduser("~/.cache/ctf-run/routes.json")
    try:
        cache = json.load(open(path))
    except (OSError, ValueError):
        cache = {"challenges": []}
    ids = {c["id"] for c in rows}
    cache["challenges"] = [
        c for c in cache.get("challenges", []) if c.get("id") not in ids
    ] + rows
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(cache, f)
    os.replace(tmp, path)  # atomic: agents may read it concurrently


def ctf_run(args, out_dir, where=CTF):
    """`where`: the ctf-run checkout whose binary runs (the main one, or a lane worktree for takeover)."""
    r = subprocess.run(
        [os.path.join(where, "target", "release", "ctf-run")] + args + ["-o", out_dir],
        cwd=where,
        text=True,
        capture_output=True,
        env=env(),
        timeout=900,
    )
    res = re.search(
        r"submit +(CORRECT|incorrect|accepted|REFUSED|already_solved)", r.stderr
    )
    saved = re.search(r"\] saved +(\S+)", r.stderr)
    errors = [
        l for l in r.stderr.splitlines() if "] error " in l or l.startswith("error")
    ]
    result = res.group(1) if res else ("ERROR" if r.returncode else "?")
    if (
        result == "already_solved"
    ):  # rehearsals on a released day: the attempt was solved before
        result = "CORRECT"
    return {
        "result": result,
        "dir": saved and saved.group(1),
        "errors": errors[:40],
        "tail": r.stderr.strip().splitlines()[-8:],
    }


# binary-RE's mastery answers 409 prerequisite_required until its time trial is CORRECT (Day 17).
SEQUENTIAL_MASTERY = {"binary"}
ORDER = {"single_solve": 0, "time_trial": 1, "mastery": 2}


def route_kind(cfg):
    if cfg.get("run_for_time_supported"):
        return "dual-pool"
    return "ONE-SHOT" if cfg.get("ranked_pool_size") == 1 else "timed by default"


def mode_args(single, mode, cfg):
    """ctf-run args for a TT/mastery route. Dual-pool: plain start = untimed regular attempt. Timed by default: plain
    start is timed (allowed under CTF_NO_RANKED=hof). One-shot (ranked pool 1, the route's only attempt): --one-shot-ok."""
    args = [single, "-t" if mode == "time_trial" else "-m"]
    if route_kind(cfg) == "ONE-SHOT":
        args.append("--one-shot-ok")
    return args


def chain(lane, single, cfgs, run, solved=(), hold=None):
    """Single-solve first; TT and mastery only once it is CORRECT (until then they answer 409 prerequisite_required,
    and a one-shot must not be spent on a solver that just failed), then both at once - except SEQUENTIAL_MASTERY
    lanes, whose mastery waits for a CORRECT TT. A mastery that still answers prerequisite_required (a lane newly
    gated on its TT; the refused start spends nothing) is run once more after a CORRECT TT. A timed-by-default or
    one-shot mode that is not CORRECT (failed, expired, or HELD) gets an untimed `--practice` run at once: the GM
    reopens the last artifact without a clock once fresh attempts are used up. A CORRECT there (e.g. the timed run
    only missed its deadline) counts for gating; otherwise its run dir is the open attempt for the grouped oracle.
    `run(args)` -> ctf_run result; modes in `solved` are already CORRECT and only gate the others; `hold` maps a mode
    to the reason it must not be started (it then counts as not CORRECT)."""
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
            p = run([single, "-t" if mode == "time_trial" else "-m", "--practice"])
            r["practice"] = p
            if p.get("result") == "CORRECT":
                r.update(timed_result=r["result"], result="CORRECT")
        return r

    def gated(r):
        return "prerequisite_required" in " ".join(
            r.get("errors", []) + r.get("tail", [])
        )

    steps = [go("single_solve", [single])]
    modes = [m for m in ("time_trial", "mastery") if f"{single}/{m}" in cfgs]
    if steps[0]["result"] != "CORRECT":
        return steps + [
            {
                "mode": m,
                "kind": route_kind(cfgs[f"{single}/{m}"]),
                "result": "WAITING (single-solve not CORRECT - fix it, then run this mode)",
            }
            for m in modes
        ]
    if lane in SEQUENTIAL_MASTERY and len(modes) == 2:
        t = tm("time_trial")
        m = (
            tm("mastery")
            if t["result"] == "CORRECT"
            else {
                "mode": "mastery",
                "kind": route_kind(cfgs[f"{single}/mastery"]),
                "result": "WAITING (time trial not CORRECT - this lane's mastery needs it first)",
            }
        )
        return steps + [t, m]
    with ThreadPoolExecutor(max(1, len(modes))) as pool:
        ran = [f.result() for f in [pool.submit(tm, m) for m in modes]]
    t = next((r for r in ran if r["mode"] == "time_trial"), None)
    for i, r in enumerate(ran):
        if r["mode"] == "mastery" and gated(r) and t and t["result"] == "CORRECT":
            ran[i] = tm("mastery")
    return steps + ran


def first_pass(day, lane, rows, routes_cfg):
    prefix = LANES[lane][0]
    out = f"runs-d{day:02d}"
    mine = sorted(c["id"] for c in rows if c["id"].startswith(prefix))
    single = next((r for r in mine if "/" not in r), None)
    report = {"lane": lane, "routes": mine, "steps": []}
    if not single:
        report["note"] = "no single-solve route for this lane"
        return report

    def run(
        args,
    ):  # one mode's crash (e.g. a 900 s timeout) must not discard the others' results
        try:
            return ctf_run(args, out)
        except Exception as e:  # noqa: BLE001
            return {"result": f"CRASHED {e!r}", "dir": None, "errors": [], "tail": []}

    # Fast solves beat Hall of Fame entries (user, Day 18): one-shot and timed-by-default routes run here too,
    # chained on a CORRECT single-solve. Only --timed (dual-pool HoF runs) stays with the coordinator.
    report["steps"] = chain(lane, single, routes_cfg, run)

    def oracle(s):
        r = subprocess.run(
            ["python3", "-I", os.path.join(CTF, "tools", "oracle_groups.py"), s["dir"]],
            cwd=CTF,
            text=True,
            capture_output=True,
            env=env(),
            timeout=1800,
        )
        s["oracle"] = r.stdout.strip().splitlines()[-40:]

    # Partial submits only mean something on an open untimed attempt: single-solve, dual-pool regular play, and the
    # practice continuation of a failed timed / one-shot attempt.
    wrong = [
        s
        for s in report["steps"]
        if s.get("result") == "incorrect"
        and s.get("dir")
        and s.get("kind") in (None, "dual-pool")
    ]
    wrong += [
        s["practice"]
        for s in report["steps"]
        if (s.get("practice") or {}).get("result") == "incorrect"
        and s["practice"].get("dir")
    ]
    if wrong:
        with ThreadPoolExecutor(len(wrong)) as pool:
            list(pool.map(oracle, wrong))
    return report


def brief(day, report):
    lane = report["lane"]
    done = all(s.get("result") == "CORRECT" for s in report["steps"]) and any(
        s.get("mode") == "mastery" for s in report["steps"]
    )
    lines = [
        f"# Day {day} - {lane} lane: first-pass brief",
        "",
        f"Routes: {', '.join(report['routes']) or '(none)'}",
        "",
    ]
    for s in report["steps"]:
        kind = f" ({s['kind']})" if s.get("kind") else ""
        via = (
            f" via practice (timed attempt: {s['timed_result']})"
            if s.get("timed_result")
            else ""
        )
        lines.append(f"## {s.get('mode')}{kind}: {s.get('result')}{via}")
        if s.get("dir"):
            lines.append(f"run dir: `{os.path.join(CTF, s['dir'])}`")
        if s.get("errors"):
            lines += ["errors:", "```", *s["errors"], "```"]
        if s.get("oracle"):
            lines += [
                "grouped oracle (untimed partial submits):",
                "```",
                *s["oracle"],
                "```",
            ]
        if (
            s.get("result") not in ("CORRECT",)
            and s.get("tail")
            and not s.get("errors")
        ):
            lines += ["tail:", "```", *s["tail"], "```"]
        p = s.get("practice")
        if p:
            lines.append(
                f"practice (untimed continuation of that attempt): {p.get('result')}"
                + (
                    f" - run dir `{os.path.join(CTF, p['dir'])}`"
                    if p.get("dir")
                    else ""
                )
            )
            if p.get("errors"):
                lines += ["```", *p["errors"], "```"]
            if p.get("oracle"):
                lines += [
                    "grouped oracle on the practice attempt:",
                    "```",
                    *p["oracle"],
                    "```",
                ]
            if (
                p.get("result") != "CORRECT"
                and p.get("tail")
                and not p.get("errors")
                and not p.get("oracle")
            ):
                lines += ["```", *p["tail"], "```"]
        lines.append("")
    tpl = open(
        os.path.join(HERE, "prompts", "task-done.md" if done else "task.md")
    ).read()
    lines.append(
        tpl.format(
            day=day,
            lane=lane,
            wt=os.path.join(WT_ROOT, lane),
            repo=REPO,
            state=state_dir(day),
            fix=LANES[lane][2],
            src=LANES[lane][1],
        )
    )
    path = os.path.join(state_dir(day), f"brief-{lane}.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    with open(os.path.join(state_dir(day), f"firstpass-{lane}.json"), "w") as f:
        json.dump(report, f, indent=1)
    return path, done


def watch(day, rehearse):
    rel = Release(day, rehearse).start()
    if not rel.released.wait(
        45 * 60
        + (
            0
            if rehearse
            else max(0, (at(22, 58, 30) - datetime.now(AEST)).total_seconds())
        )
    ):
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
            rep = {
                "lane": lane,
                "routes": [],
                "steps": [{"mode": "first pass", "result": f"CRASHED {e!r}"}],
            }
        path, done = brief(day, rep)
        log(
            f"{lane}: first pass {'COMPLETE' if done else 'needs work'} in {time.time() - t:.0f} s -> {path}"
        )
        if NO_AGENTS:
            return
        try:
            open(
                os.path.join(sd, f"sent-{lane}"), "w"
            ).close()  # stall clock: stops before this are the prime turn
            send(lane, f"Day {day} is live. Read {path} and follow it.")
        except Exception as e:  # noqa: BLE001
            log(f"{lane}: could not reach the agent pane ({e!r}) - brief is at {path}")

    threads = [threading.Thread(target=lane_job, args=(l,)) for l in LANES]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    log(
        "first pass finished for every lane; agents are working - `rush.py status` to follow"
    )
    if not NO_AGENTS:
        monitor(day)


# ---------------------------------------------------------------- stalls / takeover

STALL_GRACE_S = 180  # an agent waiting on a background build/oracle also ends its turn with no tool activity


def mtime(path):
    try:
        return os.path.getmtime(path)
    except OSError:
        return None


def stall_since(sd, lane, now=None):
    """Seconds since the lane agent stopped without finishing, or None. Stalled = its last Stop came after the
    brief was sent, no tool call / prompt since (active-<lane>), and no done marker."""
    now = now or time.time()
    sent, stop, act = (
        mtime(os.path.join(sd, f"{k}-{lane}")) for k in ("sent", "stopped", "active")
    )
    if (
        os.path.exists(os.path.join(sd, f"done-{lane}"))
        or sent is None
        or stop is None
        or stop < sent
    ):
        return None
    if act is not None and act > stop:
        return None
    return now - stop


def monitor(day, until_s=90 * 60, every_s=10):
    """Escalate stalled lane agents to the coordinator (stalled-<lane>.md + log); never acts on the worktree itself:
    a false positive must not race a live agent. The coordinator runs `rush.py takeover` (or nudges the agent)."""
    sd, end, flagged = state_dir(day), time.time() + until_s, {}
    while time.time() < end:
        pending = [
            l
            for l in LANES
            if not os.path.exists(os.path.join(sd, f"done-{l}"))
        ]
        if not pending:
            log("every solver lane agent is done")
            return
        for lane in pending:
            idle = stall_since(sd, lane)
            stop = mtime(os.path.join(sd, f"stopped-{lane}"))
            if idle is not None and idle >= STALL_GRACE_S and flagged.get(lane) != stop:
                flagged[lane] = stop
                pane = sh(
                    TMUX
                    + ["capture-pane", "-p", "-t", f"{SESSION}:{lane}", "-S", "-60"],
                    check=False,
                ).stdout
                path = os.path.join(sd, f"stalled-{lane}.md")
                with open(path, "w") as f:
                    f.write(
                        f"# {lane}: agent stopped {idle:.0f} s ago without done-{lane}\n\n"
                        f"Take over: `python3 tools/rush/rush.py takeover --day {day} --lanes {lane}` "
                        f"(or message the agent: tmux -L huntress send-keys -t rush:{lane} ...)\n\n"
                        "Pane tail:\n```\n"
                        + "\n".join(pane.rstrip().splitlines()[-40:])
                        + "\n```\n"
                    )
                log(
                    f"{lane}: STALLED (stopped {idle:.0f} s ago, no done marker) -> {path}"
                )
        time.sleep(every_s)
    log("monitor window over")


def slugify(title):
    return re.sub(r"[^a-z0-9]+", "-", (title or "").lower()).strip("-")


def latest_correct(run_root, mode):
    """Newest graded-CORRECT run dir of one mode under runs-dNN/<single id>/ (any source: first pass, agent, takeover)."""
    best = None
    for d in os.listdir(run_root) if os.path.isdir(run_root) else []:
        if not d.startswith(mode + "-"):
            continue
        try:
            ok = (
                json.load(open(os.path.join(run_root, d, "result.json"))).get("result")
                == "CORRECT"
            )
        except (OSError, ValueError):
            continue
        if ok and (best is None or d.rsplit("-", 1)[-1] > best.rsplit("-", 1)[-1]):
            best = d
    return best and os.path.join(run_root, best)


def takeover(day, lane):
    """Coordinator tool for a stalled lane, run against the lane worktree's build: every mode whose route is not
    CORRECT yet, chained like the first pass (`chain`) - except that a timed-by-default / one-shot route that already
    has an attempt gets no new timed start (a retry spends another attempt; a one-shot has none left), only the untimed
    practice continuation. Then fixtures
    for modes without one and the lane replay. Reports only - the coordinator reviews, commits on the lane branch and
    touches done."""
    wt, fix = os.path.join(WT_ROOT, lane, "ctf-run"), LANES[lane][2]
    out = os.path.join(CTF, f"runs-d{day:02d}")
    lines = [f"# takeover: day {day} {lane} ({wt})", ""]
    b = sh(["cargo", "build", "--release", "--features", "tls"], cwd=wt, check=False)
    if b.returncode:
        lines += ["build FAILED:", "```", *b.stderr.strip().splitlines()[-25:], "```"]
        return finish_takeover(day, lane, lines)
    prefix = f"{LANES[lane][0]}{day:02d}"
    cache = json.load(open(os.path.expanduser("~/.cache/ctf-run/routes.json"))).get(
        "challenges", []
    )
    rows = {
        c["id"]: c
        for c in cache
        if c["id"] == prefix or c["id"].startswith((prefix + "-", prefix + "/"))
    }
    single = next((r for r in sorted(rows) if "/" not in r), None)
    if not single:
        lines.append(f"no day-{day} routes for {lane} in the route cache")
        return finish_takeover(day, lane, lines)
    modes = [("single_solve", single, None)]
    for mode in ("time_trial", "mastery"):
        if f"{single}/{mode}" in rows:
            modes.append((mode, f"{single}/{mode}", None))
    solved, hold = set(), {}
    for mode, rid, _ in modes:
        try:
            st = get(
                f"{API}/?op=status&challenge={urllib.parse.quote(rid)}", timeout=120
            )
        except (
            urllib.error.HTTPError
        ) as e:  # 409 start_required / prerequisite_required: no attempt yet
            if e.code != 409:
                raise
            st = {}
        if st.get("result") == "CORRECT":
            solved.add(mode)
        elif (
            mode != "single_solve"
            and route_kind(rows[rid]) != "dual-pool"
            and st.get("attempt_id")
        ):
            hold[mode] = (
                f"{route_kind(rows[rid])} route already has an attempt (result {st.get('result')}): no new timed start"
            )

    def run(args):
        return ctf_run(args, out, wt)

    for r in chain(lane, single, rows, run, solved, hold):
        lines.append(
            f"- {r['mode']}"
            + (f" ({r['kind']})" if r.get("kind") else "")
            + f": {r['result']}"
            + (f" ({r['dir']})" if r.get("dir") else "")
        )
        lines += [f"    {e}" for e in r.get("errors", [])[:10]]
        if r.get("practice"):
            pr = r["practice"]
            lines.append(
                f"    practice: {pr.get('result')}"
                + (f" ({pr['dir']})" if pr.get("dir") else "")
                + (
                    ""
                    if pr.get("result") == "CORRECT"
                    else f" - grouped oracle: python3 -I tools/oracle_groups.py {pr.get('dir')}"
                )
            )
    slug = (
        slugify(rows[single].get("title") or rows[single].get("name"))
        if lane == "warmups"
        else None
    )
    run_root = os.path.join(out, single)
    for mode, _, _ in modes:
        if os.path.exists(
            os.path.join(CTF, "fixtures", fix, f"d{day:02d}-{mode}.json")
        ):
            continue
        d = latest_correct(run_root, mode)
        if not d:
            lines.append(f"- fixture d{day:02d}-{mode}: no CORRECT run yet")
            continue
        args = [
            "python3",
            "-I",
            os.path.join(CTF, "tools", "fixtures_from_runs.py"),
            d,
        ] + (["--slug", slug] if slug else [])
        r = sh(args, cwd=CTF, check=False)
        lines.append(
            f"- fixture d{day:02d}-{mode}: "
            + ((r.stdout or r.stderr).strip().splitlines() or ["?"])[-1]
        )
    rp = sh(
        [os.path.join(wt, "target", "release", "ctf-run"), "replay", f"fixtures/{fix}"],
        cwd=wt,
        check=False,
    )
    lines.append(
        f"- replay fixtures/{fix}: "
        + ((rp.stderr or rp.stdout).strip().splitlines() or ["?"])[-1].strip()
    )
    lines += [
        "",
        f"Next (coordinator): review `git -C {os.path.dirname(wt)} status`, commit on day{day:02d}/{lane}, "
        f"`touch {os.path.join(state_dir(day), 'done-' + lane)}`, then tools/rush/merge.sh {day}.",
    ]
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
        agent = (
            "done"
            if done
            else (
                f"STOPPED {idle:.0f} s ago (no done marker)"
                if idle is not None
                else "working"
            )
        )
        if not done and os.path.exists(os.path.join(sd, f"takeover-{lane}.md")):
            agent += " | takeover report written"
        summary = (
            " | ".join("{}={}".format(s.get("mode"), s.get("result")) for s in steps)
            or "-"
        )
        print(f"{lane:9} first pass: {summary}   agent: {agent}")


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    cmd = a[0]
    day = int(a[a.index("--day") + 1]) if "--day" in a else datetime.now(AEST).day
    rehearse = "--rehearse" in a
    if "--lanes" in a:
        keep = a[a.index("--lanes") + 1].split(",")
        for lane in list(LANES):
            if lane not in keep:
                del LANES[lane]
    global NO_AGENTS
    NO_AGENTS = "--no-agents" in a
    if not (1 <= day <= 31):
        sys.exit(f"day {day} outside the event")
    log(f"rush {cmd}: day {day}{' (rehearsal)' if rehearse else ''}")
    if cmd == "all" and not rehearse:
        # The 22:45 run must relaunch fresh agents (new flags/hooks, new day): yesterday's session would otherwise
        # make launch() a no-op, and prep resets the worktrees under its agents.
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
