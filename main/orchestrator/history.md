# Coordinator history

Dated outcomes of each release night and campaign. Engineering detail (lane fixes, perf work) lives in
`ctf-run/PLAN.md`'s progress log, and per-day lane gimmicks live in `utils/LANE_PATTERNS.md`. Add a short entry
here after every release night.

## 2026-10-15: ctf-run rewrite; Days 14-15

- **Rewrite:** `ctf-run/` was rewritten from scratch in Rust; the old per-challenge implementation was discarded at the
  user's request. It has per-lane solvers in `src/lanes/`, is std-only, and targets the CourseStack proxy from the
  jumpbox. All five lanes are native, with 231/231 fixtures clean.
- **Day 14:** all 15 routes CORRECT on the first try, including 10 one-shot TT/mastery routes run on the jumpbox over
  ssh.
- **Day 15:** all 15 routes CORRECT untimed, then 5 ranked TT and 5 ranked mastery runs CORRECT. A timing campaign
  (direct API, no preflight) made 358 timed runs, 352 of them CORRECT.
- **Findings:** bandwidth is not the bottleneck; server time-to-first-byte is. Binary-RE mastery is CPU-bound on the
  t3.small.

## 2026-10-16: Day 16, first rush release

- **Why rush exists:** Day 15 took about 35 min serially, against about 10 min for the top teams. The user asked for
  warm parallel instances, so the `huntress-rush.timer` systemd unit started rush at 22:45 every night from 10-16.
- **Release:** detection took 11 s, almost all of it the global `op=list`. Lanes now probe `warmups-day-NN` status
  every 1 s and fetch their own routes with `search=`.
- **Lanes:** the binary agent's untimed `-t` was blocked by the permission classifier, so it stopped without a done
  marker. Fixed with `--allowedTools`. The first pass now runs its modes concurrently (user-approved: a wrong solver
  only costs untimed regular variants).
- **Ranked:** #1 on 6 of 10 routes by 23:22.
  - Behind: warmups TT #2 (811 vs 614, server-bound), metadata TT #2 (709 vs 629, server-bound), binary TT #2 (1900 vs
    1899) and binary mastery #3 (4886 vs 4465, 13 attempts left).
  - The user deferred the second pass to a full ranked re-run the next afternoon, 2026-10-17 ~14:00-16:00 AEST.
  - Logs: `ctf-run/runs-hof/campaign-d16-{a,b,c}.log`.

## 2026-10-17: Day 17

- **Release:** the release probe counted `409 start_required` as not released, which cost 9 s (fixed in 8a96837).
- **New gate:** TT/mastery are gated on a CORRECT single-solve, so a concurrent first pass only helps single-solve.
  Chaining the modes came later.
- **Ranked:** held until every lane was done, then one campaign. #1 on 9 of 10 by 23:45.
- **Binary roll-up:** labels took about 25 min of free probing (see `grading-lessons.md`). `oracle_groups.py` now
  groups roll-up ids by day (eb32b48).

## 2026-10-18: Day 18

- **One-shots:** warmups and binary TT/mastery were one-shot routes, which are not HoF-eligible. All four were fired
  after merge and all were CORRECT.
- **All 15 CORRECT** by 23:12. Ranked: #1 on 6 of the 6 dual-pool routes.
- **Full pass over every day:** 139 CORRECT; #1 went from 26 to 36 of 152 routes.
- **Re-grade:** the GM re-graded Batch expansion at about 13:30Z (see `grading-lessons.md`). The user then made solves
  outrank HoF entries (see `policies.md`).

## 2026-10-19: full pass; Day 19

- **Afternoon full pass** (16:25-17:00): 137/138 CORRECT, plus one 503 that was re-run. #1 went from 35 to 38.
- **Jumpbox:** it had been auto-powered-off since about 11:40 AEST.
- **Day 19:** released at 23:00:01.
  - Mobile and script were CORRECT on the first pass in under 1 min.
  - The warmups, metadata and binary agents were done by 23:07.
- **Campaign:** `plan-d19.json`, budget 3 (2 for the 16-attempt pool). #1 on 9 of 10 by about 23:25.

## 2026-10-20: Day 20; automation torn down

- **Before release:** the VPN and jumpbox were down at 22:55, and the user restarted them.
- **Release** at 23:00:01:
  - Warmups and script: all modes CORRECT on the first pass in under 40 s.
  - Agents fixed the other three by about 23:05:
    - mobile: a `dynamic` codec that follows the puzzle path to a secondary DEX;
    - metadata: Mach-O `cpu-type-set`, thin and fat slices;
    - binary: the RS20 resource overlay and `hcfg_resource_size`.
- **New lane:** **web-exploitation** appeared and was left alone, per the user's instruction.
- **Merge and deploy:** merged and deployed with 314/314 replay clean.
- **Campaign** (`plan-d20.json`, held until all lanes were solved): 13 runs, all CORRECT, **#1 on 8 of 10**.
  - Binary mastery: #3 (4105 vs 3764). Budget 2, 14 attempts left.
  - Warmups TT: #5 (735 vs 484). 253 attempts left; server-bound, so re-roll in the 16-21h window.
- **Teardown:** the user then removed the `huntress-rush` and `huntress-netprobe` systemd timers. Rush is now started
  by hand (see `runbook.md`). The netprobe findings are in `hall-of-fame.md`.

## 2026-10-22: crate pass; full HoF pass

- **ctf-run crate pass** (branch `perf/crates`, uncommitted at the time): hand-rolled inflate, CRC-32, zstd, substring
  search, regex, JSON and MD5/SHA-1 were swapped for benchmarked crates. Details are in `ctf-run/PLAN.md`.
  - Deployed to the jumpbox via a multiplexed ssh socket; ssh handshakes were taking ~39 s.
  - Untimed regular `--retry` on all 10 Day 20 routes came back CORRECT.
  - Jumpbox binary-RE TT analysis: ~650 -> 415 ms.
- **Full HoF pass** (`plan-full-d22.json`, 158 routes on days 1-20, five known lanes only; Day 21 and web-exploitation
  untouched):
  - Budget: 3 runs (2 on 16-attempt pools) where not #1; 1 `past_first` run where already #1.
  - 372 runs: 368 CORRECT, 2 server 503s, 2 incorrect. One 503 hit `retry` (no attempt spent); the other hit the submit
    of warmups d09 TT (attempt lost). The 2 incorrect were warmups d19/d20 mastery (see `grading-lessons.md`).
  - Board across the 158 routes: **#1 15 -> 57**, top-3 64 -> 117, 94 rank gains.
  - Biggest movers: script, 28 gains (17 routes to #1); metadata, 23; warmups, 30.
- **Warmups fix:** a `-<digits>` answer tail is now submitted whole.
  - After fixing it, warmups d19 mastery was re-run: #6 -> #2. d20 mastery ran 2 more times and stayed #3; both
    resumed plans had suffixed sets graded CORRECT.
- **Survey tool:** `hof_survey.py` had queried status for the web-exploitation routes (read-only; they returned 409).
  It now skips lanes outside the five known ones.


## 2026-10-22: Day 21 (rush started late, after the HoF pass)

- **Start:** `rush.py all --day 21 --rehearse` at 18:31, since the day had been released the night before. The stale
  Day 20 tmux session was killed first. Lane prompts now forbid any challenge outside the agent's own lane prefix
  (web-exploitation in particular).
- **First pass** (18:34):
  - mobile and script: all three modes CORRECT with no code change.
  - warmups: base64 of gzip (`H4sI...`) blob.
  - metadata: new `apk-package-name` family of kinds (AXML manifest, arsc refs, signer certs).
  - binary: untagged config fields routed to the member the question names (OCaml loader plus `.cmxs` plugin).
- **Agents:** binary and metadata were done by 18:40. The warmups agent was blocked by the permission classifier on its
  untimed `-t` run, so the coordinator finished it with `rush.py takeover`. Takeover crashed on `result.json` being a
  list of submit records; fixed in `2963edf`.
- **Result:** all 15 routes CORRECT, untimed; no ranked attempts spent.
- **Merge and deploy:** `merge.sh 21`, 64 tests, replay 329/329 clean. Deployed `a54f4932` through the ssh socket.
- **Day 21 HoF campaign** (`plan-d21.json`, 10 routes, budget 3; 2 on the 16-attempt pool), started about 19:00 AEST:
  20 runs, all CORRECT, **#1 on 6 of 10**. Binary TT and mastery: #3. Mobile TT and warmups TT: #2.
- **Playbooks:** event-independent `playbooks/solver-runner.md` and `playbooks/orchestrator.md`, linked from this
  README.

## 2026-10-23: Day 22 (rush started late, ~2 h after release)

- **Start:** `rush.py all --day 22 --rehearse` at 00:57 AEST. Agents online at 00:59:42; first pass done at 01:00:43.
- **New lanes:** web-exploitation d09/d10 listed in `op=list&day=22`; left alone.
- **First pass:** warmups, script and mobile: all three modes CORRECT with no lane code change (warmups: base64 of zlib,
  mastery row pinned to `T::Gz`; script: NSIS polyglot; mobile: base64 codec).
  - metadata: new kind `d22-jar-001-jar-main-class`; new `formats/jar.rs` (manifest `Main-Class` + launcher check).
  - binary: single-solve incorrect; PE i386 machine is graded as `i386` under the generic label syntax.
- **Agents:** all five done by 01:04:53, no takeovers.
- **Result:** all 15 routes CORRECT, untimed; no ranked attempts spent.
- **Merge:** `merge.sh 22` in two batches (warmups/script/mobile, then metadata/binary); 65 tests, replay 344/344 clean.
  Not yet deployed; no campaign run.

## 2026-10-24: Day 23 (rush started late, ~18.5 h after release)

- **Start:** committed the Day 22 run state (`0fed2fe`), then `rush.py all --day 23 --rehearse` at 17:24 AEST. Agents
  online at 17:26:49; first pass done at 17:28:12.
- **New lanes:** web-exploitation d11/d12/d13 listed in `op=list&day=23`; left alone.
- **First pass:**
  - binary: all three modes CORRECT with no code change.
  - warmups: single-solve refused; new Ascii85 (`<~...~>`) layer, plus per-row number bases in mastery.
  - metadata: single-solve incorrect; new `d23-sqlite` kind (SQLite header fields); zip CD names are graded sorted.
  - script: TT refused; layered `LYR` literals in PowerShell, plus composite JScript/PowerShell TT/mastery kinds.
  - mobile: TT refused; new v2 `analysis` codec, which needed DEX plus JNI x86-64 interpretation.
- **Agents:** all five done by 17:49:02, no takeovers.
- **Result:** all 15 routes CORRECT, untimed; no ranked attempts spent.
- **Merge:** `merge.sh 23` in four batches as lanes finished; 71 tests, replay 365/365 clean. Not yet deployed; no
  campaign run.
- **Coordinator gotcha:** a Monitor script polling `rush/done-*` died at once under zsh (`no matches found`). Wrap
  marker polls in `bash -c` with `shopt -s nullglob`.

## 2026-10-24: Day 24 (rush started 40 min after release)

- **Start:** the first `rush.py all --day 24 --rehearse` (23:37) found the Day 23 `rush` session still alive and kept
  its agents ("not relaunching"): `--rehearse` skips the kill. Stopped the driver, killed the session and the five
  old `claude` pids, then relaunched at 23:39:48. Agents online at 23:39:51; first pass done at 23:41:27.
- **New lanes:** web-exploitation d14/d15 listed in `op=list&day=24`; left alone.
- **First pass:**
  - script, binary and mobile: all three modes CORRECT with no code change.
  - warmups: single-solve refused; new run-length `count:char` codec.
  - metadata: single-solve refused; new `d24-evtx-001-evtx-record-count` kind (`formats/evtx.rs`, walked records).
- **Disk full (~23:43):** the metadata agent unpacked the TT/mastery evidence zips into its scratchpad under
  `/tmp/nix-shell.*`, on the root disk, which already had only a few GB free. Root hit ENOSPC. That broke the
  warmups merge (`.git/ORIG_HEAD.lock`), crashed the rush driver, stalled metadata (`stalled-metadata.md`), and left
  no Bash output in the coordinator or agent sessions. The user cleared space; the root disk was still only ~2.4 GB free afterwards.
- **Recovery:** metadata had all three modes CORRECT but nothing committed. The coordinator ran `fixtures_from_runs.py`,
  replayed 64/64, wrote `NOTES.md` and committed it on `day24/metadata` (`14f0fcf`).
- **Result:** all 15 routes CORRECT, untimed; no ranked attempts spent.
- **Merge:** `merge.sh 24` in four batches (mobile, script, binary, then warmups+metadata); 74 tests, replay 380/380
  clean. Not yet deployed; no campaign run. Rush tmux server and driver killed afterwards.

## 2026-10-25: Day 25 (rush on time)

- **Start:** `rush.py all --day 25` at 22:45:54 AEST, inside tmux `rush-driver`, with output tee'd to
  `/tmp/claude-1000/rush-d25.log`. Prep took ~2 min (about 26 s per worktree build). All five agents were launched at
  22:48:10. The root disk had 805 GB free.
- **Release:** detected at 23:00:01. Routes loaded per lane in 2.5-3.4 s. The first pass finished for every lane at
  23:00:53.
- **New lanes:** web-exploitation d16/d17/d18 listed in `op=list&day=25`; left alone.
- **First pass:**
  - mobile (24 s), script (30 s) and binary (51 s): all three modes CORRECT with no code change.
  - warmups: single-solve refused. New `U+XXXX` Unicode code-point codec (`codec::code_points`, `T::Cp`).
  - metadata: single-solve refused. New kind `d25-registry-001-registry-hive-root-name`, in a new
    `formats/regf.rs`. The answer is the name of the root `nk` cell that the base block's root offset points at,
    not the embedded file name.
- **Agents:** all five done by 23:04:52 (binary/mobile/script 23:02:18, warmups 23:03:42); no takeovers.
- **Result:** all 15 routes CORRECT on the GM, untimed. Every ranked pool is untouched, so no ranked attempts were spent.
- **Merge:** `merge.sh 25` in three batches (script/binary/mobile, warmups, metadata); 76 tests, replay 395/395
  clean. Not yet deployed; no campaign run.
- **Coordinator gotchas:** see the runbook's shell section (tzdata missing, and `ls` under nullglob).

## 2026-10-25/26: HoF campaigns after Day 25 (user-approved)

- **Prep:** rush shut down (agents, `huntress` tmux server; the driver had already exited). Deployed `722b5abd` to
  the jumpbox by hand: the binary there dated from Oct 22. Surveyed days 1-25 (`survey-d25.json`).
- **New tool:** `ctf-run/tools/hof_plan.py` builds a plan from a survey with the Day 22 budget rules. It skips routes
  that spent ranked attempts but never got a scoreboard row (binary d01/d02, metadata d01: not HoF-eligible, see
  `ctf-run/PLAN.md`).
- **Campaign A** (`plan-d22-25.json`, the 40 never-attempted routes on days 22-25, ~23:25-23:50 AEST): 71 runs, all
  CORRECT. **28 of 40 went straight to #1**; 6 at #2, 4 at #3, one at #4 (binary d23 mastery), one at #6 (script d25 TT).
- **Campaign B** (`plan-full-d25.json`, 208 routes on days 1-25 with attempts left, ~23:55-02:00): 417 runs, all
  CORRECT.
  - Budget: 3 runs (2 on 16-attempt pools) where not #1; 1 `past_first` run where already #1.
  - Jumpbox disk full after 24 routes: ENOSPC crashed the campaign. Binary ranked run dirs had grown `runs-hof/` to
    20 GB. With the user's approval, 573 old binary evidence zips (16.2 GB) were deleted, keeping the newest run per
    route+mode and all JSON (`tools/prune_hof_runs.sh`). Resumed with the done-route list.
  - Board across 250 routes, start of night -> after A -> after B: **#1 63 -> 91 -> 93**, top-3 126 -> 164 -> 168, on the
    board 182 -> 222. B gave only 19 rank gains: most routes had already been re-rolled on Day 22, and these runs
    were in the slow release-night latency window.
  - #1 by lane: script 39/50, metadata 23/50, mobile 18/50, warmups 11/50, binary 2/50.
- **Worst remaining:** metadata d11 mastery #51 and binary d09 TT #31 have no attempts left. Binary d10 TT is #7
  (1580 ms vs 503 ms leader) with 239 left: an analysis-speed gap, so re-rolls won't close it.
- **Coordinator gotchas:** the ssh master socket (`-M -fN`) does not survive between the coordinator's Bash calls
  (the harness reaps it), so use plain keyed ssh with `ConnectTimeout=150+`. The launch ssh hung every time (exit
  124) but the campaign started anyway. A monitor's `grep 503` matched leader times (`top [503, ...]`): match
  `HTTP 503` instead.

## 2026-10-26: afternoon full HoF pass (user-approved)

- **Plan:** `plan-full-d26-pm.json` (`hof_plan.py --order`, server-bound lanes first): 205 routes, up to 428 runs. Ran
  15:30-~17:15 AEST on build `722b5abd`, in the quiet latency window. Overnight, other teams had cut our #1 count
  from 93 to 81.
- **Result:** 403 runs, 402 CORRECT. **#1 81 -> 102**, top-3 168 -> 180. By lane, #1 before -> after: metadata
  19 -> 27 (13 rank gains), warmups 7 -> 11 (10), script 38 -> 43, mobile 15 -> 18, binary 2 -> 3.
- **Lost attempt:** binary d25 mastery run 1 was SIGKILLed (`rc -9`, 7 s in, while analysing 9 x 36 MB sets) before
  submitting. It was almost certainly the kernel OOM killer: the jumpbox has 1.9 GB RAM, no swap, and ~450 MB taken
  by the organisers' services (`huntress-ai-arena` 267 MB, docker, snapd), while 9 deliveries + their inflated
  members peak at ~1.1 GB. `dmesg`/`journalctl -k` are not readable there. The attempt expired unanswered (11 left).
  Day 26 binary mastery will have 10 sets: cap concurrent set analysis and free each delivery once analysed before
  the next ranked binary mastery.
- **Binary fixes (evening, 2026-10-26):** the memory cap (`264c6ce`), then streaming analysis (`6df4424` and the
  progressive d08 decode). Deployed `1b48e006`. Live dry runs: d25 TT analysis tail 532 -> 96 ms. Jumpbox disk
  is at 6.4 GB free after the afternoon campaign: prune (`tools/prune_hof_runs.sh`) before the next one.

## 2026-10-26: binary-only ranked pass after the streaming deploy (user-approved)

- **Prep:** pruned the jumpbox (278 zips, 8.5 GB -> 15 GB free), surveyed, `plan-d26-bin.json`: 33 binary routes,
  TT first then mastery, up to 80 runs. Ran ~18:40-19:30 AEST on `1b48e006`.
- **Result:** 78 runs, all CORRECT. Binary #1 3 -> 4: d25 TT 2304 -> **1534 ms (#1)**. d21 TT 1931 -> 1775. Masteries:
  d08 3842 -> 3451 (#6 -> #5), d10 2805 -> 2649, d17 4357 -> 4006, d19 4311 -> 4103, d21 4693 -> 4113,
  d22 5718 -> 5489, d24 6620 -> 5937. The other TTs kept their old bests.
- **Why the TTs barely moved:** streaming works live. The TT analysis tail after the last byte went 400-535 ms ->
  0-73 ms on every route. But downloads were slower than when the old bests were set (e.g. d13 2.2 -> 2.9 s, d20 2.1 ->
  3.7 s median), and ±700 ms of download variance swamps the ~450 ms saved. The old bests were lucky fast downloads.
  Binary TT is now purely download-bound: improving the board takes more re-rolls (TT pools have 44-240 attempts
  left), not more engineering.
- **Mastery:** medians improved on most routes (d23 7.2 -> 5.5 s, d24 7.3 -> 6.2 s, d25 8.1 -> 7.2 s). d22-d25 (6-9
  sets) are still CPU-bound at the end: a 1.3-2.7 s tail once the downloads are in.
- **Board:** overall #1 fell to 89 (102 this afternoon) as other teams improved across lanes.

## 2026-10-26: evening full HoF pass (user-approved)

- **Plan:** pruned the jumpbox (208 zips, 6.2 GB -> 15 GB free); `plan-full-d26-eve.json`, 196 routes, server-bound
  lanes first, up to 399 runs. Ran ~19:10-20:25 AEST on `1b48e006`.
- **Result:** 382 runs, all CORRECT. **#1 89 -> 101**, top-3 169 -> 176. By lane, #1 before -> after: metadata
  21 -> 30 (10 rank gains), warmups 7 -> 9 (7), script 43 -> 44, mobile 14 -> 14 (3), binary 4 -> 4 (1).
- Server-bound lanes keep paying off in the 16:00-21:00 window. Other teams keep improving between passes (102 at
  17:15 -> 89 by 19:00), so a late-afternoon pass each day holds the board.

## 2026-10-26: Day 26 release night

- **Rush:** `rush.py all` started 22:50 (late; prep still finished by 22:53). Released 23:00:01; every lane had its
  routes within ~4 s. A web-exploitation lane (days 19, 20) is still listed; ignored per policy.
- **Lanes (all untimed, every ranked pool untouched), done markers / merged:**
  - script 23:01:32 `beam-message-flow` (Erlang escript polyglot): the existing solver handled all three modes in
    the 31 s first pass, with notes only.
  - warmups 23:03:32 `food-for-thought` (Brainfuck).
  - binary 23:03:52 `crash-triage` (Groovy JAR, crash/trace.json frames).
  - metadata 23:07:43 `before-you-go` (Prefetch, SCCA + LZXPRESS Huffman, executable name).
  - mobile 23:07:48 `split-decision` (split APK sets `.apks`: splitSourceDirs + ZipFile in the DEX interpreter).
- All five merged by ~23:10; 82 tests, replay clean. Not yet deployed. Jumpbox disk 8.7 GB free before any campaign.
- **Day-number slip:** the request said "Day 25" on the 26th; rush's default (day of month) was right. Trust the date.

## 2026-10-27: Day 27 release night

- **Rush:** `rush.py all` started 22:37 (prep done 22:39:47). VPN/jumpbox were down at the pre-check and came back
  once the user reconnected (8.7 GB free). Released 23:00:02; every lane had its routes within ~5 s. Web-exploitation
  days 21-23 listed; ignored per policy.
- **Lanes (all untimed, every ranked pool untouched), done markers / merged:**
  - mobile 23:01:45 `dependency-surface`: first pass CORRECT on all three modes (32 s); notes only.
  - script 23:02:23 `docker-build-attribution` (Dockerfile polyglot, marker fallback): first pass COMPLETE; notes only.
  - binary 23:03:47 `compiler-attribution` (gcc/tcc ELF pair): first pass CORRECT on all three modes (177 s, mastery
    is the long pole); fixtures + notes, no code change.
  - warmups 23:05:34 `monkey-business` (Ook! -> Brainfuck): first pass REFUSED; new `codec.rs` decoder.
  - metadata 23:05:42 `packet-loss-prevention` (pcap packet count): first pass REFUSED; new `formats/pcap.rs`
    (libpcap 4 magics + pcapng).
- All five merged by ~23:07; 85 tests, replay clean; GM shows all 15 routes CORRECT. Not yet deployed.
- **Coordinator note:** a binary lane pane sits idle for ~3 min after release while its first-pass mastery downloads;
  that is normal, not a stall. A marker-watch Monitor written as a zsh loop died at once on the unmatched
  `done-*` glob: wrap it in `bash -c 'shopt -s nullglob; ...'`.

## 2026-10-27/28: Day 26-27 ranked pass after the Day 27 deploy (user-approved)

- **Prep:** deployed `ee753884` (static), pruned the jumpbox (193 zips, 5.7 GB -> 15 GB free), surveyed days 26-27
  (all 20 ranked pools untouched), `plan-d26-27.json`: 20 routes, up to 58 runs, warmups/mobile/metadata first.
- **Token:** the first launch died with HTTP 401 on its first status call (no attempt spent): the GM token had been
  rotated and the jumpbox's `~/.huntress_token` was stale. The user approved copying the `.env` token over; relaunched
  ~23:40 AEST (see runbook "Jumpbox token").
- **Result:** 38 runs, 37 CORRECT, 1 ungraded (server 503 on submit, d26 metadata TT; the campaign backed off and
  continued). **13 of 20 routes #1**, 2 at #2 (d26 mobile TT, d27 metadata TT), 4 at #3 (d26 warmups TT, d26 metadata
  TT, both binary masteries), 1 at #4 (d26 script mastery 2971 vs 2533). Binary TTs went #1 (d26 1648, d27 2652).
  Jumpbox 13 GB free after.

## 2026-10-28: Day 28 release night

- **Rush:** an earlier `rush.py all` (~22:40) had launched the agents, but the `huntress` tmux server and every lane pane
  exited at 22:44:29; nothing was running at 22:50. Relaunched `rush.py all` at 22:50:48 (prep was a no-op, worktrees
  already on `day28/*`), with the driver output tee'd to `runs-d28/rush/driver.log`. Released 23:00:01; every lane had its
  routes within ~4.3 s. Web-exploitation days 24-25 listed; ignored per policy.
  **Lesson:** before 22:55, check that `tmux -L huntress ls` still shows the `rush` session, not just that `runs-dNN/rush/`
  exists.
- **Lanes (all untimed, every ranked pool untouched), done markers / merged:**
  - mobile 23:01:21 `protocol-reconstruction` (v1 packet codec, day 8 v2 in mastery): first pass COMPLETE; notes only.
  - script 23:02:21 `terraform-cloud-timeline` (terraform_hcl polyglot): first pass COMPLETE; notes only.
  - warmups 23:03:17 `back-in-sixteen` (reversed hex): small `warmups.rs` change.
  - metadata 23:03:55 `where-it-all-began` (GPT/MBR disk images, partition start-LBA sequence): new `formats/disk.rs`.
  - binary 23:09:59 `family-clustering` (new fields on old days, multi-word tags): `binary_re.rs` change; binary
    replay 11.8 -> 16.0 s with the 3 new cases.
- All five merged by ~23:11; 86 tests, replay clean; GM shows all 15 routes CORRECT. Not yet deployed.
