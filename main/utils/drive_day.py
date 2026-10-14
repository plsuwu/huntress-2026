#!/usr/bin/env python3
"""Drive headless Claude sessions through every challenge of one or more CTF days.

One fresh `claude -p` session per challenge (equivalent to a /clear between
challenges). Success is judged from the Game Master's `solved` flags - never from
the agent's exit code or prose.

    utils/drive_day.py 6                        # every lane of day 6
    utils/drive_day.py 4-6 --lane warmups       # days 4..6, one lane
    utils/drive_day.py 6 --dry-run              # pre-flight table + prompts only
    tail -f utils/driver_runs/<run>/<id>.log    # watch what the agent is doing

Per challenge:
  1. Pre-flight: skip if all three modes are solved. For each unsolved timed mode
     (time_trial / mastery) work out the retries remaining; if any is below
     --min-retries the challenge is FLAGGED and claude is never invoked.
  2. Run claude with stream-json in AND out, so a "wrap up" message can be
     injected mid-run (it is delivered after the in-flight tool call).
  3. While it runs, poll `op=status`; if retries drop below --min-retries, or the
     time budget's grace window opens, or the operator hits Ctrl-C, inject the
     wrap-up message (no new timed attempts, write notes, stop). Hard-kill only
     after the grace window.
  4. Re-query the GM and classify: solved (all modes) / partial / failed, plus flags.

"Retries remaining" is `unique_attempts_remaining` from `op=status` once a timed
attempt exists; before that it is `ranked_pool_size - 1` from `op=list` (the
variants left after the first one `start` hands out).

Outputs go to --out-dir (default utils/driver_runs/<UTC stamp>/): per challenge
`<id>.stream.jsonl` (raw), `<id>.log` (human readable), `<id>.stderr`, and one
`summary.jsonl` record per challenge. Sessions are persisted, so a failed
challenge can be resumed with `claude --resume <session_id>`.
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
sys.path.insert(0, str(HERE))
import gen_prompts as gp  # noqa: E402  (reuses token loading, list call, prompt template)

MODES = gp.MODES
TIMED = ("time_trial", "mastery")

DRIVER_NOTE = (
    "\n\n--- Driver constraints (automated run) ---\n"
    "Mode state at start: {state}. Continue from the first open mode.\n"
    "You have a wall-clock budget of {timeout_min:g} minutes for ALL modes combined. "
    "Near the end you will receive a message beginning `DRIVER NOTICE`; obey it "
    "immediately.\n"
    "Never call `retry` or start a fresh timed variant if `unique_attempts_remaining` "
    "is below {min_retries}; stop and record that in your notes instead.\n"
    "Keep your notes in the work-in-progress directory current as you go, so an "
    "abrupt stop loses nothing."
)

WRAP_COMMON = (
    "Write your final notes now (what is solved per mode, what remains, and any "
    "pitfalls/observations for the next agent) into this challenge's directory under "
    "utils/solver_py_scripts/, tidy the workdir, then end your turn with a one-paragraph "
    "summary. Do not start any new work."
)
WRAP_MESSAGES = {
    "timeout": (
        "DRIVER NOTICE: the time budget is nearly exhausted (about {grace}s left). "
        "Do NOT call `retry`, start a new timed attempt, or begin another mode. "
        "If a live attempt can be submitted within seconds, you may do so. " + WRAP_COMMON
    ),
    "low_retries": (
        "DRIVER NOTICE: the retry pool for this challenge is nearly exhausted "
        "({detail}; minimum allowed {min_retries}). Do NOT call `retry`, `practice`, or "
        "start a new timed attempt. If a live attempt can be submitted within seconds, you "
        "may do so. " + WRAP_COMMON
    ),
    "operator": (
        "DRIVER NOTICE: the operator has asked this run to stop. Do not call `retry` or "
        "start new attempts. " + WRAP_COMMON
    ),
}

STOP = threading.Event()        # first Ctrl-C: graceful wrap-up
HARD_STOP = threading.Event()   # second Ctrl-C: kill now


# --------------------------------------------------------------------------- GM

def gm_status(token: str, challenge_id: str) -> dict:
    """GET op=status. Side-effect free; an unstarted mode answers `start_required`."""
    q = urllib.parse.urlencode({"op": "status", "challenge": challenge_id})
    req = urllib.request.Request(
        f"{gp.BASE_URL}?{q}", headers={"Authorization": f"Bearer {token}"}
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.load(resp)
    except urllib.error.HTTPError as e:
        try:
            return json.loads(e.read())
        except ValueError:
            return {"error": f"http_{e.code}"}
    except (urllib.error.URLError, TimeoutError, ValueError) as e:
        return {"error": f"request_failed: {e}"}


def fetch_day(day: int, token: str) -> list[dict]:
    """gp.list_challenges exits the process on failure; retry once and re-raise."""
    for attempt in (1, 2):
        try:
            return gp.list_challenges(day, token)
        except SystemExit as e:
            if attempt == 2:
                raise RuntimeError(f"cannot list day {day}: {e}") from None
            time.sleep(3)
    return []


@dataclass
class ModeInfo:
    id: str
    mode: str
    solved: bool
    pool: int          # ranked_pool_size from op=list


def group_modes(challenges: list[dict]) -> dict[str, dict[str, ModeInfo]]:
    out: dict[str, dict[str, ModeInfo]] = {}
    for c in challenges:
        base = c["id"].split("/", 1)[0]
        out.setdefault(base, {})[c["mode"]] = ModeInfo(
            id=c["id"], mode=c["mode"], solved=bool(c.get("solved")),
            pool=int(c.get("ranked_pool_size") or 0),
        )
    return out


def retries_remaining(token: str, m: ModeInfo) -> tuple[int | None, str]:
    """(remaining fresh variants, source) for one timed mode; None = unknowable."""
    st = gm_status(token, m.id)
    for key in ("unique_attempts_remaining", "ranked_attempts_remaining"):
        if isinstance(st.get(key), int):
            return st[key], f"status.{key}"
    if m.pool > 0:
        return max(m.pool - 1, 0), "list.ranked_pool_size-1"
    return None, "unknown"


def preflight(token: str, modes: dict[str, ModeInfo], min_retries: int) -> tuple[dict, list[str]]:
    """Retries remaining for every unsolved timed mode, and the ones below the floor."""
    info, low = {}, []
    for name in TIMED:
        m = modes.get(name)
        if m is None or m.solved:
            continue
        rem, src = retries_remaining(token, m)
        info[name] = {"remaining": rem, "source": src}
        if rem is not None and rem < min_retries:
            low.append(f"{name}={rem}")
    return info, low


# ------------------------------------------------------------------ rendering

def one_line(s: str, n: int = 160) -> str:
    s = " ".join(str(s).split())
    return s if len(s) <= n else s[: n - 1] + "…"


def describe_event(ev: dict) -> list[str]:
    ty = ev.get("type")
    lines: list[str] = []
    if ty == "system" and ev.get("subtype") == "init":
        lines.append(f"session {ev.get('session_id')} model={ev.get('model')}")
    elif ty == "assistant":
        for b in ev.get("message", {}).get("content", []):
            if b.get("type") == "text" and b.get("text", "").strip():
                lines.append("say: " + one_line(b["text"], 200))
            elif b.get("type") == "tool_use":
                name, inp = b.get("name", "?"), b.get("input", {}) or {}
                if name == "Bash":
                    detail = inp.get("command", "")
                elif name in ("Read", "Edit", "Write", "NotebookEdit"):
                    detail = inp.get("file_path", "")
                elif name in ("Grep", "Glob"):
                    detail = inp.get("pattern", "")
                elif name in ("Agent", "Task"):
                    detail = inp.get("description", "")
                elif name == "Skill":
                    detail = inp.get("skill", "")
                else:
                    detail = json.dumps(inp)
                lines.append(f"{name}: {one_line(detail)}")
    elif ty == "user":
        c = ev.get("message", {}).get("content")
        if isinstance(c, list):
            for b in c:
                if b.get("type") == "tool_result" and b.get("is_error"):
                    body = b.get("content")
                    if isinstance(body, list):
                        body = " ".join(x.get("text", "") for x in body if isinstance(x, dict))
                    lines.append("  ! tool error: " + one_line(body or "", 140))
    elif ty == "result":
        lines.append(
            f"RESULT {ev.get('subtype')} turns={ev.get('num_turns')} "
            f"cost=${ev.get('total_cost_usd')} :: {one_line(ev.get('result') or '', 160)}"
        )
    return lines


def mmss(sec: float) -> str:
    return f"{int(sec) // 60:02d}:{int(sec) % 60:02d}"


# ----------------------------------------------------------------------- run

@dataclass
class RunOutcome:
    elapsed: float = 0.0
    wrap_reason: str | None = None
    wrap_at: float | None = None
    killed: bool = False
    exit_code: int | None = None
    session_id: str | None = None
    cost: float | None = None
    turns: int | None = None
    result_subtype: str | None = None
    min_retries_seen: int | None = None
    low_detail: str = ""


def build_claude_cmd(args, session_name: str) -> list[str]:
    cmd = [args.claude_bin, "-p",
           "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
           "--name", session_name,
           "--permission-mode", args.permission_mode,
           "--permission-prompts", "none"]
    if args.model:
        cmd += ["--model", args.model]
    if args.allowed_tools:
        cmd += ["--allowedTools", args.allowed_tools]
    if args.max_budget_usd is not None:
        cmd += ["--max-budget-usd", str(args.max_budget_usd)]
    cmd += args.claude_arg
    return cmd


def kill_group(proc: subprocess.Popen, sig: int) -> None:
    try:
        os.killpg(proc.pid, sig)
    except (ProcessLookupError, PermissionError):
        pass


def run_claude(args, token: str, entry: dict, modes: dict[str, ModeInfo],
               prompt: str, logbase: Path, tag: str) -> RunOutcome:
    out = RunOutcome()
    raw = open(f"{logbase}.stream.jsonl", "w", buffering=1)
    log = open(f"{logbase}.log", "w", buffering=1)
    err = open(f"{logbase}.stderr", "w")
    budget = args.timeout_min * 60.0
    soft_at = max(budget - args.grace_sec, 0.0)

    proc = subprocess.Popen(
        build_claude_cmd(args, f"{tag}"), cwd=REPO, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=err, text=True, bufsize=1, start_new_session=True,
    )
    q: queue.Queue = queue.Queue()

    def reader() -> None:
        assert proc.stdout is not None
        for line in proc.stdout:
            raw.write(line)
            try:
                q.put(json.loads(line))
            except ValueError:
                pass
        q.put(None)

    threading.Thread(target=reader, daemon=True).start()
    t0 = time.monotonic()

    def say(msg: str) -> None:
        line = f"[{mmss(time.monotonic() - t0)}] {msg}"
        log.write(line + "\n")
        if not args.quiet:
            print(f"    {line}", flush=True)

    def send(text: str) -> bool:
        try:
            assert proc.stdin is not None
            proc.stdin.write(json.dumps(
                {"type": "user", "message": {"role": "user", "content": text}}) + "\n")
            proc.stdin.flush()
            return True
        except (BrokenPipeError, ValueError, OSError):
            return False

    stdin_closed_at: float | None = None

    def close_stdin() -> None:
        nonlocal stdin_closed_at
        if stdin_closed_at is None:
            stdin_closed_at = time.monotonic()
            try:
                assert proc.stdin is not None
                proc.stdin.close()
            except OSError:
                pass

    def wrap(reason: str, detail: str = "") -> None:
        out.wrap_reason, out.wrap_at, out.low_detail = reason, time.monotonic() - t0, detail
        say(f"==> wrap-up requested ({reason}{': ' + detail if detail else ''})")
        send(WRAP_MESSAGES[reason].format(
            grace=int(args.grace_sec), detail=detail, min_retries=args.min_retries))

    send(prompt)
    next_poll = t0 + args.poll_sec
    settle_until: float | None = None
    timed_ids = [m.id for n, m in modes.items() if n in TIMED and not m.solved]

    try:
        while True:
            now = time.monotonic()
            el = now - t0
            if HARD_STOP.is_set() or el >= budget:
                out.killed = True
                say("==> hard kill (" + ("operator" if HARD_STOP.is_set() else "time budget") + ")")
                break
            if out.wrap_reason is None and stdin_closed_at is None:
                if STOP.is_set():
                    wrap("operator")
                elif el >= soft_at:
                    wrap("timeout")
                elif now >= next_poll:
                    next_poll = now + args.poll_sec
                    seen = []
                    for cid in timed_ids:
                        st = gm_status(token, cid)
                        for key in ("unique_attempts_remaining", "ranked_attempts_remaining"):
                            if isinstance(st.get(key), int):
                                seen.append((cid, st[key]))
                                break
                    if seen:
                        cid, rem = min(seen, key=lambda x: x[1])
                        if out.min_retries_seen is None or rem < out.min_retries_seen:
                            out.min_retries_seen = rem
                        if rem < args.min_retries:
                            wrap("low_retries", f"{cid.split('/')[-1]} has {rem} left")
            if settle_until is not None and now >= settle_until:
                settle_until = None
                close_stdin()
            if stdin_closed_at is not None and now - stdin_closed_at > args.exit_wait_sec:
                say("==> process did not exit after stdin closed; terminating")
                out.killed = True
                break
            try:
                ev = q.get(timeout=1.0)
            except queue.Empty:
                continue
            if ev is None:
                break
            for line in describe_event(ev):
                say(line)
            ty = ev.get("type")
            if ty == "system" and ev.get("subtype") == "init":
                out.session_id = ev.get("session_id")
            elif ty == "assistant" and settle_until is not None:
                settle_until = None            # wrap-up started a new turn; keep waiting
            elif ty == "result":
                out.cost = ev.get("total_cost_usd")
                out.turns = ev.get("num_turns")
                out.result_subtype = ev.get("subtype")
                if out.wrap_reason is None:
                    close_stdin()              # agent finished naturally
                else:
                    # The wrap-up message may have been merged into the turn that just
                    # ended, or it may start another one; wait briefly to find out.
                    settle_until = time.monotonic() + args.settle_sec
    finally:
        if proc.poll() is None:
            close_stdin()
            try:
                proc.wait(timeout=5 if out.killed else args.exit_wait_sec)
            except subprocess.TimeoutExpired:
                out.killed = True
                kill_group(proc, signal.SIGTERM)
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    kill_group(proc, signal.SIGKILL)
                    proc.wait()
        kill_group(proc, signal.SIGTERM)       # reap any orphaned tool children
        out.exit_code = proc.returncode
        out.elapsed = time.monotonic() - t0
        for f in (raw, log, err):
            f.close()
    return out


# ---------------------------------------------------------------------- main

def parse_days(specs: list[str]) -> list[int]:
    days: list[int] = []
    for s in specs:
        if "-" in s:
            a, b = s.split("-", 1)
            days += list(range(int(a), int(b) + 1))
        else:
            days.append(int(s))
    return days


def solved_map(modes: dict[str, ModeInfo]) -> dict[str, bool]:
    return {m: (modes[m].solved if m in modes else False) for m in MODES}


def process_challenge(args, token, entry, modes, run_dir: Path, summary) -> dict:
    base = entry["id"]
    before = solved_map(modes)
    rec: dict = {
        "ts": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "day": entry["day"], "lane": entry["lane"], "id": base, "title": entry["title"],
        "modes_before": before, "flags": [],
    }
    if all(before.values()):
        rec.update(outcome="already_solved")
        return rec

    retries, low = preflight(token, modes, args.min_retries)
    rec["retries_before"] = retries
    if low:
        rec.update(outcome="skipped", flags=["low_retries"], low_detail=", ".join(low))
        return rec
    if args.dry_run:
        rec.update(outcome="dry_run")
        return rec

    state = ", ".join(f"{m}={'solved' if s else 'open'}" for m, s in before.items())
    prompt = entry["prompt"] + DRIVER_NOTE.format(
        state=state, timeout_min=args.timeout_min, min_retries=args.min_retries)
    logbase = run_dir / base
    tag = f"d{entry['day']:02d}-{entry['lane']}"
    print(f"  -> running claude (budget {args.timeout_min:g}m, grace {args.grace_sec}s) "
          f"log: {logbase}.log", flush=True)
    res = run_claude(args, token, entry, modes, prompt, logbase, tag)

    time.sleep(2)  # let CourseStack/GM state settle before judging
    try:
        after_modes = group_modes(fetch_day(entry["day"], token)).get(base, modes)
    except RuntimeError as e:
        after_modes = modes
        rec["flags"].append("gm_unreachable_after")
        rec["error"] = str(e)
    after = solved_map(after_modes)
    retries_after, low_after = preflight(token, after_modes, args.min_retries)

    if all(after.values()):
        outcome = "solved"
    elif sum(after.values()) > sum(before.values()):
        outcome = "partial"
    else:
        outcome = "failed"
    flags = rec["flags"]
    if res.wrap_reason == "timeout" or (res.killed and res.wrap_reason is None):
        flags.append("timeout")
    if res.wrap_reason == "low_retries" or (low_after and not all(after.values())):
        flags.append("low_retries")
    if res.wrap_reason == "operator":
        flags.append("operator_stop")
    if res.killed:
        flags.append("killed")
    if res.exit_code not in (0, None) and not res.killed:
        flags.append("claude_error")
    rec.update(
        outcome=outcome, modes_after=after, retries_after=retries_after,
        elapsed_s=round(res.elapsed, 1), wrap_reason=res.wrap_reason,
        wrap_at_s=None if res.wrap_at is None else round(res.wrap_at, 1),
        exit_code=res.exit_code, session_id=res.session_id, cost_usd=res.cost,
        num_turns=res.turns, result_subtype=res.result_subtype,
        min_retries_seen=res.min_retries_seen, low_detail=res.low_detail,
        log=str(logbase) + ".log",
    )
    rec["flags"] = sorted(set(flags))
    return rec


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("days", nargs="+", help="day numbers or ranges, e.g. 6  or  4-6  or  4 5 6")
    ap.add_argument("--lane", action="append", default=[], help="restrict to a lane (repeatable)")
    ap.add_argument("--only", action="append", default=[], help="restrict to a base challenge id")
    ap.add_argument("--dry-run", action="store_true", help="pre-flight only; never invoke claude")
    ap.add_argument("--timeout-min", type=float, default=12.0,
                    help="wall-clock budget per challenge across all modes (default 12)")
    ap.add_argument("--grace-sec", type=float, default=120.0,
                    help="wrap-up window at the end of the budget for writing notes (default 120)")
    ap.add_argument("--settle-sec", type=float, default=20.0,
                    help="after a result post wrap-up, wait this long for a new turn (default 20)")
    ap.add_argument("--exit-wait-sec", type=float, default=30.0,
                    help="max wait for the process to exit once stdin is closed (default 30)")
    ap.add_argument("--min-retries", type=int, default=5,
                    help="flag/abort when retries remaining fall below this (default 5)")
    ap.add_argument("--poll-sec", type=float, default=30.0, help="status poll interval (default 30)")
    ap.add_argument("--model", default=None)
    ap.add_argument("--permission-mode", default="auto",
                    help="claude --permission-mode (default auto; prompts are denied, never hang)")
    ap.add_argument("--allowed-tools", default=None, help="claude --allowedTools value")
    ap.add_argument("--max-budget-usd", type=float, default=None)
    ap.add_argument("--claude-bin", default="claude")
    ap.add_argument("--claude-arg", action="append", default=[],
                    help="extra raw argument passed through to claude (repeatable)")
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--quiet", action="store_true", help="don't echo the live feed to stdout")
    args = ap.parse_args()
    if args.grace_sec >= args.timeout_min * 60:
        ap.error("--grace-sec must be smaller than the time budget")

    days = parse_days(args.days)
    token = gp.load_token()
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    day_tag = "-".join(f"{d:02d}" for d in days)
    run_dir = Path(args.out_dir) if args.out_dir else HERE / "driver_runs" / f"{stamp}-day-{day_tag}"
    run_dir.mkdir(parents=True, exist_ok=True)
    summary = open(run_dir / "summary.jsonl", "a", buffering=1)

    def on_sigint(_s, _f):
        if STOP.is_set():
            HARD_STOP.set()
        STOP.set()
        print("\n!! stop requested - wrapping up current challenge "
              "(Ctrl-C again to kill)", flush=True)
    signal.signal(signal.SIGINT, on_sigint)

    print(f"run dir: {run_dir}")
    results: list[dict] = []
    for day in days:
        if STOP.is_set():
            break
        challenges = fetch_day(day, token)
        entries = gp.build_prompts(challenges, set(args.lane), unsolved=False)
        if args.only:
            entries = [e for e in entries if e["id"] in set(args.only)]
        grouped = group_modes(challenges)
        print(f"\n=== day {day:02d}: {len(entries)} challenge(s) ===")
        if not entries:
            print("  (nothing matched - day not released, or filters excluded everything)")
            continue
        for entry in entries:
            if STOP.is_set():
                break
            print(f"\n[{entry['lane']}] {entry['id']}", flush=True)
            rec = process_challenge(args, token, entry, grouped[entry["id"]], run_dir, summary)
            summary.write(json.dumps(rec, ensure_ascii=False) + "\n")
            results.append(rec)
            fl = f" flags={rec['flags']}" if rec["flags"] else ""
            extra = f" ({rec['low_detail']})" if rec.get("low_detail") else ""
            print(f"  = {rec['outcome']}{fl}{extra}", flush=True)

    print("\n=== summary ===")
    for r in results:
        print(f"{r['day']:02d} {r['outcome']:<14} {','.join(r['flags']) or '-':<22} {r['id']}")
    bad = [r for r in results
           if r["outcome"] not in ("solved", "already_solved", "dry_run") or r["flags"]]
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
