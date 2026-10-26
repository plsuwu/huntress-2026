# ctf-run rewrite plan (v2, 2026-10-15)

A from-scratch reimplementation in pure, extensible Rust. The previous `ctf-run` (local and jumpbox copies) is
discarded. Primary target: the CourseStack proxy `http://10.0.0.219/huntress-ctf-proxy`, run from the jumpbox
`HuntressCTF@10.1.181.90`. Solvers are **per lane**, built from shared format/codec/script modules, so a new day is
"add a question kind, a container reader or a language front-end", never "write a new challenge solver".
Background on what changes day to day: `../utils/LANE_PATTERNS.md`.

Principles: hand-written format parsers; hot primitives (inflate, CRC-32, zstd, substring search, regex, JSON, MD5/SHA-1)
come from crates where a benchmark showed they beat the hand-rolled code (2026-10-22, see the log), `--features tls` for
the external https API only; every lane is a full native port of its proven Python reference, gated by replaying the
saved fixtures; nothing ever submits a guess.

## 1. What the proxy costs (measured from the jumpbox, 2026-10-15)

| Request | Time to first byte | Notes |
|---|---|---|
| TCP connect | ~2 ms | connection setup is irrelevant on this link |
| `status` (small) | 0.35-0.7 s | same with 6 in parallel; keep-alive works (HTTP/1.1, Vercel behind Cloudflare) |
| `start` reconnect, SA d13 single (485 KB JSON) | 0.45-0.55 s | gzip: 485 KB -> 15 KB, TTFB unchanged on this LAN |
| `status` SA d13 mastery (2.8 MB JSON) | ~1.0 s | gzip: 2.8 MB -> 102 KB (28x), saves ~0.15 s here, far more off-LAN |
| small artifact / capability download | p50 0.3-0.4 s, p90 ~0.9 s | **tail: ~10-20 % take 2.3-7.8 s**, any size, any concurrency |
| 32 MB binary-RE zip | 2.7-8.9 s TTFB, then ~0.6 s transfer | proxy appears to buffer the whole body; 5 in parallel = 9.5 s wall |
| `op=list` | 8-24 s | never on the timed path |

- Proxy honours `Accept-Encoding: gzip` (and `br`); responses are then chunked.
- 30 s idle before a request costs only ~0.1-0.2 s - cold starts are not the tail.
- Hedging probe (40 trials, duplicate GET after 0.8 s): inconclusive (only 4 hedges fired, mild tail that sample).
- Unmeasured so far: a **fresh** `start` (older off-LAN notes: 4-6 s), a real `submit` (~3-4 s off-LAN), the download
  shape of timed vs regular starts, and partial-submit behaviour. Phase 0 measures them.

### Phase 0 results (2026-10-15, `tools/phase0_probe.py`, raw data in `fixtures/raw/phase0/`)

| Leg | mobile d13 mastery (regular, 5 sets) | warmups d13 mastery (**timed**, 63 sets) |
|---|---|---|
| fresh `retry` TTFB | **6.5 s** | **4.0 s** |
| downloads | 5 per-set APKs (`downloads[]` + `artifacts[]`), 0.38-1.93 s, all in parallel | **one top-level `download`** (bundle zip, 63 members), 1.6 s |
| partial submit | `accepted`, `remaining: 4`, 1.9 s | `accepted`, `remaining: 32`, 1.6 s (on a timed attempt) |
| final submit | `CORRECT`, 3.5 s | `CORRECT`, 2.5 s (Hall of Fame eligible) |

- The fresh start is the single biggest leg (4-6.5 s) and cannot be overlapped with anything we control.
- Download shape varies by attempt: an earlier warmups timed mastery used 63 per-question `downloads`, this one a single
  bundle. Normalise: top-level `download` | `downloads[]` | `artifacts[]{download, filename, sha256, kind?}`.
- `op=artifact&challenge=<id>&delivery=0` returns the same bundle; `delivery=1` -> HTTP 400 (0-based, bundle = 0).
- Correct partial submits are accepted mid-timer on timed attempts, so per-set early submits are safe for answers the
  lane has self-validated (an `incorrect` partial was not tested and stays unsafe).
- The server gzips everything it sends with `Accept-Encoding: gzip`, including `application/octet-stream` downloads.
- Proxy download links come back as `/api/huntress/game-master/download/<cap>`; `?op=download&capability=<cap>` works.
- Submit latency (1.6-3.5 s) suggests the final `CORRECT` includes CourseStack scoring; partial submits are cheaper.

**On the proxy, bytes on the wire are not the bottleneck; server think-time and its tail are.** The networking goals are
therefore: fewest sequential round trips, maximum overlap after `start`, fewest requests exposed to the tail.

1. **Fewest tail-exposed requests**: normalise the start response's download shape and prefer a bundle when one is
   offered (warmups timed mastery delivered 63 per-question files; its regular attempt advertises one `kind:"bundle"`
   zip). Never fetch the same delivery twice.
2. **Overlap after `start`**: all downloads at once on pre-opened keep-alive connections; each delivery is analysed the
   moment it lands; nothing (disk writes, logging, pretty-printing) between the last answer and `submit`.
3. **Free time before `start`** (the timer starts at `start`): route preflight via `status`, connection warm-up, solver
   self-check against the lane's fixtures.
4. **gzip** on every request (inflate is already needed for zip); a no-op where the server does not compress.
5. Experimental, off by default: `--hedge MS` for idempotent GETs; `--early-submit` (per-set partial submits) only if
   Phase 0 shows partial submits are safe on timed attempts.
6. Not building: brotli, HTTP/2, streaming JSON parse of the start body.

Jumpbox constraint: 2 vCPUs (Xeon 8259CL, AES-NI/AVX2), 1.9 GB RAM (~1.3 GB free), Rust 1.99 stable. A 5-set
binary-RE mastery is 5 x 32 MB AES zips expanding past 100 MB each, so evidence is decrypted/inflated **one member at a
time, on demand**, and the code stays stable-Rust compatible (local toolchain is nightly).

## 2. Architecture

```
src/
  main.rs            CLI: run | replay | oracle | facts | bench
  run.rs             pipeline: preflight -> start -> fetch+analyse -> answer -> submit -> persist (after submit)
  replay.rs          run a lane over saved fixtures, diff against the graded answers
  oracle.rs          untimed subset submits to localise wrong fields (the `accepted`/`remaining` oracle)
  net/  http.rs (keep-alive HTTP/1.1, gzip, chunked, per-request timing; tls feature)
        pool.rs (pre-opened connections, one kept hot for submit)  fetch.rs (parallel scheduler, completion order)
  gm.rs              protocol: proxy vs external dialect, start/retry/practice/submit bodies, route preflight
                     (run_for_time_supported, pools, deadline, set count), response-shape normalisation, one-shot guard
  json.rs            parser, writer, pretty-printer
  evidence.rs        deliveries -> lazily opened nested zips (stored/deflate/WinZip-AES `infected`), members addressed by
                     set / delivery / path; question routing (`setN.` prefix, `delivery N:`, `a.zip -> member` labels)
  crypto/            aes (AES-NI via std::arch + portable fallback), sha1/hmac/pbkdf2 (HMAC midstates), sha256, md5, crc32
  formats/           inflate, zip, elf, pe (+ VS_VERSION_INFO), macho, jvm (class constant pool / jar), dotnet
                     (metadata tables, #GUID/#Strings), png, jpeg, pdf, cfb (+ MSI string pool / Property), lnk
  codec/             text codecs shared by every lane: base64/base64url/base32/hex/binary/decimal/octal/percent/html
                     entities/backslash escapes/rot-n/atbash/reverse/xor/split; strict detectors + English/template scoring
  script/            the script-analysis toolkit (see section 3)
  lanes/  mod.rs (registry by id prefix)  warmups.rs  mobile.rs  metadata.rs  binary_re.rs  script_analysis.rs
```

### Lane contract: analyse once, answer many

Python solvers parse each specimen once and answer many questions from it (~10 per binary-RE specimen, ~11 per
script file, up to 4565 answers per mastery), and some answers depend on the mode or set. So:

```rust
pub struct Lane {
    pub prefix: &'static str,                                  // "binary-reverse-engineering-day-", ...
    pub analyse: fn(&Specimen, &RunCtx) -> Result<Analysis, Facts>,  // once per specimen, in the fetch pipeline
    pub kinds: &'static [Kind],                                // most specific first
}
pub struct Kind {
    pub name: &'static str,
    pub matches: fn(&Question) -> bool,                        // id suffix / answer_format test
    pub answer: fn(&Question, &Analysis, &RunCtx) -> Option<String>,
}
pub struct RunCtx { pub day: u8, pub mode: Mode, pub set: Option<u16> }
```

- No matching kind, or `None` from `answer` -> FACTS dump (everything the parsers can see) and **no submit**.
- Self-validation wherever the lane allows: warmups template, mobile `^ANS-[0-9A-F]{16}$`, HCFG CRC32, descriptor `v=`.
- Day/mode-specific grading quirks are data tables (binary-RE `format` casing per specimen; batch-expansion labels per mode).

### Lane notes

| Lane | Analysis | Shared modules used |
|---|---|---|
| warmups | whole-blob layer peeling + `NN<TAB>fragment` rows (whitespace-faithful, fixed row->codec map, dash padding) | codec |
| mobile | APK (zip) -> `assets/huntress_puzzle.json` -> codec by name, generic search fallback | evidence, json, codec |
| metadata | per-file FACTS map (hashes, magic, sizes, SOF/IHDR/PDF/EOCD/CD names/PE/.NET/MSI/LNK fields) | formats, crypto |
| binary-RE | per-specimen summary: container (ELF/PE/Mach-O/JAR), arch/machine/class/entry/symtab/sections, CRC-validated HCFG (raw, `.hcfg`, base64 `SENGR`, class constant) + offsets | formats, crypto |
| script-analysis | per-file family dispatch -> decoded descriptor + structural facts -> field answers | script/, codec, evidence |

## 3. Script-analysis: full native port

The Python reference is ~4.5k lines (days 3-13; day-13 `stager_relationships.py` chains day-7 -> 6 -> 5 -> 4 -> 3)
and is cumulative: every day's question set contains all previous days'. Most of it is reusable machinery rather than
per-day logic, so it becomes a toolkit:

```
script/
  descriptor.rs   pipe descriptors (v=1/v=2/v=3), nested hex / dotted-hex / base64 / hex-of-base64 literal decoding
  pipeline.rs     the common decoder IR: representation (decimal_space, hex_compact, percent_hex, base64_chunks, ...)
                  + byte transforms (identity, xor, rolling/chained xor, repeating-key xor, add/sub mod 256, reverse,
                  passes) - every language front-end lowers its Decode/recover body to this IR
  expr.rs         tiny integer/byte expression evaluator shared by front-ends (JS loop bodies, $(( )), VBS Chr/Xor, ...)
  lex.rs          shared lexing helpers: quoted strings per language, comments, line continuations, number literals
  taxonomy.rs     role_of / attack_of / stage_count / steps tables (language technique T1059.00x), batch-expansion rule
  lang/
    powershell.rs jscript.rs batch.rs (cmd.exe model: %/!/FOR/CALL expansion) vbscript.rs
    python.rs     whitelisted subset parser + evaluator (the py_static.py construct list; nothing executed)
    bash.rs       (+ advanced bash forms)   node.rs
    polyglot.rs   descriptor-literal families: php, lua, mof, ts, au3, applescript, hta/wsf/sct hosts - one literal
                  scanner table; the `HUNTRESS_ANALYSIS_V1:` marker as a language-independent fallback
```

Port order follows the cumulative question set, each step gated by replay of that day's graded fixtures before moving on:
d3 (PS + JScript + Batch) -> d4 VBS -> d5 Python -> d6 Bash -> d7 Node + advanced Bash -> d8 polyglot/host descriptors ->
d9-d13 polyglot literal families. The Python reference wins any disagreement (fix the port).

## 4. Phases

**Phase 0 - ground truth (first; mostly measurement)**
- Live protocol measurement on the jumpbox (small Python probe, untimed where possible):
  - one untimed regular `retry` on `mobile-android-day-13-keystore-misuse/mastery` (regular pool 8, 5 sets, trivial
    solver): time a **fresh** `start`, the downloads, a **partial** submit (set 1 only -> expect `accepted`/`remaining`)
    and the final submit; record the response shapes;
  - one **timed** `warmups-day-13/mastery` attempt (ranked pool 256, 80 s deadline): does a timed start offer a bundle,
    per-question files, or both? (solved with the existing Python runner, so it also counts as a normal attempt);
  - `op=artifact&delivery=N` behaviour on that attempt.
- Fixture normalisation moved to the start of each lane's port (Phase 3/4): layouts differ per lane and many saved
  runs lack `start.json` or expected answers. Each lane gets `tools/fixtures_<lane>.py`, which writes manifests
  `fixtures/<lane>/<case>.json` = {start (path or synthesised questions), deliveries (paths, in order), expected answers
  (saved graded answers, else the Python reference's output), graded: correct|solver}. Manifests point at the original
  files - nothing large is copied.
- Replace `src/` wholesale; rewrite `AGENTS.md`/`README.md` skeletons (the old docs describe the discarded design).

**Phase 1 - core**: `json`, `net/*`, `gm`, `run`, `replay`, `oracle`, CLI. Validated on the jumpbox with single-solve
reconnects and untimed regular attempts only, and compared against curl timings from section 1.

**Phase 2 - crypto, formats, evidence, codec**, each with unit tests on real fixture bytes; `ctf-run bench` for
AES/inflate/hash throughput on a real 32 MB binary-RE zip (local and jumpbox).

**Phase 3 - native lanes**: warmups -> mobile -> metadata -> binary-RE, each gated by 100 % replay agreement with its
graded fixtures, then one live untimed run per lane.

**Phase 4 - script-analysis** per section 3, gated per day by replay.

**Phase 5 - docs**: AGENTS.md "new day" recipe per lane (dry run -> FACTS -> add kind/reader/front-end -> oracle ->
replay -> timed), module map, testing rules.

## 5. Safety rules
- Never test new code against a timed or one-shot route; timed attempts are only spent deliberately (Phase 0 list).
- Read-only ops (`status`, `artifact`, `download`) and `start` reconnects are free to use for latency tests.
- The one-shot guard refuses `run_for_time_supported:false` routes with ranked pool 1 unless `--one-shot-ok` is given.
- Token from env only (`HUNTRESS_GM_TOKEN`), never printed, never sent over plain http; the proxy needs none.

## 6. Progress log

- 2026-10-15 - Phase 0 done (live measurements above). Phase 1 done: `net/` (keep-alive, gzip for JSON only -
  downloads go identity: gzip of AES zips was larger and cost ~1 s of decode on 32 MB), `gm.rs` (shape
  normalisation, bundle preference, solved-attempt material via `status`), `run.rs` (fail-closed route guard incl.
  `--dry`, pipelined fetch+analyse, early submit banked by `accepted_ids`, UTC clock stamps vs `started_at`),
  `replay.rs`, `oracle.rs`, `routes` cache. Phase 2 started: crc32, sha1/hmac/pbkdf2, AES (AES-NI + portable),
  inflate (table-driven), zip (AES/ZipCrypto/ZIP64); `bench` = 194 MB/s on the jumpbox (a 32 MB binary-RE set in 0.41 s).
- Warmups lane native: `src/lanes/warmups.rs` + `src/codec.rs`; `tools/fixtures_warmups.py` -> 32 manifests,
  `ctf-run replay fixtures/warmups` 32/32 clean (days 5-13, incl. the burned day-12 mastery and the Phase 0 bundle).
  Live: single-solve dry run correct. Timed `warmups-day-13/mastery --retry --timed` (approved: up to 50 such runs):
  CORRECT, start->result 1.97 s client-side (Python runner ~9 s); 1 bundle download 315 ms, solve 1 ms, submit 774 ms.
- **The server timer excludes `start`**: start sent -797 ms / received +86 ms relative to `started_at`, submit received
  +1177 ms. `started_at` is stamped at the end of the server's start processing, so the timed window is
  download + solve + submit only; the 4-6 s fresh-start cost is free. Optimise the post-start legs.
- Mobile lane native (`src/lanes/mobile.rs`): known codecs + bounded transform search + `java.util.Random`
  keystreams for seeded puzzles (Day 14 guess); `tools/fixtures_mobile.py` -> 68/68 clean; live single+mastery dry
  runs match the graded answers.
- Metadata lane native (`src/lanes/metadata.rs`, `src/evidence.rs`, formats: media/pe/elf/dotnet/cfb(MSI)/lnk,
  crypto md5/sha256): suffix handler table + LNK field kinds (Day 14 guess) + FACTS on unknown kinds;
  `tools/fixtures_metadata.py` -> 28/28 clean (incl. the 90-answer d13 mastery); live dry runs answer everything.
- Binary-RE lane native (`src/lanes/binary_re.rs`, formats: hcfg/jvm/macho/x86): one generic specimen analysis for
  every dNN; container casing/ELF-class format follow the question's `answer_format.kind` (exact_text -> `elf`,
  integer -> `64`) instead of a per-day table; `tools/fixtures_binary_re.py` -> 30/30 clean; live 130/130 + 390/390.
  Perf: SWAR memchr + hashing big specimens on a side thread: d08 (67 MB) specimen 262 -> 156 ms locally.
  Next perf step if needed: stream inflate -> SHA-256 on another thread (hash is now the floor, ~0.4 s on the jumpbox).
- Script-analysis lane native (`src/lanes/script_analysis.rs`, `src/script/`, `src/rx.rs`): Python-`re`-compatible
  regex engine so reference patterns port verbatim; label-driven decoding (Codec_ labels were truthful in every
  fixture) validated by descriptor parse, replacing the Python/Bash/VBScript/Node interpreters; faithful ports of the
  cmd.exe model and JScript decoder evaluator; polyglot literal finder with marker/any-string fallbacks (Day 14 guess).
  Batch `expansion` grading follows eras (days <= 11: Day 3 rule; day >= 12: Day 13 rule; day-13 single = old labels).
  `tools/fixtures_script.py` -> 35/35 clean (days 4-13, every mode, incl. the 4565-answer d13 mastery); live 913/913
  and 4565/4565 dry runs. Perf: fast PBKDF2 (2 compressions/iteration) + parallel member decryption: 163 -> 50 ms/set.
- All lanes: 193/193 fixtures clean, 0 warnings, tls build green. Docs: AGENTS.md (rules, layout, new-day recipe), README.md.
- Remaining ideas: streaming inflate -> SHA-256 for binary-RE (hash is the floor); hedged downloads (inconclusive);
  `--early-submit` live trial on a dual-pool mastery (correct partials are banked mid-timer, verified in Phase 0).
- Fix (advisor review): the live loop marked early-answered questions done even when they failed, so an answer that
  needed another delivery (warmups row transform learned from another set, per-file mastery) was never retried.
  Now `run::Answerer` (shared by live runs and replay) keeps failures open and retries them in `finish`; replay feeds
  deliveries in reverse order through the same path. Regression test `early_failures_are_retried_once_everything_landed`.
  193/193 fixtures still clean.
- Verification status: real submits through the new client so far: warmups (1 timed mastery, CORRECT). Mobile,
  metadata, binary-RE and script-analysis are verified offline (fixtures) and by live dry runs only; their submit paths
  (incl. the ~250 KB 4565-answer script mastery body) are not yet exercised live.
- 2026-10-15 - Timing campaign (`tools/campaign.py`, jumpbox, arm `direct+nopre+gap10` = external API, no preflight,
  10 s gaps): 358 timed runs, 352 CORRECT. Failures: chunk-norris mastery (delivery-order bug, fixed; re-run 5/5 CORRECT,
  best submit_sent 235 ms), one `503 temporarily_unavailable`, four day-13 masteries `503 retry_rotation_invalid`
  (server refused another variant). Direct beat the proxy (warmups d13 mastery p50 ~300 vs ~1000 ms); `--warm` hurt.
  Small routes: timer ~200-650 ms, dominated by the download's server TTFB (150-250 ms); link RTT ~8 ms
  (t3.small us-east-1, Cloudflare IAD), 5x31 MB lands in ~1 s. Binary-RE mastery (3.4-7.3 s) is CPU-bound:
  ~5.5 s per set analysis with 5 sets on 2 burstable vCPUs.
- 2026-10-15 - Day 14: all five single-solves CORRECT. Warmups/mobile/script/binary-RE needed no change (pre-built
  guesses held: rot search, PRNG keystreams, MSBuild literal fallback, Mach-O). Metadata: `lnk-target-path-windows`
  -> `lnk_target` (LinkInfo base+suffix, IDList shell-item path, literal environment target must all agree
  case-insensitively; disagreement/relative-only = Err; no source = `none`); `formats/lnk.rs` gained `idlist_path`
  (My Computer root, volume, file entries with 0xBEEF0004 long names validated against the short name; an unvalidated name drops the IDList source instead of vetoing). Warmups row 14 pinned to Rot(19) (graded).
  Day-14 time_trial/mastery routes are ONE-SHOT (ranked pool 1, 30 s): never add them to campaign.py's VERIFIED.
  `tools/fixtures_from_runs.py` turns graded run dirs into fixtures; 198/198 clean.
- Latency experiments (2026-10-15, `tools/campaign.py`, warmups-day-13/mastery, metric submit_sent - started_at):
  direct API (bearer) beat the proxy (best 221 vs 370 ms; noisy); a `--warm 6` status burst before `start` made every
  run slower (downloads 726-1074 ms) - extra requests just before `start` hurt; skipping the preflight status and
  leaving ~10-20 s idle before `start` gave the tightest results (median ~234 ms). Default campaign arm:
  `direct+nopre+gap10`. The jumpbox t3.small gets CPU-throttled under sustained load (bench 414 -> 830 ms, ~12 % steal):
  binary-RE masteries (3.2-5 s) and day-8/9 script masteries (~1.5 s) are CPU-bound - a non-burstable compute instance in
  us-east-1 (direct API) would help those; small-download lanes are server-bound (~180-250 ms best per route).
- Campaign: 368 timed runs over days 4-13 (results: jumpbox `~/ctf-run-v2/campaign/*.jsonl`, runs in `runs-campaign/`):
  362 CORRECT; failures = 1 metadata bug (two mastery deliveries with the same file name: `delivery N:` label must win -
  fixed), 1 server 503 `temporarily_unavailable`, 4 x `retry_rotation_invalid` (server refuses a second timed `retry`
  right after a completed timed day-13 mastery; no attempt consumed). Experiments also found a warmups variant whose
  final row is padded with `-<digits>` (fixed; test `digit_padded_final_row`).
- 2026-10-15 - Day 14 one-shots (user-approved, run on the jumpbox via direct API, `-t/-m --one-shot-ok`, preflight
  on): all 10 CORRECT first time. submit_sent vs started_at: warmups TT 594 / M 243 ms (68 sets), script TT 597 /
  M 884 ms (1942 answers), binary-RE TT 1401 / M 3234 ms (3x32 MB, ~2 s analysis per set on 2 vCPUs), mobile TT 202
  / M 805 ms, metadata TT 229 / M 232 ms. Every LNK (TT + 2 mastery sets) had agreeing LinkInfo/IDList targets
  (`C:\Windows\System32\cmd.exe`); the "impossible destination" never surfaced as a disagreement. One-shot routes:
  the server's timer can start before our `start` response lands (warmups TT: +467 ms). Fixtures: 208/208 clean.
- 2026-10-15 - Day 15 (Rust-only, no Python reference first; all TT/mastery routes dual-pool): all 15 routes CORRECT via
  untimed regular attempts (no timed runs yet). Changes: warmups ROT47 (`codec::rot47`, row 15 pinned; a letter transform
  that yields the template now beats structural peeling - ROT47 output `\42` was being eaten as an octal escape);
  metadata `ooxml-<field>` kinds (`formats/ooxml.rs`: `.rels` relationships, core/app props) - list answers are graded
  **sorted by value** (file order rId9,rId10 was `incorrect`); script `host-<lang>-` ids all route to the literal family
  (new `host-perl-`); binary-RE d15 ships `-stripped` + `-symbolized` builds: every non-.json specimen is analysed,
  `analysis_<tag>_<field>` routes to the specimen named `...-<tag>`, `analysis_machine` with `integer` format = raw
  e_machine (62), and `artifact_sha256_N` follows **central-directory order** (symbolized before stripped; name sort was
  wrong). Script mastery: `for %%G in (..) do call set "cfg=%%cfg%%%%h%%G%%"` is graded `for_indirect` on mastery
  (Day 3 rule said call_double; d14 single-solve had accepted call_double) - v13 rule now mode-dependent. New
  `tools/oracle_groups.py`: family-grouped partial submits (found the 1 wrong field in ~70 requests vs slow bisection).
  Fixtures: 222/222 clean.
- 2026-10-15/16 - Day 15 follow-up. Perf (binary-RE mastery is the only CPU-bound route on the 2-vCPU jumpbox): SHA-256
  now via `ring` (BoringSSL asm; already rustls's provider) - jumpbox 182 -> 362 MB/s (OpenSSL 412; a hand-unrolled
  Rust compress only reached ~200, `-C target-cpu=native` ~219); the .NET `BSJB` scan runs only on PE/unknown containers;
  `has_mangled` stops at the first hit. Per-set analysis on the jumpbox 3.7-3.9 s -> 1.2-2.7 s (5 sets in parallel);
  `bench` now reports SHA-256 throughput. Bug fixed: `routes --refresh` cached a 503 body and wiped the jumpbox route
  cache - op=list is validated before caching. Ranked (user-approved, jumpbox, direct API, `--retry --timed
  --no-preflight`, 10 s gaps): all five d15 time trials CORRECT, Hall of Fame eligible; submit received vs started_at:
  mobile 1061, warmups 1619, metadata 2138, script 1806, binary-RE 2613 ms (download TTFB dominated). Ranked runs become
  `fixtures/<lane>/dNN-<mode>_ranked.json` (`fixtures_from_runs.py` tags `run_for_time` runs). 227/227 clean.
- Day 15 ranked masteries (user-approved, same arm): all five CORRECT, Hall of Fame eligible; submit received vs
  started_at: mobile 1482, metadata 1501, warmups 1836, binary-RE 5747 (last delivery +4.2 s, analysis ~0 after it),
  script 8587 ms (2.9 MB start, delivery +3.7 s, then a 4.7 s server-side submit for 5130 answers - the next lever is
  the submit leg: try `--early-submit` per-set partials overlapping the download, or check whether a gzip request body
  is accepted). Fixtures: `dNN-mastery_ranked.json` for the four small lanes; 231/231 clean.
- Submit-leg experiments (2026-10-16, jumpbox, untimed regular variants of script d15 mastery, 5130 answers / 302 KB;
  `tools/submit_experiment.py`): **gzip request bodies are rejected** (`400 invalid_json` - the server ignores
  Content-Encoding). **Parallel partial submits are safe but not faster**: no lost updates (`remaining` steps down
  serially, the last one returns CORRECT), but wall time full 1832/2486/2704/2826 ms vs 5 parallel chunks
  2106/2652/2995 ms vs 10 chunks 2402/3337 ms - the server appears to serialise one attempt's submits, and the ranked
  run's 4.7 s was server variance. Keep the single full submit; `--early-submit` only helps when per-set deliveries
  land at different times (multi-delivery masteries), never for bundles. Cost: 11 regular script-mastery variants.
- 2026-10-16 - Hall of Fame mechanics + script mastery to #1. `GET /api/speed-scoreboard?mode=<time_trial|mastery>&
  view=rankings&limit=N&offset=0&sort=elapsed_ms&direction=asc&challenge=<base id>` (bearer) lists `team_name`, `rank`,
  `elapsed_ms` (we are team "help"). Calibrated against our clocks: **elapsed_ms ~= submit sent + server grading** (the
  CourseStack scoring round trip after grading is not counted); grading scales with answer count (~0.35 ms/answer,
  more under load). Script mastery was #9 (6705 ms): 1.05 s server lag + 1.5 s download + **1.14 s our analysis** +
  2.9 s grading in a busy moment. Analysis was WinZip-AES key derivation (580 members x 1000-round PBKDF2) on a
  t3.small whose 2 vCPUs are hyperthreads of ONE core (2 threads ~1.15x). Fixes: multi-buffer AVX2 PBKDF2
  (`crypto/pbkdf2x8.rs`, 8 chains/stream) + password verified once per archive (later members derive only the AES key
  blocks) + ring PBKDF2 elsewhere -> unpack 465 -> ~110 ms, script mastery analysis 1178 -> ~275 ms on the jumpbox.
  Three ranked re-runs: 3860/5325/3398 ms client-side; scoreboard **#1 at 2028 ms** (prev #1 2137).
  Ruled out: gzip request bodies (400), parallel partial submits (server serialises), VPS tuning (no root: sudo needs a
  password; tcp_slow_start_after_idle=1, cubic; no cpufreq). The t3.small is burstable - stop compiling on it:
  `flake.nix` packages.ctf-run is a static x86_64-musl build (+tls, mimalloc - musl malloc was 40-70 % slower; inflate
  now copies matches inline - musl memcpy per match was ~25 % slower), and `tools/deploy.sh` ships that binary
  (`--remote-build` keeps the old path). A Nix glibc binary cannot run there (glibc 2.44 vs Ubuntu 2.39).
- 2026-10-16 - Mobile mastery / warmups TT (server-bound routes). Our client time is ~0 on both; elapsed = server
  start lag (normally ~70 ms, 280-365 ms in bad moments) + slowest download + ~150-250 ms grading. Probes on untimed
  regular variants (`tools/download_probe.py`): the capability link is consistently faster than `op=artifact&delivery=N`
  (50-500 ms) - never use op=artifact for downloads; regular downloads are tight (~150-280 ms TTFB, no stragglers), but
  **ranked downloads are slower and straggly** (mobile: one run all 628-791 ms, one with a single 671 ms straggler;
  warmups TT 342-657 ms vs ~200-300 regular). New `--hedge`: each delivery fetched twice on detached threads (the loser
  never delays the submit), first good response wins - neutral when quiet (A/B 321 vs 325 ms), aimed at the single-
  straggler case; use it for small-delivery lanes (warmups/mobile/metadata), never binary-RE (doubles 32 MB downloads).
  It cannot help when the server is slow for every request (warmups' single file stayed 469-610 ms hedged).
  Ranked re-rolls (6 each lane, all CORRECT): warmups TT #9 -> #3 (660 ms; #1 559), mobile mastery #13 -> #4
  (621 ms; #1 391). Remaining gap = server luck; more re-rolls are a lottery, not engineering.
- 2026-10-16 - Hall of Fame campaign over every route with ranked attempts left (`tools/hof_survey.py` ->
  `runs-hof/plan-*.json` -> `tools/hof_campaign.py` on the jumpbox; logs/surveys in `runs-hof/`). 435 ranked runs over
  111 routes, **430 CORRECT, 0 wrong answers**; the 5 others were server-side (2x `409 legacy_attempt_pinned` on metadata
  d01 TT/mastery - no new ranked variant there; 3x HTTP 503). Scoreboard: #1 on 13 -> 27 routes, top-3 39 -> 54, on a
  board 95 -> 126. Day 14 routes were re-published as dual-pool after the day (one-shot attempts were never HoF-eligible,
  so the boards were empty) - we took #1 on all 10, then tightened them (run_for_time:true + retry is required; plain
  start/retry gives nothing ranked). Day 12 is still one-shot/exhausted (pool 1, no ranked pool published). Days 1-4
  had no fixtures: hand-tested first with free `--dry` reconnects on solved routes (new check_route exception); binary
  d01/d02 use a different question scheme (`slot0_*`, `c_sha256`) that the lane does not support. Old-day leaders
  (~200-330 ms) were set when the server answered faster; today the same routes download in 300-600 ms, so our
  day 1-4 entries sit mid-table despite ~0 ms client time. Lessons: the server 503s under load (the driver now backs
  off and retries; keep its own queries resilient), and `pkill -f` on a pattern that appears in the ssh command
  line kills the ssh session itself - use `ps -eo args | grep "^[p]attern"`.
- 2026-10-16 - Day 16, first rush release (`tools/rush/`). First pass at 23:00:25-23:01:02: script + mobile CORRECT on
  all three modes with no code change; warmups (Morse of hex), metadata (`email-message-id` on `.eml`, new
  `formats/email.rs`) and binary-RE (WebAssembly pair, new `formats/wasm.rs`) fixed by their lane agents by ~23:06.
  The binary agent's plain untimed `-t` start was blocked by its permission classifier - the coordinator ran its
  untimed TT/mastery (both CORRECT first submit) and finished the lane. All five merged; 246/246 replay clean.
  Ranked (user-approved, jumpbox, `runs-hof/plan-d16-{a,b,c}.json` -> `campaign-d16-*.log`): 40 runs, **40 CORRECT**,
  deployed in waves (never deploy while `hof_campaign.py` runs: deploy.sh removes the binary before scp). Board at
  23:22: **#1 on 6/10** (script TT 1032 / mastery 2005, mobile TT 465 / mastery 467, warmups mastery 821, metadata
  mastery 805); #2 warmups TT (811 vs 614, server-bound), metadata TT (709 vs 629, server-bound), binary TT (1900 vs
  1899); #3 binary mastery (4886 vs 4465). Binary is the one engineering lever: analysis ~600 ms per TT run and
  ~2.6 s per mastery on the jumpbox, after the download. Ranked attempts left: TT 24-252, binary mastery 13.
- 2026-10-17 - Day 17 rush. Release seen at 23:00:09: the status probe treated 409 `start_required` as not released,
  costing 9 s (fixed). New gate: TT/mastery `start` -> 409 `prerequisite_required` until single-solve is CORRECT
  (binary mastery also waits for TT), so the concurrent first pass only scored script's single-solve. Lane fixes:
  mobile `indexed` permutation codec (+2 min), warmups Bacon-of-hex (+4), metadata WAV `pcm_s16le` via new
  `formats/audio.rs` (+7), script AHK `:=` literal hardening (+13), binary-RE 12-fields-per-day roll-up incl.
  `hcfg_storage` labels and MinGW-PE `.text$`/`.rdata$` symbol answers (+36, ~200 free partial submits). All 15 routes
  CORRECT untimed; 269/269 replay clean. `oracle_groups.py` now groups `dNN_` ids by day.
  Ranked (user held ranked runs until every lane was merged, except an early mobile/warmups wave; jumpbox,
  `runs-hof/plan-d17-{a,b,c,d}.json` -> `campaign-d17-*.log`, `d` = one full `past_first` re-run): 29 runs,
  **29 CORRECT**. Board 23:45: **#1 on 9/10** - warmups TT 534 / mastery 566, mobile TT 247 / mastery 248, metadata
  TT 638 / mastery 638, script TT 728, binary TT 1980 (next 3696) / mastery 4750 (next 93 s). #2 script mastery
  1667 vs 1645 (5695 answers; 6 re-rolls stuck at 1667 - client-side parse/solve time is the lever, not luck).
  Ranked run dirs (small lanes) in `runs-d17/ranked/`.
- 2026-10-18 - Day 18 rush. Release seen 23:00:09. Warmups and binary-RE TT/mastery were ONE-SHOT (ranked pool 1, no
  regular pool, `hall_of_fame_eligible:false`). The coordinator fired all four from the jumpbox after each lane merged
  (`-t/-m --one-shot-ok`), and all were CORRECT first time. Lane fixes: warmups 3-rail fence (`codec::rail_fence_decode`),
  metadata MP4/MKV/AVI video geometry (`formats/video.rs`), script Apps Script `[..].join("")` literals plus day-18
  single `for_indirect`. Mobile and binary needed no code change. Mobile's agent was blocked by the classifier on
  untimed `-t/-m` (its `cd <wt> && ./target/...` form misses the allowedTools rule). The coordinator ran them, and
  `rush.py takeover` crashed on 409 `start_required` (fixed). All 15 routes CORRECT by 23:12; 280/280 replay clean.
  Ranked (`plan-d18-a.json` -> `campaign-d18-b.log`; `-a` died on the jumpbox's stale route cache, so run
  `ctf-run routes --refresh` there first): 7 runs, all CORRECT, **#1 on 6/6** dual-pool routes. Full pass
  (`plan-full-d18{,-b,-c}.json`, 139 eligible routes, budget 1, past_first): 139 CORRECT, 40 rank gains, board
  **#1 26 -> 36 of 152**, top-3 70 -> 80. **GM re-grade:** between 13:18Z and 13:42Z every TT/mastery `batch-*-expansion`
  switched to a pure-syntax rule (`batch::classify_expansion_syntax`, gated on `RunCtx.started_ms`; see
  `utils/solver_py_scripts/day-18-script-analysis/process-story/NOTES.md`). Before it was found, 6 ranked script runs
  (days 3-6) came back incorrect.
- 2026-10-19 - Full HoF pass before Day 19 (16:25-17:00 AEST; netprobe timer stopped for the run). The jumpbox had been
  powered off since ~11:40 AEST (auto shutdown on uptime). After the user restarted it, its binary hashed equal to a HEAD
  `nix build`, so no deploy was needed. Survey `runs-hof/survey-full-d19.{json,txt}` (days 1-18, 180 routes): 138
  eligible with attempts left. Binary d09 TT is exhausted. Binary d01/d02 and metadata d01 still have attempts but are not
  HoF-eligible. `plan-full-d19.json` (budget 1, past_first) -> `campaign-full-d19.log`: 137 CORRECT, 1 submit HTTP 503
  (warmups d05 mastery: the attempt was consumed and the campaign's 503 retry does not fire at budget 1; re-run in
  `plan-full-d19-b`, CORRECT). 40 time improvements, 21 rank gains; across the 138 routes **#1 35 -> 38**, top-3 78 -> 84.
- 2026-10-19 - Rush for Day 19. New `CTF_NO_RANKED=hof` mode (`run::check_no_hof`): only `--timed` is refused. The rush first pass
  is now chained: single-solve, then TT + mastery once single is CORRECT. One-shots run with `--one-shot-ok`, binary's
  mastery after its TT. Lane agents get the same rules. Rehearsed on solved Day 18 (warmups/binary one-shots, mobile dual-pool): all CORRECT via
  already_solved reconnects, same attempt ids, nothing spent. 284/284 replay clean.
- 2026-10-22 - Crate pass over hand-rolled primitives (branch `perf/crates`). Each swap was kept only if it
  benchmarked faster: perf/hyperfine on `replay fixtures/<lane>`, plus micro-benchmarks on the real day-20 payloads
  (scratch crate, not kept). Kept: `memchr` (`bytes::find`/`find_all`, http header scan), `crc32fast` (9x slice-by-8),
  `libdeflate-sys` (inflate 587 -> 1011 MB/s, vs zlib-rs 774 and miniz_oxide 566. flate2's zero-fill cost another 8 %),
  `zstd`/libzstd (3-6x ruzstd on the 3.7 MB script-mastery start body), `serde_json` with `arbitrary_precision` into
  `json::Value` (1.6x; identical `Value`s on all 1520 JSON files in the repo. Exponents are normalised to Python/JS
  form, `1e5` -> `1e+5`), `fancy-regex` behind `rx` (below), `md-5` (1.2-1.5x), `sha1` (5x with SHA-NI, 2x without),
  RustCrypto `pbkdf2` for single WinZip keys (3x ring with SHA-NI, 1.7x without), and mimalloc on glibc too (it was
  musl-only). Rejected after measuring: RustCrypto `aes`+`ctr` (9.9 vs 12.0 GB/s for ours), any PBKDF2 crate for the
  batch (`pbkdf2x8`, 2.2 ms per 80 blocks, vs RustCrypto 5.1/9.0 ms and ring 16 ms), sonic-rs (its serde path
  drops number text). Replay suites, before -> after (musl static, the deployed build): binary_re 8.81 -> 5.19 s,
  script 3.60 -> 3.34 s, metadata 428 -> 343 ms; warmups/mobile unchanged (noise). Fresh-process day-20 fixtures
  (musl): binary_re TT 206 -> 120 ms, mastery 293 -> 198 ms; metadata TT 20 -> 16 ms; script mastery 210 -> 200 ms;
  script TT neutral. `rx` lessons: compile cost matters because a live run is one process. Unicode `\w` makes regex
  NFAs ~10x costlier to build, so each pattern has an ASCII-mode build used for ASCII subjects (same results there) and
  a lazily built Unicode one. A global cache keeps `format!`ed patterns to one compile each. Without these two the
  swap was a 40 % *loss* on script fixtures. Python syntax is bridged by `rx::translate` (`$` -> `(?=\n?\z)` outside
  MULTILINE, `\Z`, `{,n}`, stray braces, `[`/`&`/`~` in classes, punctuation escapes as `\x{..}`). On this hybrid
  P/E-core desktop, compare `cpu_core/cycles` (perf stat) as well as wall time. `tools/bench.sh OLD NEW [lanes]`
  repeats the comparison.
  Deployed 2026-10-22 16:05 AEST (static musl `48c2542f`; the previous binary is kept as
  `target/release/ctf-run.prev-608adf3f` on the jumpbox). Live check there: untimed regular `--retry` on all 10 day-20
  TT/mastery routes, all CORRECT. Jumpbox analyse legs (`tools/run_legs.py`): binary TT 629-669 -> 415 ms, binary
  mastery 2083 ms (vs 1766-3639 before). Regular-pool downloads had a 4.5 s ttfb (fresh variants are generated on
  demand) against 0.2-0.4 s on ranked pools.
- 2026-10-22 - Full HoF pass on the new binary (see `../orchestrator/history.md`): 158 routes, #1 15 -> 57. Two
  warmups masteries failed on a `-<digits>`-suffixed answer, which `match_template` used to truncate. The suffix is now
  submitted whole (test `digit_suffixed_answer_is_submitted_whole`, real day-19 bytes), and ranked suffixed sets
  were confirmed CORRECT. Deployed `7f302241` (previous: `ctf-run.prev-48c2542f`, `ctf-run.prev-608adf3f`).
- 2026-10-26 - Binary-RE HoF gap analysis (161 ranked jumpbox runs, Oct 22-26; no code change). Board: 2/50 #1, most
  routes #2-#4 at 1.1-1.5x the leader. Per run: start ~50-300 ms, download, then the whole analysis, then submit.
  **The download is capped per connection at ~8-25 MB/s (never >30)** while 8-9 parallel connections reach 90-113 MB/s,
  on every day and time. So it is not t3 network credits, and not our TCP (`tcp_rmem` max 14 MB, RTT ~8 ms): the cap
  is upstream (Cloudflare/origin). A 36 MB TT zip takes ~2.1 s.
  **Analysis never overlaps the download:** the tail after the last delivery equals the last set's analysis, 400-535 ms
  per TT (d08+) and 0.6-2.1 s per mastery (sets contend for the 2 hyperthreads). Locally the d25 TT set (89.6 MB
  inflated) is 92 ms decrypt+inflate plus 33 ms SHA-256; the jumpbox is ~4x slower (Xeon 8259CL: AES/AVX2/AVX-512,
  no SHA-NI). Specimens differ per variant (config, XOR keys and hashes all vary), so caching answers is out; only
  a few shared libraries (e.g. `FSharp.Core.dll`) are constant.
  Levers, best first:
  1. HTTP `Range` split downloads, if the origin honours Range (untested: expired capabilities 404; needs a live
     untimed attempt). Could cut TT downloads ~4x; keep the split small for GM load.
  2. Stream decrypt/inflate/SHA-256 per member as bytes arrive (local headers carry sizes), so analysis hides
     under the download: -0.4..-0.5 s TT, -0.6..-2.1 s mastery.
  3. Mastery CPU: multi-buffer AVX-512 SHA-256 across members/sets, cached hashes for the constant libraries, and
     less thread oversubscription (9 sets x 2 workers + hash threads on 2 hyperthreads).
  Leader times of 113-224 ms on 32-95 MB routes (d10 mastery, d13 TT/mastery) are not reachable by any full download.
