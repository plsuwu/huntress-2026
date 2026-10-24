# Web pipeline runbook

For a copy/paste walkthrough from a fresh terminal or chat, read
[docs/fresh-session.md](fresh-session.md). The Python coordinator automatically
starts the configured Claude workers; do not launch them manually.

## Preflight

1. Read AGENTS.md and docs/policies.md. Establish whether the task is explanation,
   read-only inspection, or an authorized solve.
   Workers receive docs/web-worker-lessons.md automatically; it summarizes the
   restored Day 19/20 references, cumulative mastery and historical code limits.
2. Ensure the GM token is in .env or the environment and Claude authentication
   works. Do not print credentials. No SSH, worktree or remote deployment setup
   is needed.
3. Run `python3 tools/orchcfg.py check` and
   `python3 -m unittest discover -s tests -v`.
4. Run `python3 tools/web.py plan --day N`. Review which routes are untimed and
   which are held. No attempt has started. If the shape is unfamiliar, update
   the adapter and tests using a sanitized read-only response before proceeding.
   N is the API release day: Day 21 includes curriculum 06–08. If sandbox DNS
   fails, use approved network-enabled execution; do not change DNS or the
   endpoint. Both `web` and `web-exploitation` are API lane names; the latter is
   the canonical response value used in configuration.

## Authorized solve

Run `python3 -u tools/web.py run --day N --execute`. The coordinator groups the day's
routes into challenge families and launches one worker per family (up to
`[agents].workers`, default 3). Each worker begins, solves and submits every mode itself
through `tools/wk.py`, single-solve → time trial → mastery, in one resumed Claude session,
moving on as soon as a mode is CORRECT. Other modes run only when their prerequisites are
confirmed CORRECT and the list advertises an untimed pool; a worker that must wait
(scoring pending, team target limit) waits inside `wk.py begin`. Real web assignments use
`targets[]`, with no downloadable artifacts. The coordinator monitors the workers and
takes over from failed ones; it never starts or submits. A nonzero exit means at least one
route needs attention or was held.

One independent Claude session owns each full base challenge id. GM requests from all
processes share one rate limit. To select one family, add
`--challenge web-exploitation-day-06-reflect-upon-this` on release Day 21. Use `--route`
to select a single mode instead.

For future releases use `python3 -u tools/web.py watch --day N --execute`.
It discovers immediately, polls every 30 seconds, and attempts each route once.
Remove `--execute` for read-only monitoring. Use `--once` to stop after one poll.
It does not infer release dates from a local clock or invent predictable IDs.

## Panes (tmux backend)

- `tmux -L huntress-web attach -t web` shows one window per active family; `python3 tools/web.py
  panes` lists them read-only. Typing into a pane is fine for a human; the coordinator only types a
  pointer to the next round's `prompt.md`.
- Blocked with "waiting on an interactive prompt": read `rounds/NNN/stalled.md`, attach, answer the
  folder-trust or permission dialog yourself, then rerun the same explicit command. The pane is kept.
- To reset a family's pane, `tmux -L huntress-web kill-window -t '=web:=<window>'` (the window name is
  the base id without `web-exploitation-day-`). Never touch `-L huntress`; never run `kill-server`
  there. Use `kill-server` on `huntress-web` only when no coordinator is running.
- `state: waiting` in `workers/web/<base>/<mode>/state.json` means a bounded bottleneck wait (scoring
  pending, or the team target limit); its `reason` says which. It resolves itself or ends `blocked`.
- Use `--backend print` to bypass tmux for one run if tmux or the pane launch misbehaves.

## Review, recovery and handoff

- `python3 tools/web.py status --day N` reads local job states.
- `python3 tools/web.py status --day N --refresh` queries all cached web routes
  for independent server evidence. This does not start attempts.
- Look under `workers/web/<base>/<mode>/`: state.json identifies the phase;
  start.json contains the full assignment; deliveries/manifest.json records
  names/hashes; results.json records grading; rounds/ preserves prompts, answers
  and logs; NOTES.md describes the worker's evidence.
- The family directory has challenge-state.json and worker-session.json.
  `targets.json` preserves assigned live sites; provisioning-*.json records the
  bounded readiness polling. Worker CLI sessions use the family directory so
  earlier mode notes remain available without sharing another family's data.
- On `accepted` or `incorrect`, the worker sees the GM's response itself and may try
  again, bounded by `max_rounds` submissions per attempt. After `needs_work`, inspect
  evidence and raise `max_rounds` explicitly; there is no unbounded guessing/oracle loop.
- A family that ends `needs_attention` (takeover limit, open dialog) keeps its files and,
  for tmux, its pane. Fix the cause and rerun the same `run --challenge ...` command: the
  leftover pane is replaced by a resumed worker with a takeover brief.
- After interruption or `submission_unknown`, read GM status first. A new `run`
  resumes through status and plain start, never retry. CORRECT status skips the
  worker; a changed attempt ID archives the previous job directory.
- Timed/unknown metadata, changed attempt IDs during work, unknown artifact
  links, bad hashes, permission refusals and unsupported API shapes block work.
  Diagnose from saved evidence. Do not loosen a guard to get past a refusal.
- CORRECT with `status=pending` remains `correct_pending`; do not claim points or
  run dependent modes until confirmed.
- Ctrl-C terminates the worker process group; assignments and evidence remain.
  Ctrl-C also closes the worker panes. Do not delete a lock file to unlock a running coordinator. The OS releases the
  advisory lock when the coordinator exits.
- An untimed target stops after 30 idle minutes. A target-capacity conflict is waited out
  (bounded); a dead target should be investigated from saved responses; the pipeline never
  sends reset, retry or switch_target, and must not disrupt a concurrent worker.

The old rush, campaign, oracle, merge and deploy entrypoints are disabled for
this configuration. They must not be used as fallback paths.
