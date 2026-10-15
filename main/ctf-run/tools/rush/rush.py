#!/usr/bin/env python3
"""Day-release orchestrator: five lane agents (Claude Code in tmux) plus an automated first pass.

  rush.py all     [--day N] [--rehearse]   prep + launch + watch (what the systemd timer runs at 22:45 AEST)
  rush.py prep    [--day N]                reset the lane worktrees to master, build every target (warm caches)
  rush.py launch  [--day N]                tmux session `rush` (socket -L huntress): one primed Claude per lane
  rush.py watch   [--day N] [--rehearse]   wait for the release, run the first pass per lane, dispatch briefs
  rush.py status  [--day N]                first-pass results and lane done-markers
Options: --lanes a,b (subset, e.g. for a rehearsal), --no-agents (watch: first pass + briefs only, no tmux).

Release: 23:00 Australia/Brisbane; Day N = that date's day of the month (Day 15 = 2026-10-15). The first pass is
pure ctf-run, no model: per lane, submit single-solve, then the untimed regular time trial and mastery (dual-pool
routes only - timed-by-default ones are left to the coordinator). Anything incorrect gets the grouped oracle. Each
lane's brief (results, errors, oracle output, instructions) is written to runs-dNN/rush/brief-<lane>.md and the lane
agent is told to read it - as soon as that lane's first pass ends, not when all lanes end.
Every ctf-run here and in the agents' panes runs with CTF_NO_RANKED=1: ranked attempts stay with the coordinator.
`--rehearse` skips the wait and replays the flow on an already-released day (solved routes only reconnect).
Attach: tmux -L huntress attach -t rush
"""
import json, os, re, subprocess, sys, threading, time, urllib.request
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
CTF = os.path.dirname(os.path.dirname(HERE))          # .../2026/ctf-run
REPO = os.path.dirname(CTF)                            # .../2026
WT_ROOT = os.path.join(os.path.dirname(REPO), "wt")    # .../huntress-ctfs/wt/<lane>
API = "https://2026.huntress.ctf.games"
AEST = timezone(timedelta(hours=10))                   # Brisbane: no daylight saving
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
    return dict(os.environ, HUNTRESS_GM_TOKEN=token(), HUNTRESS_GM_URL=API, CTF_NO_RANKED="1")


def sh(args, cwd=None, check=True, **kw):
    r = subprocess.run(args, cwd=cwd, text=True, capture_output=True, **kw)
    if check and r.returncode != 0:
        raise RuntimeError(f"{' '.join(args)}: {r.stderr.strip()[-400:]}")
    return r


def state_dir(day):
    d = os.path.join(CTF, f"runs-d{day:02d}", "rush")
    os.makedirs(d, exist_ok=True)
    return d


# ---------------------------------------------------------------- prep

def prep(day):
    """Persistent worktrees (Claude's folder trust is accepted once), reset to master, built warm."""
    os.makedirs(WT_ROOT, exist_ok=True)
    for lane in LANES:
        wt = os.path.join(WT_ROOT, lane)
        branch = f"day{day:02d}/{lane}"
        if not os.path.isdir(wt):
            sh(["git", "worktree", "add", "-f", wt, "-B", branch, "master"], cwd=REPO)
        else:
            dirty = sh(["git", "status", "--porcelain"], cwd=wt).stdout.strip()
            if dirty:
                log(f"{lane}: worktree dirty - stashing before reset")
                sh(["git", "stash", "push", "-u", "-m", f"rush day {day} prep"], cwd=wt)
            sh(["git", "checkout", "-B", branch, "master"], cwd=wt)
        # Gitignored, shared with the main checkout: fixtures (manifests point at main's artifacts) and .env.
        for rel in ("ctf-run/fixtures", ".env"):
            link = os.path.join(wt, rel)
            if not os.path.lexists(link):
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
    return tpl.format(day=day, prev=day - 1, lane=lane, prefix=prefix, src=src, fix=fix,
                      wt=os.path.join(WT_ROOT, lane), repo=REPO, state=state_dir(day))


def launch(day):
    have = sh(TMUX + ["has-session", "-t", SESSION], check=False).returncode == 0
    if have:
        log("tmux session already exists - not relaunching (tmux -L huntress kill-session -t rush to reset)")
        return
    sd = state_dir(day)
    e = env()
    envflags = sum((["-e", f"{k}={e[k]}"] for k in ("HUNTRESS_GM_TOKEN", "HUNTRESS_GM_URL", "CTF_NO_RANKED")), [])
    sh(TMUX + ["new-session", "-d", "-s", SESSION, "-n", "control", "-c", CTF] + envflags)
    for lane in LANES:
        prompt_file = os.path.join(sd, f"prime-{lane}.md")
        with open(prompt_file, "w") as f:
            f.write(prime_prompt(day, lane))
        launcher = os.path.join(sd, f"launch-{lane}.sh")
        with open(launcher, "w") as f:
            f.write("#!/bin/sh\n"
                    # --add-dir: briefs, first-pass runs, done markers and the real fixtures/artifacts live in the
                    # main checkout's ctf-run/ and utils/; without it every read there stops at a permission dialog.
                    # Deliberately not the repo root: that holds the jumpbox SSH key, and CTF_NO_RANKED only guards
                    # this machine - workers must not be able to reach a ranked run on the jumpbox.
                    # Model/effort pinned here: worktrees carry no settings file, so they would otherwise inherit
                    # whatever ~/.claude/settings.json says (including per-model effort overrides).
                    f'exec claude --model claude-opus-5-5 --effort high --permission-mode auto --add-dir "{CTF}" "{os.path.join(REPO, "utils")}" '
                    f'-n "d{day:02d}-{lane}" "$(cat {prompt_file})"\n')
        os.chmod(launcher, 0o755)
        sh(TMUX + ["new-window", "-t", SESSION, "-n", lane, "-c", os.path.join(WT_ROOT, lane)] + envflags + [launcher])
        log(f"{lane}: agent launched (window rush:{lane})")


def send(lane, text):
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "-l", text])
    time.sleep(0.3)
    sh(TMUX + ["send-keys", "-t", f"{SESSION}:{lane}", "Enter"])


# ---------------------------------------------------------------- watch / first pass

def get(url):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token()}"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())


def wait_release(day, rehearse):
    if not rehearse:
        start = datetime.now(AEST).replace(hour=22, minute=58, second=30, microsecond=0)
        while datetime.now(AEST) < start:
            time.sleep(5)
    deadline = time.time() + 45 * 60
    while time.time() < deadline:
        try:
            rows = get(f"{API}/?op=list&day={day}&all=true").get("challenges", [])
            rows = [c for c in rows if c.get("day") == day]
            if rows:
                log(f"day {day} released: {len(rows)} routes")
                return rows
        except Exception as e:  # noqa: BLE001
            log(f"list failed: {e!r}")
        time.sleep(3)
    sys.exit("no release within 45 minutes")


def merge_routes(rows):
    """Merge the day's routes into ctf-run's cache (a full op=list refresh takes ~90 s)."""
    path = os.path.expanduser("~/.cache/ctf-run/routes.json")
    try:
        cache = json.load(open(path))
    except (OSError, ValueError):
        cache = {"challenges": []}
    ids = {c["id"] for c in rows}
    cache["challenges"] = [c for c in cache.get("challenges", []) if c.get("id") not in ids] + rows
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(cache, f)
    os.replace(tmp, path)  # atomic: agents may read it concurrently


def ctf_run(args, out_dir):
    r = subprocess.run([os.path.join(CTF, "target", "release", "ctf-run")] + args + ["-o", out_dir],
                       cwd=CTF, text=True, capture_output=True, env=env(), timeout=900)
    res = re.search(r"submit +(CORRECT|incorrect|accepted|REFUSED|already_solved)", r.stderr)
    saved = re.search(r"\] saved +(\S+)", r.stderr)
    errors = [l for l in r.stderr.splitlines() if "] error " in l or l.startswith("error")]
    result = res.group(1) if res else ("ERROR" if r.returncode else "?")
    if result == "already_solved":  # rehearsals on a released day: the attempt was solved before
        result = "CORRECT"
    return {"result": result, "dir": saved and saved.group(1),
            "errors": errors[:40], "tail": r.stderr.strip().splitlines()[-8:]}


def first_pass(day, lane, rows, routes_cfg):
    prefix = LANES[lane][0]
    out = f"runs-d{day:02d}"
    mine = sorted(c["id"] for c in rows if c["id"].startswith(prefix))
    single = next((r for r in mine if "/" not in r), None)
    report = {"lane": lane, "routes": mine, "steps": []}
    if not single:
        report["note"] = "no single-solve route for this lane"
        return report
    step = ctf_run([single], out)
    step["mode"] = "single_solve"
    report["steps"].append(step)
    if step["result"] == "CORRECT":
        for mode, flag in (("time_trial", "-t"), ("mastery", "-m")):
            rid = f"{single}/{mode}"
            cfg = routes_cfg.get(rid)
            if not cfg:
                continue
            if not cfg.get("run_for_time_supported"):
                report["steps"].append({"mode": mode, "result": "SKIPPED (timed by default - coordinator only)"})
                continue
            s = ctf_run([single, flag], out)
            s["mode"] = mode
            report["steps"].append(s)
            if s["result"] != "CORRECT":
                break
    for s in report["steps"]:
        if s.get("result") == "incorrect" and s.get("dir"):
            r = subprocess.run(["python3", "-I", os.path.join(CTF, "tools", "oracle_groups.py"), s["dir"]],
                               cwd=CTF, text=True, capture_output=True, env=env(), timeout=1800)
            s["oracle"] = r.stdout.strip().splitlines()[-40:]
    return report


def brief(day, report):
    lane = report["lane"]
    done = all(s.get("result") in ("CORRECT",) or str(s.get("result")).startswith("SKIPPED") for s in report["steps"]) \
        and any(s.get("mode") == "mastery" for s in report["steps"])
    lines = [f"# Day {day} - {lane} lane: first-pass brief", "",
             f"Routes: {', '.join(report['routes']) or '(none)'}", ""]
    for s in report["steps"]:
        lines.append(f"## {s.get('mode')}: {s.get('result')}")
        if s.get("dir"):
            lines.append(f"run dir: `{os.path.join(CTF, s['dir'])}`")
        if s.get("errors"):
            lines += ["errors:", "```", *s["errors"], "```"]
        if s.get("oracle"):
            lines += ["grouped oracle (untimed partial submits):", "```", *s["oracle"], "```"]
        if s.get("result") not in ("CORRECT",) and s.get("tail") and not s.get("errors"):
            lines += ["tail:", "```", *s["tail"], "```"]
        lines.append("")
    tpl = open(os.path.join(HERE, "prompts", "task-done.md" if done else "task.md")).read()
    lines.append(tpl.format(day=day, lane=lane, wt=os.path.join(WT_ROOT, lane), repo=REPO,
                            state=state_dir(day), fix=LANES[lane][2], src=LANES[lane][1]))
    path = os.path.join(state_dir(day), f"brief-{lane}.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    with open(os.path.join(state_dir(day), f"firstpass-{lane}.json"), "w") as f:
        json.dump(report, f, indent=1)
    return path, done


def watch(day, rehearse):
    rows = wait_release(day, rehearse)
    merge_routes(rows)
    cfg = {c["id"]: c for c in rows}

    def lane_job(lane):
        t = time.time()
        try:
            rep = first_pass(day, lane, rows, cfg)
        except Exception as e:  # noqa: BLE001
            rep = {"lane": lane, "routes": [], "steps": [{"mode": "first pass", "result": f"CRASHED {e!r}"}]}
        path, done = brief(day, rep)
        log(f"{lane}: first pass {'COMPLETE' if done else 'needs work'} in {time.time() - t:.0f} s -> {path}")
        if NO_AGENTS:
            return
        try:
            send(lane, f"Day {day} is live. Read {path} and follow it.")
        except Exception as e:  # noqa: BLE001
            log(f"{lane}: could not reach the agent pane ({e!r}) - brief is at {path}")

    threads = [threading.Thread(target=lane_job, args=(l,)) for l in LANES]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    log("first pass finished for every lane; agents are working - `rush.py status` to follow")


def status(day):
    sd = state_dir(day)
    for lane in LANES:
        fp = os.path.join(sd, f"firstpass-{lane}.json")
        steps = json.load(open(fp))["steps"] if os.path.exists(fp) else []
        done = os.path.exists(os.path.join(sd, f"done-{lane}"))
        summary = " | ".join("{}={}".format(s.get("mode"), s.get("result")) for s in steps) or "-"
        print(f"{lane:9} first pass: {summary}   agent done: {'yes' if done else 'no'}")


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
    if cmd in ("prep", "all"):
        prep(day)
    if cmd in ("launch", "all"):
        launch(day)
    if cmd in ("watch", "all"):
        watch(day, rehearse)
    if cmd == "status":
        status(day)


if __name__ == "__main__":
    main()
