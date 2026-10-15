# ctf-run - agent guide

`ctf-run` solves Huntress CTF Game Master challenges **per lane** (warmups, mobile-android, metadata-forensics,
binary-reverse-engineering, script-analysis) with a low-latency runner built for the CourseStack proxy
(`http://10.0.0.219/huntress-ctf-proxy`, reached from the jumpbox `HuntressCTF@10.1.181.90`). Rust, std-only apart from `ring` (fast SHA-256; also rustls's provider);
`--features tls` adds the external https API. Background: `PLAN.md` (measurements, design, progress log) and
`../utils/LANE_PATTERNS.md` (how each lane evolves day to day).

## Rules

- **Every run sends `start`**, including `--dry` (a dry run still starts or reconnects). `run::check_route` refuses:
  `--dry` on timed-by-default routes, `--timed` where unsupported, one-shot routes (ranked pool 1) and routes missing
  from the cache unless `--one-shot-ok`. Keep `ctf-run routes --refresh` current. Never test new code on a timed route:
  use `replay`, single-solve, or solved routes (a solved attempt's material is re-fetched via `status`, so a dry run on
  a solved route is a free live test).
- **Lanes never guess.** Return `Err` (with what was seen) when unsure; the runner then refuses to submit (`--partial`
  overrides). Self-validate wherever the lane allows (warmups template, mobile `ANS-` pattern, HCFG CRC, descriptor parse).
- **Fixtures are ground truth.** Every lane must keep `ctf-run replay fixtures/<lane>` at 100 % before a live run. Saved
  run directories (`runs/...`) are replayable as-is.
- Timed or ranked attempts are spent only with the user's explicit approval.
- Token only via `HUNTRESS_GM_TOKEN`, never printed, never sent over plain http. The proxy needs none.

## Layout

| Path | Role |
|---|---|
| `src/main.rs` | CLI: `run` (default), `replay`, `oracle`, `status`, `routes`, `lanes`, `bench` |
| `src/run.rs` | the live pipeline: guard -> preflight (free) -> start -> parallel fetch+analyse -> answer (+ early submits) -> submit -> persist (free) |
| `src/replay.rs` | offline: run a lane over saved runs / fixture manifests, diff against expected answers |
| `src/oracle.rs` | untimed subset submits + bisection to find wrong answers |
| `src/gm.rs` | Game Master protocol, response normalisation (bundle preference), route cache |
| `src/net/` | keep-alive HTTP/1.1 (gzip for JSON, identity for downloads), connection pool, fetch workers |
| `src/lanes/` | one file per lane + `probe` (answers nothing; pipeline testing) |
| `src/formats/` | zip (AES/ZipCrypto/ZIP64), inflate, elf, pe, macho, jvm, dotnet, cfb/MSI, lnk, media (jpeg/png/pdf), hcfg, x86 (tiny), bytes |
| `src/crypto/` | aes (AES-NI + portable), sha1/hmac/pbkdf2, sha256, md5, crc32 |
| `src/codec.rs` | text codecs (base64/32, hex, numbers, percent, entities, escapes, rot/atbash/reverse, English scoring) |
| `src/script/` | script-analysis toolkit: descriptor model + rubric, label-driven decode, cmd.exe model, JScript evaluator, families |
| `src/evidence.rs` | nested-zip registry (`a.zip -> member` labels) |
| `src/rx.rs` | Python-`re`-compatible backtracking regex (backrefs, lookaround) so reference patterns port verbatim; `re!()` caches compiled patterns |
| `tools/` | `deploy.sh` (build+test on the jumpbox), `fixtures_<lane>.py` (manifest generators), `fixtures_from_runs.py` (graded run dirs -> fixtures), `oracle_groups.py` (family-grouped untimed oracle), `campaign.py`, `phase0_probe.py` |
| `fixtures/<lane>/*.json` | replay manifests (point at the original files under `../utils/solver_py_scripts`) |

## The lane contract (`src/lanes/mod.rs`)

`analyse(delivery)` runs once per download on its fetch worker (parse everything the lane needs; overlaps other
downloads). `answer(question)` runs per question against all analyses: match the question-id suffix (after `setN.`)
against the lane's kind table. `delivery_of` lets questions be answered (and `--early-submit`ted) as soon as their
delivery lands. Analyses are downcast with `done.get::<T>(i)` / `done.all::<T>()`.

## New day: the recipe

1. `ctf-run routes --refresh day-NN` - note `run_for_time_supported`, pools, deadlines (ONE-SHOT flags).
2. `ctf-run <lane>-day-NN-<slug> --dry` (single-solve is always untimed). Errors name the unknown kind and dump FACTS.
3. Extend the lane (usually one of):
   - **warmups**: a new encoding -> `codec.rs` decoder + a `T` variant / `fixed_codec` row in `lanes/warmups.rs`
     (mastery row N = day N's encoding; unknown rows are searched and learned across sets).
   - **mobile**: a new `codec` name -> a branch in `lanes/mobile.rs::decode` (the bounded search usually finds it already).
   - **metadata**: a new `<format>-<field>` kind -> a `HANDLERS` row in `lanes/metadata.rs` (+ a parser in `formats/`);
     learn the graded format (casing, `0x` padding, braces) from single-solve.
   - **binary-RE**: usually nothing (`analyse_specimen` is generic). A new container -> `formats/` + `Container`; a new
     field -> a row in `answer_spec`. Formats follow the question's `answer_format.kind`.
   - **script-analysis**: a new language with a descriptor literal is found by `families::analyse_literal`'s fallbacks;
     add an explicit pattern there if not. A new `Codec_` representation/transform -> `script/labelled.rs`.
4. Submit single-solve (`ctf-run <id>`). On `incorrect`: `ctf-run oracle runs/<id>/<dir>` localises the wrong fields;
   for thousands of answers use `python3 -I tools/oracle_groups.py <run dir>` first (one partial submit per question
   family, then per field - minutes instead of a long blind bisection). Single-solve resubmits are free, so test an
   alternative format with one curl submit before encoding it. Dual-pool TT/mastery: plain `-t`/`-m` is an untimed
   regular attempt (flag + unlock, same oracle).
5. Regression: `ctf-run replay fixtures/<lane> runs/<new run>`; turn graded runs into fixtures with
   `python3 -I tools/fixtures_from_runs.py <run dirs> [--slug S]` (warmups need `--slug`); notes go in
   `../utils/solver_py_scripts/day-NN-<lane>/<slug>/NOTES.md`, day summary + next-day predictions in `../utils/LANE_PATTERNS.md`.
6. Timed: dual-pool routes use `--timed` (Hall of Fame) and `--retry` for the next variant; timed-by-default routes are
   timed on plain `start` (one-shot needs `--one-shot-ok`). Self-validating lanes may add `--early-submit`.

## Testing and performance

- `cargo test` (unit tests use real bytes; `testdata/`), `ctf-run replay fixtures/<lane>` (all lanes: 231 cases),
  `cargo build --features tls` must stay green.
  The jumpbox runs the static musl binary from `nix build .#ctf-run` (repo flake), shipped by `tools/deploy.sh`; never
  compile on the burstable t3.small before ranked runs (`--remote-build` only if the flake is unavailable).
- Hall of Fame: `/api/speed-scoreboard?mode=...&challenge=<base id>&sort=elapsed_ms` (team "help"); elapsed ~= submit
  sent + server grading, so every ms before the submit counts. `tools/run_legs.py <run dirs>` splits a run into legs.
  Ranked runs on small-delivery lanes (warmups/mobile/metadata): add `--hedge` (duplicate downloads beat the ranked
  straggler tail); never on binary-RE. Downloads always use the capability link (op=artifact is slower). Locally (external API via `HUNTRESS_GM_URL=https://...`) always build
  with `--release --features tls`: a plain `cargo build --release` overwrites the binary with one that refuses https.
  Keep a day's live runs in `runs-dNN/` (`-o runs-dNN`); prune pure `--dry` duplicates once graded runs exist. Build/run on the jumpbox with `tools/deploy.sh` (stable Rust 1.99 there).
- `CTF_PROFILE=1` prints per-specimen / per-file analysis timings and replay analyse/answer splits.
- Measured: the server timer starts at the end of `start` processing (start latency is free); the timed window is
  download + analyse + submit. Downloads are the long pole (server think-time, 2-8 s tail); prefer bundles, fetch all
  in parallel on warm connections, analyse on the fetch workers, nothing between the last answer and `submit`.

## Day release automation (`tools/rush/`)

- `install-timer.sh` installs a systemd user timer (12:45 UTC = 22:45 AEST, event days) running `rush.py all`:
  **prep** (persistent lane worktrees `../../wt/<lane>` reset to `master` on branch `dayNN/<lane>`, symlinked
  `ctf-run/fixtures` + `.env`, every target pre-built) -> **launch** (tmux `-L huntress`, session `rush`, one Claude per
  lane in auto mode with `--add-dir <repo>`, primed by `prompts/prime.md`) -> **watch** (polls `op=list&day=N` from
  22:58:30, merges the day into the route cache, then per lane in parallel the **first pass**: single-solve, untimed
  regular time trial + mastery, grouped oracle on `incorrect`; writes `runs-dNN/rush/brief-<lane>.md` and sends that
  lane's agent "read the brief" the moment its own pass ends).
- Everything automated runs with `CTF_NO_RANKED=1` (ctf-run refuses `--timed`, `--one-shot-ok` and starts on
  timed-by-default routes). Ranked runs, deploys and merges are the coordinator's: `tools/rush/merge.sh DAY [--deploy]`
  merges lanes whose agent touched `runs-dNN/rush/done-<lane>`, then gates on `cargo test` + the full replay.
- Commit to `master` before 22:45 - lanes branch from the committed HEAD. Follow: `journalctl --user -fu huntress-rush`,
  `tmux -L huntress attach -t rush`, `rush.py status --day N`. Rehearse on a released day with
  `rush.py watch --day N --rehearse [--lanes x] [--no-agents]` (solved routes only reconnect; submits say already_solved).
