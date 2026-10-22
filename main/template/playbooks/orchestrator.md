# Orchestrator playbook: create and manage CTF lanes with parallel agents

For the **coordinator**: the one session that defines lanes, runs a release, watches the lane agents, merges,
deploys and spends ranked attempts. It assumes a solver binary built along the lines of `solver-runner.md`. Nothing
here names a particular lane, event or day.

## 1. Roles

| Role | Owns | Never does |
|---|---|---|
| **User** | approvals for anything ranked, timed, one-shot, destructive or outward-facing; lane scope | |
| **Coordinator** | lane registry, release automation, merges, deploys, the run host, ranked campaigns, docs | silently widen scope or spend ranked attempts without approval |
| **Lane agent** (one per lane, warm, in its own terminal) | its lane's module, new format readers, its notes, untimed attempts on its own routes | touch other lanes or new lanes, ranked runs, deploys, merges, shared-file signature changes, the route cache |
| **Automation** (first pass, watcher) | mechanical solves, briefs, stall detection | anything needing judgement |

Spawn agents only in the way the user asked for (e.g. warm instances in a terminal multiplexer rather than ad-hoc
sub-agents). Each spawn costs context; don't parallelise without a reason.

## 2. Lanes are configuration

Keep one **lane registry**, a single table that every tool reads:

```
lane-name -> (challenge-id prefix, lane source file, fixtures dir, notes dir pattern)
```

- **Adding a lane:** add a registry row, a lane module that implements the contract (initially failing loudly on every
  kind), a fixtures directory, a notes directory and a prime-prompt section if the lane needs special rules. Then run
  its single-solve by hand until it is CORRECT before letting automation touch it.
- **Retiring or pausing a lane:** remove its registry row. Tools iterate the registry, so nothing else changes.
- **Scope is an allowlist.** Work only on challenges whose id starts with a registered prefix. When the event adds a
  lane that is not registered, **do not engage with it in any capacity** unless the user says so. That includes list
  browsing, status queries, starts, downloads and solver scaffolding.
  - Every tool that enumerates challenges (surveys, campaigns, release watchers) must filter on the registry
    prefixes, not on "everything the API lists".
  - Put the rule explicitly in every agent prompt.

## 3. Standing policies (record each with the date and the user's reason)

- Timed and ranked attempts need explicit approval for each campaign; approval does not carry over.
- Solves beat leaderboard entries. One-shot modes (single attempt, no untimed pool) may be fired by automation only
  once single-solve is CORRECT and the replay is clean. A failure there automatically gets an untimed practice
  continuation and a grouped oracle (keep this feedback loop intact).
- Hold ranked campaigns until every lane of the day is solved, then run one campaign over all of the day's routes.
- Ranked attempts run only from the designated run host. If it is down, ask the user; never fall back to another
  machine.
- Never saturate shared endpoints: one request at a time or small bounded concurrency, timeouts, session reuse,
  backoff. A 5xx can be challenge content, so record it rather than retrying blindly.
- Probe grading in groups with visible progress, not with blind serial bisection.

## 4. Release-night flow

### Before release

- Commit everything to the main branch: lane worktrees branch from the committed HEAD.
- Check that the run host answers. Know how to tell "VPN down" from "host powered off".
- Refresh the route cache, both locally and on the run host.

### Prep (about 15 minutes before)

- One persistent git **worktree per registered lane**, reset to main on a `day<N>/<lane>` branch (stash anything
  dirty). Symlink the shared, git-ignored data (fixtures, the env file) into each one.
- Pre-build every worktree so the agents start warm.

### Launch

- One terminal window per lane, each running an agent with a **prime prompt** containing:
  1. **Identity:** its lane, worktree, branch and challenge prefix.
  2. **Prepare now:** read the repo instructions, the lane's recent notes and its predictions; read the lane module;
     run its replay. Then reply "ready" and wait.
  3. **Rules:**
     - its own prefix only, never other or new lanes;
     - which files it may edit freely and which only additively;
     - which files it must never edit (run loop, protocol, network, manifest);
     - no route-cache refreshes;
     - runs go to `-o runs-d<N>`;
     - what is free (single-solve, untimed regular attempts) and what is not;
     - never work around a refusal.
- Launch settings:
  - pin the model and effort;
  - allowlist only the lane's own solver binary;
  - give read access to shared data, but **not** to anything holding run-host credentials.
- Export the no-ranked guard variable in every agent pane, so the solver itself refuses ranked starts.
- Install hooks that write a heartbeat file on every tool call and a "stopped" file when the agent ends its turn.

### Release detection

- Poll a cheap per-lane status endpoint from shortly before release (a 404 turning into 200), with the full list as a
  fallback. Then each lane fetches its own routes with a prefix-filtered list query and merges them into the route
  cache under a lock.

### Automated first pass (no model)

- Per lane: single-solve; once it is CORRECT, the timed/mastery modes in the order the lane requires.
  - Dual-pool routes: an untimed regular attempt.
  - Timed-by-default routes: a timed start, or a one-shot with the explicit flag, per policy.
- Failures get the grouped oracle on an untimed attempt or the practice continuation.
- Write a **brief per lane**: the result per mode, run directories, errors, oracle output, route kind, plus a task
  section saying what to do next. Send each agent "read <brief>" as soon as *its* lane's first pass ends.

### Agent task (from the brief template)

1. Fix the lane.
2. Rebuild and replay (fixtures plus the new runs) until clean.
3. Submit only the modes that are not CORRECT yet.
4. Fixtures from the graded runs, then notes.
5. Commit on the lane branch.
6. Touch `done-<lane>` and report anything the coordinator must do.

### Watching

- **Stall detection:** the agent stopped after its brief, no heartbeat since, no done marker, and a grace period has
  passed (it may be waiting on a background job). The watcher then writes `stalled-<lane>.md` with the pane tail.
- **Takeover:** a coordinator tool that runs a stalled lane's remaining modes from its worktree build. It must hold
  back any timed or one-shot mode that already has an attempt, write fixtures and replay, then report. The
  coordinator reviews it, commits on the lane branch and touches `done-<lane>`.
- **Common stall:** the agent's own permission classifier refuses a legitimate untimed run. A good agent reports it
  and does not try to work around it. Take over rather than loosening the agent's permissions mid-run.
- **Messaging an agent:** clear any open picker first (e.g. Escape), but not while the agent runs a long job. Verify
  the message landed; if it didn't, do the small remaining step yourself.
- **Verify "done" independently:** query the status of every one of the day's routes. Markers and agent reports are
  claims, not evidence.

### Merge and deploy

- A merge script merges every lane with a done marker (no-fast-forward, stopping at the first conflict), then runs the
  tests and the **full** replay. Lanes can merge early; unfinished lanes' new fixtures may fail until they land.
- Deploy only between campaigns, as described in `solver-runner.md` §7: build off-host, keep the previous binary,
  atomic swap, refresh the remote route cache.

## 5. Ranked campaigns

1. **Survey:** for each in-scope route, `status` (attempts left, eligibility; note that eligibility can read false
   simply because the latest attempt was untimed) joined with the scoreboard (our rank and time, the leaders' times).
2. **Plan:** a JSON list `{route, rft, budget, past_first?}`.
   - Budget: about 3 runs where we aren't #1, fewer on small pools.
   - `past_first`: one run to widen a lead we already hold.
   - Exclude routes that are not eligible or have no attempts left.
3. **Run it detached on the run host**: `setsid nohup ... &`, wrapped in a `timeout` on the launching ssh, which often
   hangs. The campaign is resumable and writes one JSON line per run plus a human log.
4. **Stopping rules per route:** stop on #1, on budget, after N runs in a row without improvement, on no attempts left,
   or on **any** non-CORRECT result. Transient server errors back off and retry, but check whether a 5xx on submit
   consumed the attempt.
5. **On a non-CORRECT result**, stop and investigate before spending more on that route or its lane:
   - Pull the failing run directories. Replay them with the previous and current binaries: identical answers rule out
     a regression.
   - Run an independent checker over the failure *and* every saved graded run of that route family. Look for a
     property present only in the failures.
   - Try to reproduce on untimed variants (free). If the quirk never appears there, it may be ranked-only: weigh the
     evidence and the cost of one more ranked attempt with the user's standing approval in mind.
   - Fix, add a unit test from the real bytes, replay, deploy, then resume only the stopped routes with their
     remaining budget.
6. **Timing:** server-bound routes vary by time of day; re-roll them in the quietest window you have measured.
   CPU-bound routes are improved by engineering, not re-rolls: compare your legs with the leaders' totals first.

## 6. Monitoring patterns

- Every monitor's filter must match **all** terminal states (failure, crash, disconnect, exit), not just success.
  Silence must mean "still running", never "died".
- Make patterns specific enough to avoid false alarms. A bare `503` also matches a 503 ms leaderboard time; anchor
  on the log's own phrasing.
- For one notification, use a background command that exits on the condition. For a stream, use a tail with a
  line-buffered filter. Re-arm on expiry.
- Shells differ: unmatched globs can abort loops (use `nullglob` or bash), and unquoted variables may not
  word-split. Run multi-step remote loops under an explicit `bash -c`.
- On a slow remote, use one multiplexed ssh master. Check for leftover remote tail processes when done.

## 7. Documentation discipline

Keep coordinator knowledge in the repo, one topic per file, updated in place:

| File | Contents |
|---|---|
| `policies` | standing user decisions, each with its date and reason |
| `runbook` | the release-night steps and the shell gotchas |
| `leaderboard` | scoreboard API, what the timer measures, campaign tooling, latency by time of day |
| `grading-lessons` | each grader surprise: evidence, rule, procedure |
| `history` | a dated log of each release and campaign: results, attempts lost, fixes |
| lane-pattern doc | how each lane evolves, plus next-day predictions; written by the lane agents |
| per-challenge notes | written by the lane agents |

Record lessons where the next coordinator will look, not in private memory. When an instruction turns out wrong,
correct the document rather than adding a contradicting note.

## 8. Checklists

**New event**
- [ ] Read the organisers' docs; confirm API semantics (pools, timing flags, retry/practice, prerequisites).
- [ ] Build or port the solver per `solver-runner.md`; set up the guards and the no-ranked variable.
- [ ] Create the lane registry, release tooling (prep/launch/watch/first-pass/brief/takeover/merge), survey and
      campaign tools, all reading the registry.
- [ ] Write the policies file with the user's answers on approvals, scope and solves-vs-leaderboard.
- [ ] Rehearse the whole release on an already-released day.

**New lane**
- [ ] Registry row, module skeleton, fixtures dir, notes dir.
- [ ] Single-solve CORRECT by hand; fixtures; 100% replay.
- [ ] Prime-prompt section with any lane-specific rules.
- [ ] Rehearse its first pass.

**Every release**
- [ ] Main branch committed; run host up; route caches fresh; stale agent sessions killed.
- [ ] Prep, launch, watch; briefs dispatched; stalls taken over.
- [ ] Every route verified CORRECT by status; merge; tests plus full replay; deploy; untimed live check.
- [ ] With approval: survey, plan, campaign; investigate every non-CORRECT result; history entry.
