# Web pipeline adaptation plan

How to rebuild the web solution pipeline as an adaptation of the main solution pipeline
(`/home/please/Documents/huntress-ctfs/2026`), per `ADAPTATION-REQUIREMENTS.md`. The previous
web automation (`tools/web.py`, `tools/wk.py`, `config.toml`, planners, tests, playbooks) was
torn out as ineffective; this plan ignores it in favour of the main pipeline's proven
`rush.py` design. Written 2026-10-24. Status: steps 1–2 (prompt templates, `tools/wk.py`) done
2026-10-24; steps 3–5 pending.

## 1. Core idea

Port `ctf-run/tools/rush/rush.py`'s **shape** — a single Python driver with
`prep / launch / watch / status / takeover` subcommands, a private tmux server, one primed
Claude window per work item, marker files plus Stop/heartbeat hooks for stall detection, an
automated first pass that writes a markdown brief and dispatches it to the agent — and change:

- **what the work items are**: web-exploitation challenges (discovered at release), not the
  five lanes;
- **what the first pass runs**: a Python recon + recovery pipeline over live targets
  (`solver_py_scripts/recovery_lib.py`), not the `ctf-run` binary;
- **how workers solve**: `uv`/Python with a guarded `tools/wk.py` GM helper, not Rust.

The old web pipeline failed because it rebuilt a config-driven framework (config.toml,
planner, API adapters, session-resume takeover, synthetic test suite). The main pipeline
works because it is one ~1000-line script with direct GM calls and tmux as the only UI.
The web pipeline is `rush.py` with the lane table replaced by "web challenges discovered at
release".

## 2. Requirements (from `ADAPTATION-REQUIREMENTS.md`)

1. Web pipeline exclusively solves the `web-exploitation` lane.
2. Multiple workers: the lane always has more than one challenge per day (2–3 observed on
   release days 19–23). Pre-warm/contextualize 3 workers; hand them their challenge IDs when
   the challenges become available.
3. Unique tooling names: tmux server names and worker directories must not conflict with the
   main pipeline.
4. Worker prompts: each worker is given its challenge ID and told to solve `single_solve`,
   `time_trial` and `mastery` for that challenge as quickly as possible.
5. No `ctf-run` binary — workers solve using `uv`/Python.
6. Workers take notes; the orchestrator merges them into a document that assists future
   worker agents.

## 3. Naming and isolation

| Resource | Main pipeline | Web pipeline |
|---|---|---|
| tmux socket | `-L huntress` | `-L huntress-web` |
| tmux session | `rush` | `web` |
| tmux windows | `control` + one per lane | `control` + one per challenge (`day-NN-<slug>`) |
| Worker dirs | `../wt/<lane>` (git worktrees) | `workers/web/<full-base-id>/` (plain dirs) |
| Driver state | `ctf-run/runs-dNN/rush/` | `workers/web/.rush-dNN/` |
| Driver script | `ctf-run/tools/rush/rush.py` | `tools/web_rush.py` |
| Worker GM helper | `ctf-run` binary | `tools/wk.py` |
| Prompts | `ctf-run/tools/rush/prompts/*.md` | `tools/prompts/{prime,task,task-done}.md` |
| Route cache | `~/.cache/ctf-run/routes.json` | `workers/web/routes-dNN.json` |
| Events log | — | `workers/web/.events` (route-completion lines; no GM lock — section 8) |

Scope guard, symmetric to rush's prime.md ("never web-exploitation"): **web workers only ever
touch ids starting `web-exploitation-`**. The driver's route fetch uses
`op=list&day=N&lane=web-exploitation` and re-filters rows by the `web-exploitation-` prefix,
exactly like rush's `lane_rows()` filters by lane prefix. The main pipeline's agents already
refuse web; web workers refuse everything else.

## 4. Directory layout

```
2026-web/
  tools/
    web_rush.py            # driver: all|prep|launch|watch|status|takeover [--day N] [--rehearse] [--workers N]
    wk.py                  # worker GM helper: begin|submit|status|next (untimed-only guards)
    prompts/
      prime.md             # pre-release warm-up prompt (per worker slot)
      task.md              # "finish this challenge" brief tail
      task-done.md         # "first pass solved it all — bookkeeping only" tail
  workers/web/             # git-ignored runtime state (existing)
    <base-id>/
      family.json
      single_solve/  start.json targets.json answers.json results.json NOTES.md solve.py <evidence>
      time_trial/    ...
      mastery/       ...
      prompts/launch-001.md
      tmux/          settings.json
    .rush-dNN/
      prime-<slot>.md  settings-<slot>.json  launch-<slot>.sh
      brief-<chal>.md  firstpass-<chal>.json
      sent-<chal>  stopped-<chal>  active-<chal>  done-<chal>  stalled-<chal>.md
    routes-dNN.json  .events
  solver_py_scripts/
    recovery_lib.py                       # shared classifiers/recoverers — grows every day
    day-NN-web-exploitation/<slug>/{solve.py,NOTES.md}
  docs/  runbook.md  policies.md  history.md  lane-patterns.md  web-worker-lessons.md  live-web-catalog.md
  ADAPTATION-REQUIREMENTS.md  ADAPTATION-PLAN.md (this file)
```

Keep the old pipeline's per-mode state layout (`start.json`, `targets.json`, `answers.json`,
`results.json`, `NOTES.md`, `solve.py`, evidence dumps) — it is proven and the NOTES files are
the merge source. What goes away is everything around it: `config.toml`, the planner,
`orchcfg.py`, `tests/`, `playbooks/`, `worker-session.json`/`challenge-state.json`
bookkeeping, `rounds/` prompt logs.

No git worktrees and no `merge.sh`: there is no Rust codebase, and concurrent workers write
disjoint per-challenge directories. The user initialized a git repository on 2026-10-24
(initial commit on `master`; `workers/web/`, `.env`, the SSH key and the VPN config are
git-ignored). Workers commit their own day directory
(`solver_py_scripts/day-NN-web-exploitation/<slug>/`) directly to `master` — disjoint paths,
so no branches or merge step are needed (retry once if another worker's commit holds the
index lock). The coordinator commits the shared-doc updates (section 10).

## 5. The driver: `tools/web_rush.py`

Copy rush.py's skeleton nearly verbatim — `log()`, `token()` (env or `.env`), `env()`,
`sh()`, `state_dir()`, the `Release` class, `merge_routes()`, `send()` with the **Escape-first
picker dismissal**, `mtime()`/`stall_since()`/`monitor()`, `slugify()`, and the `main()`
subcommand dispatch. Then adapt:

- **`LANES` table → nothing.** Work items are discovered at release:
  `op=list&day=N&lane=web-exploitation`, grouped into base challenges (strip `/time_trial`,
  `/mastery`). The count varies; there is no static table.
- **`prep(day)`** → create `workers/web/<base-id>/` dirs; symlink the read-only shared trees
  in (`AGENTS.md`, `docs/`, `solver_py_scripts/`, `tools/`) the way rush.py symlinks
  `fixtures` and `.env` into worktrees; clear stale `.rush-dNN` markers. No `cargo build` —
  nothing to compile. Symlinking `AGENTS.md`/`docs/` into the worker dir matters: the repo
  root holds `huntress-id_ed25519` and `.env`, so the root must never be a worker's cwd or
  an `--add-dir` (same discipline as rush's "not the repo root: the jumpbox key lives
  there").
- **`launch(day, n_workers=3)`** → `tmux -L huntress-web new-session -d -s web -n control`
  with `-e HUNTRESS_GM_TOKEN=... -e HUNTRESS_GM_URL=... -e WEB_UNTIMED_ONLY=1` via tmux env
  flags, exactly like rush's `envflags`, then one window per pre-warmed slot, each running a
  generated `launch-<slot>.sh`:

  ```sh
  exec claude --model claude-opus-5-5 --effort high --permission-mode auto \
    --add-dir "<root>/docs" "<root>/solver_py_scripts" "<root>/tools" \
    --allowedTools "Bash(python3 <root>/tools/wk.py *)" \
    --settings "<state>/settings-<slot>.json" -n "web-dNN-<slot>" "$(cat prime-<slot>.md)"
  ```

  Model/effort pinned like rush (workers have no settings file to inherit from).
  `--allowedTools` covers the wk.py GM path so the auto-mode classifier cannot stall a
  submit (the Day 16 lesson). Everything else (curl, `uv run`, python3 scripts) runs under
  auto mode. `all` kills the previous day's tmux session before relaunching, like rush.
- **`watch(day)`** → release detection adapted to web: poll
  `op=list&day=N&lane=web-exploitation` every 1 s from 22:59:50 (the lane-filtered list is
  fast; a full `all=true` list takes 4–10 s — avoid it). When rows appear: group into base
  challenges, save `routes-dNN.json`, run the **first pass** (section 6) in one thread per
  challenge, write `brief-<chal>.md` per challenge, and dispatch each brief to its worker
  window **as soon as that challenge's first pass ends** (not when all end — rush's rule).
  Then `monitor()`.
- **Assignment at release:** pre-warmed slot *i* gets challenge *i*. If a day releases more
  challenges than slots, `tmux new-window` extra primed workers on the spot (the assignment
  message includes the challenge id). If fewer, extra slots get "stand down" and their windows
  close. Window names use the challenge slug (`day-NN-<slug>`); markers use the same short
  name, same convention as rush's `done-<lane>`.
- **`status(day)`** → rush's status, one line per challenge: first-pass step results plus
  agent state (`done` / `STOPPED N s ago` / `working`).
- **`takeover(day, challenges)`** → not "run ctf-run against the worktree" and not the old
  pipeline's session-resume. Capture the pane tail + GM status per route + a listing of the
  worker dir, write a takeover brief, kill the stalled window, launch a **fresh** worker
  window whose prompt is the brief. `wk.py begin` is idempotent (plain `start` reconnects),
  so the replacement resumes the same attempt with no lost work. The coordinator can also
  just nudge first (`send()`).

## 6. The automated first pass

Rush's first pass is pure automation, no model, and it solved whole lanes unaided on many
days. Web can do the same because `solver_py_scripts/recovery_lib.py` already classifies and
recovers 10 bug classes. Per challenge, in a thread:

1. **Start single_solve** (plain `POST {"op":"start","challenge":"<base>"}` — untimed). Save
   `start.json` (answer_ids, questions, targets, attempt_id).
2. **Wait for targets** (bounded status poll until `result` leaves `preparing`; save
   `targets.json` and each provisioning response). Handle `409 web_target_limit_reached` with
   a bounded wait (poll `workers/web/.events` for a teammate finishing, then retry; never
   switch/reset another family's target).
3. **Recon each target**: `GET /` (save `root.html` + headers), `GET /api/clues` (save
   `clues.json`). Cheap, and seeds the worker even when recovery fails.
4. **Classify + attempt recovery**: `classify_and_recover` per target, joining
   `questions[].artifact` → `targets[].id` (never array order). If every answer id has a
   proof → write `answers.json` → **submit**.
5. **Chain on CORRECT**: start `time_trial` (reconnect-safe), recon/recover/submit; on
   CORRECT, `mastery` (2–3 targets, classify each independently — mastery is cumulative
   across the curriculum). Mirrors rush's `chain()`: a mode is never started before its
   prerequisite is CORRECT (they answer `409 prerequisite_required`).
6. **On `incorrect`/`accepted`**: record `accepted_ids`/`remaining` in the brief — on an
   untimed attempt, partial submits are a free per-field oracle, and the worker can continue
   submitting subsets through `wk.py`.
7. **Write the brief** (`brief-<chal>.md`, modeled on rush's `brief()` + `task.md`): routes,
   per-mode result (`CORRECT (auto)` / `needs work`), run dir paths, recon evidence summary,
   submit feedback, and the task tail (`task-done.md` variant when the first pass solved
   everything — bookkeeping only: write NOTES.md, touch the done marker).

Guards, mirroring rush's `CTF_NO_RANKED=hof`: the first pass **never** sends
`run_for_time:true`, never `retry`/`practice`/`reset`/`switch_target`. All web TT/mastery
routes observed so far are dual-pool (`run_for_time_supported:true`), so plain starts are
free untimed regular attempts. GM calls are direct — no lock, no proactive pacing
(2026-10-24 decision: solution speed over gating; workers and the first pass are
trusted not to flood the GM, and both back off reactively on 429/5xx — reintroduce a
shared lock if rate limits appear).

Run first passes **serially per challenge** (or bounded to 1–2 threads): 3 challenges × up to
3 live targets each can hit the team target limit, and the 30-minute untimed-target idle
clock means the worker handoff must happen while the first pass's targets are still warm.
`start` is idempotent, so the worker reconnects to the first pass's attempt — no work is
duplicated or lost.

## 7. Worker model and prompts

**Pre-warm 3 workers** at ~22:45 with `prime.md`:

> You are web worker {i} of {n} for Huntress CTF Day {day}. The other workers own other web
> challenges; the main pipeline's lane agents own every other lane — never touch their
> challenges. The day has NOT been released yet — prepare now, then wait.
>
> Your worker directory is `{wt}`. Work only there.
>
> Prepare: 1. Read `AGENTS.md` (symlinked), `docs/web-worker-lessons.md`. 2. Read
> `docs/lane-patterns.md` (web section) and `solver_py_scripts/recovery_lib.py`. 3. Skim
> the newest `solver_py_scripts/day-*-web-exploitation/*/NOTES.md`.
>
> Then reply only "ready" and wait. When the day is released you will be told your challenge
> id and to read a brief the automated first pass wrote. Do not contact the Game Master
> before that.
>
> Rules: your challenge's ids only (`web-exploitation-day-NN-<slug>*`); untimed only — never
> `run_for_time:true`, `retry`, `practice`; you hold the GM token in your environment — use
> it only through `tools/wk.py`, never send it to a target; solve single_solve → time_trial
> → mastery as fast as possible, submitting as soon as you have answers; partial submits are
> a free oracle on untimed attempts; keep evidence and NOTES.md per mode; touch
> `{state}/done-<chal>` when every mode is CORRECT.

At release, `send()` delivers: *"Day {day} is live. Your challenge is `{base}` ({title}).
Read `{brief path}` and follow it."* The brief's task tail (adapted from rush's `task.md`)
tells the worker to finish whatever is not CORRECT, in mode order, reconnecting with
`wk.py begin` (idempotent), submitting with `wk.py submit`, iterating on
`accepted`/`remaining` feedback, writing
`solver_py_scripts/day-NN-web-exploitation/<slug>/NOTES.md`, keeping `solve.py` generic
(derive routes from `/api/clues`, never hard-code per-attempt answers), committing the day
directory, and touching the done marker.

Prompt shape change from the main pipeline, per the requirements: workers are **not** told a
lane — they are told a **challenge id** and told to drive all three modes. One worker = one
challenge family end-to-end (the old pipeline's "one session per family" decision was right;
keep it).

## 8. `tools/wk.py` — the worker's only GM door

Much simpler than the old one — no config, no planner, no registry beyond a `family.json`
in the worker dir. Four commands, all guarded:

- `wk.py begin [mode]` — plain `start` (reconnect-safe); bounded wait for
  `preparing`→ready targets and for target-capacity 409s; saves `start.json`/`targets.json`/
  provisioning evidence into the mode dir.
- `wk.py submit [mode]` — submits `answers.json` with **exactly** the `answer_ids` from the
  current `start.json`, verified against the current `attempt_id`; bounded submission count;
  prints the GM verdict (`CORRECT` / `accepted`+`remaining` / `incorrect`).
- `wk.py status` — local record per route (last verdict, attempt_id, target count);
  `status --live` additionally runs GM `op=status` for the family's three routes
  (read-only) and saves the raw response to `<mode>/status-gm.json`.
- `wk.py next` — first route not CORRECT, read from the local `results.json`
  (CORRECT is terminal, so the local record is authoritative; no GM calls).

Hard guards (kept from the old pipeline — they were the good part): untimed only (refuse any
`run_for_time`, `retry`, `practice`, `reset`, `switch_target` — there is no code path for
them, plus an env check), own-family routes only (challenge id must match `family.json`'s
base), token only from env, never printed, never sent to a target, artifact redirects
rejected, no GM lock or proactive pacing (2026-10-24 decision — workers are trusted not to
flood the GM; `wk.py` backs off reactively on 429/5xx, and a shared lock comes back if rate
limits appear), bounded HTTP timeouts and retries. Exit codes:
0 ok, 1 blocked (message on stderr — do not work around), 75 still waiting (run again).

The GM machinery (`gm_request`, `route_status`) and the evidence shapes (`answers.json`
`{"answers": {...}}`, `results.json` round list, `result.json`) are importable —
`web_rush.py` does `import wk` so the first pass and the workers share one
implementation. A CORRECT submit appends `<route> correct` to `workers/web/.events`.

## 9. Stall detection and monitoring — copy rush.py exactly

Per-worker `settings-<slot>.json` with the same three hooks (`Stop` → `touch stopped-<chal>`;
`PostToolUse(*)` + `UserPromptSubmit` → `touch active-<chal>`), the `sent-<chal>` marker
written when the brief is dispatched, `STALL_GRACE_S = 180`, `monitor()` escalating to
`stalled-<chal>.md` with the pane tail and the takeover command. `send()` keeps the
Escape-first picker dismissal (the Day 17 "Teach auto mode" picker ate typed messages). The
coordinator watches via `tmux -L huntress-web attach -t web` and
`python3 tools/web_rush.py status --day N` — human oversight stays intuitive through tmux.

## 10. Notes → future-worker knowledge (requirement 6)

The loop that makes the pipeline compound:

1. Each worker writes `solver_py_scripts/day-NN-web-exploitation/<slug>/NOTES.md` (existing
   convention) and keeps a generic `solve.py`.
2. After the day (coordinator, human-driven, ~10 min — the workers have already committed
   their own day directories; what remains is the shared documentation):
   - fold new bug-class methods into `docs/web-worker-lessons.md` (the recognized-families
     table);
   - fold the day's outcome + next-day prediction into `docs/lane-patterns.md`;
   - **fold any new `rec_*` recoverer into `solver_py_scripts/recovery_lib.py`'s
     `classify_and_recover` dispatch** — this is what makes the *next* day's automated first
     pass stronger; the old pipeline told workers not to edit the shared tree, so new classes
     never reached a first pass. The coordinator does it deliberately, after reviewing the
     graded evidence;
   - append a dated entry to `docs/history.md`.
3. `docs/web-worker-lessons.md` is injected into every worker prompt (prime + brief), so each
   worker starts with the accumulated curriculum knowledge — including that mastery mixes
   earlier bug classes and every target must be classified independently.
   The coordinator commits these shared-doc updates (one commit per day is fine).

## 11. Docs to write/update

- `docs/runbook.md` — rewrite as the rush-style release-night runbook: preflight (token in
  `.env`, `python3 tools/web_rush.py status --day N-1` sanity), start
  `tmux new -d -s web-driver "python3 -u tools/web_rush.py all --day N"` at ~22:45, watch /
  attach / nudge / takeover commands, notes-merging checklist. Remove the old
  `fresh-session.md` / `api-contract.md` / playbook references.
- `docs/policies.md` — update: web-exploitation scope only; untimed only (no timed/ranked
  override — HoF campaigns for web stay a coordinator decision using the main pipeline's
  jumpbox tooling, out of scope for workers); workers hold the token via `wk.py`; no
  sub-agents (warm tmux workers, per the main pipeline's standing policy); GM endpoint-load
  discipline (shared lock, bounded concurrency, back off on 429, record 5xx as evidence);
  30-min target idle + capacity-wait rules.
- `docs/history.md`, `docs/lane-patterns.md` — keep, append per day.
- `README.md` — rewrite to describe the rush-shaped pipeline (the current README describes
  the torn-out one and should not survive as-is).

## 12. What deliberately does *not* carry over from the old web pipeline

- `config.toml`, `orchcfg.py`, the planner/`plan` command, API-shape adapters, `--catalog`
  offline mode, the `inspect`/`panes` subcommands — ceremony; the driver talks to the GM
  directly.
- Session-resume takeover (`worker-session.json`, `rounds/`, takeover briefs resuming the same
  Claude session) — replaced with rush-style takeover: fresh window + brief, idempotent
  `begin` reconnects.
- The blanket-allow-rule rejection / explicit per-tool allowlist — use rush's
  `--permission-mode auto` plus a pinned `--allowedTools` for `wk.py` only.
- The strict "coordinator never starts or submits, ever" split — rush's first pass starts
  attempts and the agent continues on the same attempt; for web, the first pass starting
  attempts (and submitting when auto-recovery fully solves a mode) is the main speed win,
  and `start` idempotency makes the handoff lossless. The coordinator *process* still never
  submits on a worker's behalf.
- Ranked/timed/campaign machinery — not in this pipeline.

## 13. Open decisions

1. **Model pin**: rush pins `claude-opus-5-5 --effort high`; the old web pipeline used
   sonnet. Web exploitation is creative work where model quality matters most — match rush
   (opus) unless cost is a concern; it is a one-line constant either way.
2. **Worker count**: 3 pre-warmed slots covers release days 19–23 (max 3). If a day ever
   releases 4+, the driver launches extras at release — confirm that is acceptable vs.
   capping at 3 and queueing.
3. **First-pass aggressiveness**: should the automated first pass submit when it has full
   answers (recommended — it is what makes rush fast), or only recon and leave all submits
   to the worker? Submitting costs nothing on untimed routes, and `accepted`/`remaining` is
   banked progress.
4. **git** (resolved 2026-10-24): the user initialized a repository in the web directory.
   Workers commit their own day directories to `master`; the coordinator commits shared-doc
   updates. See section 4.
5. **GM rate limiting** (resolved 2026-10-24, user direction): no `.gm.lock`/`.gm.last`
   gating in `wk.py` — solution speed over gating. Workers are trusted to use the
   partial-submit oracle without flooding the GM; `wk.py` still retries transient
   429/5xx/network failures with backoff. Reintroduce a shared lock if rate limits
   are hit.

## 14. Implementation order

1. `tools/prompts/prime.md`, `tools/prompts/task.md`, `tools/prompts/task-done.md` — done
   2026-10-24.
2. `tools/wk.py` (guards first; it is the security boundary) — done 2026-10-24.
   Four commands (`begin`/`submit`/`status [--live]`/`next`), untimed-only and
   own-family guards, no GM lock per user direction (section 13 item 5), evidence
   shapes importable for `web_rush.py`. Tested against the live GM (read-only paths)
   and a synthetic worker dir (guards, validation, error paths).
3. `tools/web_rush.py`: skeleton copied from `rush.py` (log/token/env/sh/Release/send/
   stall machinery), then `prep`/`launch`/`watch`/`status`/`takeover` adapted per sections
   5–6.
4. Rehearse with `--rehearse` against already-released days (routes reconnect via status;
   solved routes are recognised from GM status without launching a worker).
5. Rewrite `docs/runbook.md`, `docs/policies.md`, `README.md`; delete references to the old
   tooling.
