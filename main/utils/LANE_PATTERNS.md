# Lane patterns & per-day "gimmick" predictions (as of Day 24, 2026-10-24)

Derived from every `utils/solver_py_scripts/day-*/**/NOTES.md` (days 2-13) plus the GM `op=list` route metadata.
Read this before starting a new day: most of the day's "gimmick" is predictable, and the real risk is usually the
route configuration (one-shot timed attempts), not the analysis.

## TL;DR

- **Three lanes are cumulative** (day N = day N-1 + one new piece): script-analysis, binary-RE, and warmups *mastery*.
  Metadata-forensics is one-new-kind-per-day (with occasional cumulative recap bundles); mobile is not cumulative at all.
- **The prompt's theme only changes the container, never the logic.** No prompt narrative (impostor PDFs, colliding
  installers, exploit chains, coroutine state machines, "which pairing is genuine", "find the lying report") has ever
  become a question. The work each day is "where does the same payload sit now?".
- **"Cumulative" does not mean "frozen"**: binary-RE day 8 added new questions to the old d01-d07 specimens, and the
  script-analysis batch-expansion labels were graded differently in single-solve vs time-trial/mastery on day 13.
- **Since Day 17, modes are gated**: TT/mastery `start` answers 409 `prerequisite_required` until single-solve is
  CORRECT (binary-RE: mastery also waits for TT). Solve single first; a concurrent start of all three wastes nothing
  but must be retried.
- **Check the route config first** (`op=list&all=true`): `run_for_time_supported`, `ranked_pool_size`,
  `regular_pool_size`, `ranked_deadline_ms`, `ranked_set_count`. It decides whether you get a free untimed attempt.

## Route configuration is the main risk

| `run_for_time_supported` | `start` behaviour | Consequence |
|---|---|---|
| `true` | plain `start` = regular untimed attempt; `run_for_time:true` = ranked/timed | free dry run + subset-submit oracle on TT/mastery |
| `false` | plain `start` **is** the timed attempt; sending `run_for_time:true` -> HTTP 409 `timed_intent_not_available` (nothing burned) | no dry run; pool may be 1 |

**All Day 14 TT/mastery routes (every lane) are `run_for_time_supported:false`, `ranked_pool_size:1`, `regular_pool_size:0`,
30 s deadline** - one shot per mode, no untimed TT/mastery oracle. `practice` after a burned attempt recovers the flag but
not Hall of Fame. Precedent: day 12 warmups mastery was burned by a row-parsing quirk (whitespace strip + dash padding),
not by the gimmick.

Workflow on one-shot days:
1. Solve **single_solve** first - subset submits there return `accepted`/`accepted_ids`/`remaining` (free per-field
   oracle; wrong fields cost nothing). Learn answer *formats* here (casing, `0x` padding, braces).
2. Dry-run the solver on the single-solve fixture **and** saved prior-day TT/mastery fixtures (same lane).
3. Make the runner tolerant of response shape (`download` vs `downloads[]`/`artifacts[]`, `setN.` prefixes,
   nested vs bare zips, AES `infected` zips) before the one-shot start.
4. Only then take the timed shot.

Self-validating lanes (safe to enumerate candidates even under a timer): **warmups** (decoded text must match the
"Nice work decoding this warmup! ... Game Master: <slug>" template), **mobile** (`^ANS-[0-9A-F]{16}$`), **binary-RE**
(HCFG header CRC32 must verify). Metadata-forensics and script-analysis have no self-check -> rely on the single-solve
oracle.

## Per-lane summary

### Script analysis - strictly cumulative
- Answer count grows monotonically: 87 (d2) -> 159 (d3) -> ... -> 695 (d9) -> 753 -> 811 -> 855 -> 913 (d13).
  Day N's question set contains day N-1's in full.
- Days 2-8 added interpreters per language (PS, JScript, Batch, VBS, Python carriers, Bash, Node). Since day 8 every
  file carries a `v=…|…` pipe descriptor (`descriptor_<id> = "..."`, encodings: hex, dotted-fragment hex, base64,
  hex-of-base64) and **answers are the descriptor's fields verbatim**.
- Since day 9 each day adds one `polyglot-<lang>-{benign-utility,downloader,launcher,staged-chain}` family
  (4 files x 11 answers) and occasionally 14 advanced-Bash answers. Days 11-13 needed only a one-line literal regex
  change (TS type annotation; AppleScript `property x : "..."`); day 12 needed nothing.
- **Since d23, TT/mastery use a new schema** (single-solve still the old one): files are named by hex id (`10c7ec.php`),
  question ids are `<family>-<hex>-<kind>`, literals may be layered (`HUNTRESS_ANALYSIS_V1:LYR-<recipe>-<payload>`,
  1-3 of hex/b32/b64/rev/frag), and new composite kinds (`-campaign-token`, `-indicator`, `-style`, `-url-host`,
  `-path-leaf`, PowerShell `-secondary`/`-tertiary`) are built from the descriptor fields. Details:
  `solver_py_scripts/day-23-script-analysis/game-resource-loader/NOTES.md`.
- Standing risk: `batch-*-expansion` labels (empirical rule in `day-13-script-analysis/stager_relationships.py::_classify_expansion`;
  single-solve grader disagreed with TT/mastery on 4 shapes).

### Binary RE - strictly cumulative
- d01..dNN specimen bank; older files are byte-identical day to day; ~10 questions per specimen (130 on d13).
- Each day adds `dNN` in a new language (Go, Rust, Java, Kotlin, Swift, Nim, Zig, D, Pascal, Crystal) but the config is
  always the **HCFG blob**: `HCFG | ver=01 | xor_key | len LE16 | crc32 LE32 | payload^key` (JSON payload), either raw
  (possibly in a `.hcfg` section) or as a base64 string constant starting `SENGR`. Raw `HCFG` hits can be decoys -
  validate by CRC. No language-specific parsing has ever been needed.
- Gotchas: hex fields `0x`-prefixed lowercase; `format` casing differs per specimen (d11 `ELF`, d12/d13 `elf`);
  `hcfg_offset` = file offset of the base64 text (or of the validated raw magic); stripped d01 variants exist.
- Bottleneck is bandwidth (31-32 MB AES zip per set because of the 67 MB d08 Swift ELF), not analysis.
- Since d19: appended overlays validated by SHA-256 (`PK19` packed, `RS20` resource; `formats/overlay.rs`), multi-member
  specimens (d21 loader + plugin, d22/d23 .NET assembly + support files) whose question text names the member to use,
  and `format` is an enum (`elf`, `pe`, `pe_cli`, ...). A PE i386 `machine` is `i386` unless the syntax names
  `IMAGE_FILE_MACHINE` (d03 keeps `x86`).

### Warmups - single = one layer, mastery = cumulative
- Single/TT: the whole blob in the day's encoding, decoding to the fixed template; answer = slug `word-word-word-NN`.
- Mastery: `NN<TAB>fragment` rows, raggedly sliced from the template; **row N uses day N's encoding**:
  01 base64, 02 rot13, 03 hex, 04 8-bit binary, 05 decimal, 06 octal, 07 %-hex, 08 `&#dec;`, 09 `"\uXXXX"`, 10 base32,
  11 base64url (unpadded), 12 reversed (+ `-` right padding to 13 chars), 13 atbash, 14 caesar +7, 15 ROT47,
  16 Morse of hex, 17 Bacon of hex, 19 hex of UTF-16LE + BOM, 20 hex of UTF-16BE + BOM (rows 19/20 carry a BOM on
  every row), 21 base64 of gzip, 22 base64 of zlib, 23 Ascii85 (`<~...~>`).
- Rows 04/05/06 (binary/decimal/octal) are **pinned by radix**: a digit-only fragment can parse as both decimal and
  octal, and the score-based reader picked octal on a tie (latent since d5, first hit on d23 mastery).
- Mastery set count = **5N - 2** (23 on d5 ... 63 on d13 ... 113 on d23).
- Parse rows whitespace-faithfully (leading spaces matter), use the fixed index->codec mapping first, enumerate only as fallback.
- **Submit exactly what the sentence says.** Since 2026-10-22, a set can end `...: word-word-word-NN-<digits>`. The whole
  string is the answer, despite the two-digit `answer_format`; truncating it was graded incorrect twice (`orchestrator/grading-lessons.md`).

### Metadata forensics - one new kind per day, sometimes a recap bundle
- Single-solve asks 1 question of the day's new kind. Kinds so far: jpeg SOF dims (`WxH`), png IHDR dims, pdf header
  version, zip EOCD total entries, zip CD names (compact JSON), PE COFF machine (`0x014c`), PE entry RVA (`0x%08x`),
  PE OriginalFilename (VS_VERSION_INFO), .NET MVID (lowercase bare GUID), MSI ProductCode (`{UPPER}` with braces),
  plus evidence-note filename/size/md5/sha1/sha256/magic16/utf8-bom and "synthesis order" (sha256-sorted JSON array).
  Later: email Message-ID (d16), wav codec (d17), ELF `e_machine` (`0x%04x`, d19), Mach-O cpu-type set (JSON
  `["0x0100000c"]`, d20), APK package (d21), JAR Main-Class (d22), SQLite header page size (d23).
- Each new kind gets a `-<format>-<field>` dispatcher in `lanes/metadata.rs` that runs before HANDLERS. Field names
  starting with a digit fall through to the hash handlers. Only the graded field is known; the others are guesses.
- **JSON lists are graded sorted** unless proven otherwise: ooxml rel targets (d15) and zip CD member names (d23 TT;
  every earlier fixture happened to be stored sorted) were both rejected in file order.
- TT/mastery: days 8, 9, 13 (regular) asked the whole cumulative bundle (12 -> 18 questions); days 10-12 (ranked) asked
  only the day's kind. Build the solver to handle either. `-filename` suffix collides between kinds - match carefully.

### Mobile Android - not cumulative
- Same tiny hand-built APK every day; answer = `assets/huntress_puzzle.json` decoded, `ANS-` + 16 uppercase hex. The
  Android story (PendingIntent, providers, WebView, cleartext, trust, keystore) is pure decoration.
- Codecs seen (by day): 4 xor, 5 split(parts/order), 6 hex, 7 caesar(key), 8 reverse, 9 base64, 10 split, 11 xor,
  12 hex, 13 base64, 14 caesar (printable-ASCII shift, `key`), 15 native (plain `data` + `libarena.so`), 17 indexed,
  19 reflection (plain `data`), 20 dynamic (`data` = path to `assets/payload.dex` holding the code), 21 reverse,
  22 base64. Since d15 mastery mixes earlier days' codecs (each APK has a `day` field). Per-APK keys differ - read
  them from the JSON. Mastery = N bare APKs (5 on d19-d21, 6 on d22, 7 on d23), ids `setN.answer`.
- **Since d23 TT/mastery, puzzle schema v2** (`huntress.mobile.puzzle.v2`, `codec:"analysis"`, no `data`): the answer is
  computed by the app's own `Release.code()` in `classes.dex`. ctf-run interprets it (`formats/dex.rs`, `dalvik.rs`, JNI
  leaves via `x64emu.rs`). v2 mastery rebuilds earlier days so that each day's token chain uses that day's mechanism
  (d15/d16 JNI natives, d20 in-memory secondary DEX + reflection, d23 brute-force loops). Single-solve was still v1.

## Day 15 outcome (2026-10-15)

Routes went back to **dual-pool** (`run_for_time_supported:true`, ranked pools 16-256, regular pools 16-128): free
untimed TT/mastery attempts + per-field oracle again. All 15 routes CORRECT via untimed regular attempts, solved directly in
ctf-run (no Python step). Details in `solver_py_scripts/day-15-*/*/NOTES.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | ROT47 ("Room Forty-Seven") | ROT47 output looks like `\NN` escapes - try letter/rotation transforms for a template match before structural peels; mastery row 15 = ROT47 |
| metadata | `ooxml-external-relationship-targets` (.docx) | JSON list **sorted by value**, not `.rels` file order |
| script | `polyglot-perl-*` + `host-perl-cgi-perl` | route every `host-` id to the literal family; mastery grades FOR-over-`h<N>` as `for_indirect` (single accepted `call_double`) |
| binary-RE | d15 = `-symbolized` + `-stripped` ELF pair | `artifact_sha256_N` in zip CD order; `analysis_<tag>_<field>` per specimen; integer `machine` = e_machine |
| mobile | `codec:"native"` (answer in `data`, also in `libarena.so`) | nothing; mastery now samples earlier days' codecs (`day` field per APK) |

## Day 16 outcome (2026-10-16)

Dual-pool again (ranked pools 16-256, `rft=true`). First rush release: script + mobile CORRECT on the automated first pass
(no code change); warmups/metadata/binary-RE fixed by their lane agents within ~6 min of release, all 15 routes CORRECT
untimed. Details in `solver_py_scripts/day-16-*/*/NOTES.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | Morse code of the sentence's **hex** digits ("Dot Com") | Morse -> hex -> text; mastery row 16 = Morse (of hex) |
| metadata | `email-message-id` on a `.eml` | graded **with angle brackets** as in the header; new `formats/email.rs` (other email fields are ungraded guesses) |
| script | `polyglot-ruby-*` + `host-ruby-rack-ruby` | uppercase `DESCRIPTOR_<HEX>` constant - found only via the `HUNTRESS_ANALYSIS_V1` marker fallback |
| binary-RE | `-compact` + `-direct` WebAssembly pair | `format`=`wasm`, `architecture`=`wasm32`; HCFG raw in a data segment; new `formats/wasm.rs` |
| mobile | `codec:"native"` again | nothing |

## Day 17 outcome (2026-10-17)

Dual-pool (ranked 16-256). Script passed the first pass with no code change. Mobile was done at +2 min, warmups at +4,
metadata at +7 and script at +13 (hardening only). Binary-RE took until +36 min: new fields on old specimens, and graded
labels that cost ~200 free partial submits. All 15 routes CORRECT untimed. Details in
`solver_py_scripts/day-17-*/*/NOTES.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **Bacon** a/b 5-bit groups of the sentence's **hex** digits ("Bacon Bits") | Bacon -> hex -> text (3rd day running of "<cipher> of hex"); mastery row 17 = Bacon |
| metadata | `media-primary-audio-codec-name` on a `.wav` | graded with the **ffprobe `codec_name`** spelling (`pcm_s16le`); new `formats/audio.rs` |
| script | `polyglot-auto_hotkey-*` (no `host-` file) | AHK v2 `descriptor_x := "..."` - `:=` was not in the literal regex (marker fallback saved it) |
| binary-RE | d17 = Ada/GNAT ELF, raw HCFG in `.rodata` | d17 itself fine; **every day now has 12 questions** - new fields on d01-d14 (marker, recover_code, C++ symbols, go/rust/coroutine runtime flags, JVM methods/access flags, `hcfg_storage`) |
| mobile | `codec:"indexed"` (`data` + `order` permutation, scatter) | one decode branch |

Graded labels that no synonym search finds: `hcfg_storage` = `base64` / `raw_hcfg_section` / `raw_const_array`; on a
MinGW PE the symbol answers keep the COFF section prefix (`.text$_ZNK...`, `.rdata$_ZTI...`); stripped d01's
`recover_code` = `none`.
Ranked (one campaign after the last merge): see `ctf-run/PLAN.md`.

## Day 18 script (lane agent, 2026-10-18)

- Script: `polyglot-google_apps_script-*` + `host-google-docs-sidebar-google-apps-script`. The literal is **split into
  a `[..].join("")` fragment array** (marker copy prefixed `FRAG-`/`B64-`). A lone fragment can decode to a
  truncated descriptor. Batch FOR-over-`h<N>` expansion is now `for_indirect` on single-solve too.
  Details: `solver_py_scripts/day-18-script-analysis/process-story/NOTES.md`.

## Day 19 outcome (2026-10-19)

Dual-pool, released 23:00. Mobile and script were CORRECT on the first pass with no code change; warmups, metadata
and binary were fixed by their agents by +7 min. All 15 routes CORRECT untimed.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | hex of **UTF-16LE with a BOM** ("Little Things") | decoded bytes failed `printable` (NULs); `codec::text_any` picks UTF-8 or UTF-16 by BOM; mastery row 19 = hex |
| metadata | `elf-e-machine-u16-hex` | `0x003e`, `0x%04x` lowercase; `elf_answer` reads the header only, byte order from EI_DATA |
| script | `polyglot-csharp_script-*` + `host-aspx-inline-csharp` | nothing; the `.aspx` host's escaped `\"` literal relied on the marker (pattern hardened on d20) |
| binary-RE | stripped ELF + **`PK19` packed overlay** | new `overlay_offset`/`overlay_size`; every old question reworded, `format` now an enum (`ELF` -> `elf`, .NET -> `pe_cli`) |
| mobile | `codec:"reflection"` | nothing (`data` is the plain answer) |

## Day 20 outcome (2026-10-20)

Dual-pool, released 23:00. Warmups and script were CORRECT on the first pass; the other three were fixed by about
+5 min. All 15 routes CORRECT untimed. Web-exploitation appeared as a new lane and was left alone.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | hex of **UTF-16BE with a BOM** ("Big Entrance") | nothing (d19's `text_any`); row 20 pinned to `T::Hex` |
| metadata | `macho-cpu-type-set` | compact JSON of `0x%08x` cputypes (`["0x0100000c"]`; names incorrect); new `macho::slices` (thin + fat); multi-slice order unverified |
| script | `polyglot-groovy-*` + `host-jenkins-pipeline-groovy` + `host-classic-asp-vbscript` + 2 advanced-Bash files | nothing; classic ASP `attack` graded bare `T1059` |
| binary-RE | ELF + **`RS20` resource overlay** | new `hcfg_resource_size`; `overlay.rs` tries both layouts, SHA-256 decides |
| mobile | `codec:"dynamic"` (`data` = `assets/payload.dex`) | read the named member, accept one distinct `ANS-` code |

## Day 21 outcome (2026-10-22)

Dual-pool; the rush started the evening after release. Mobile and script passed the first pass with no code change;
the warmups untimed runs were finished by the coordinator with `rush.py takeover`. All 15 routes CORRECT untimed.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **base64 of gzip** (`H4sI...`, "Under Pressure") | base64 gave binary; new `T::Gz` (`decompress` needs a gzip or valid zlib header); mastery row 21 = `T::Gz` |
| metadata | `apk-package-name` | manifest `package` verbatim; new `formats/apk.rs` (AXML, arsc refs, signer certs; other fields guesses) |
| script | `polyglot-github_actions-*` (`.yml`) | nothing; `HUNTRESS_DESCRIPTOR:` env value is found via the marker fallback |
| binary-RE | OCaml loader + `.cmxs` plugin (HCFG only in the plugin) | untagged `config_*` answered from the member the question names (`named_member`) |
| mobile | `codec:"reverse"` (old d8 codec) | nothing |

## Day 22 outcome (2026-10-23)

Dual-pool; the rush started ~2 h after release. Warmups, script and mobile passed the first pass with no lane code
change; all five were done within 8 min. All 15 routes CORRECT untimed. Mastery went from 5 to 6 sets (script,
binary, mobile, metadata).

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **base64 of zlib** (`eNo...`, "Deflated Ego") | nothing (d21's `decompress` takes zlib); row 22 pinned to `T::Gz` |
| metadata | `jar-main-class` | manifest `Main-Class` verbatim; new `formats/jar.rs` (missing/launcher-less class is an error, rule ungraded) |
| script | `polyglot-nsis-*` (`.nsi`, `StrCpy $0 "..."`) | nothing; marker fallback |
| binary-RE | F# .NET `sample.dll` + runtimeconfig + `FSharp.Core.dll` (R2R) | `machine` for PE 0x14c graded **`i386`** (`x86` and `0x14c` incorrect despite the syntax) |
| mobile | `codec:"base64"`; mastery first samples d1 `plain` | nothing |

## Day 23 outcome (2026-10-24)

Dual-pool. The rush started ~18.5 h after release. Binary passed the first pass with no code change; the other four
lanes were fixed by their agents. All five were done 25 min after the rush started, and all 15 routes were CORRECT
untimed, with no ranked attempts spent. Details in `solver_py_scripts/day-23-*/*/NOTES.md`; run log in
`orchestrator/history.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **Ascii85** `<~...~>` ("Eighty-Five Alive") | peel Ascii85 before the escape/percent/entity peels (its alphabet has `\ % &`); mastery row 23 = Ascii85; pin rows 4-6 by radix (decimal/octal tie refused 20/113 sets) |
| metadata | `sqlite-page-size-bytes` | header u16 @16 (`1` = 65536); the generic `-size-bytes` row answered the file length first. TT: zip CD names graded **sorted** |
| script | `polyglot-gdscript-*` (single); new TT/mastery schema | hex-id files, layered `LYR-<recipe>-` literals (bounded decode search), composite kinds; PowerShell collection `-secondary` = first collected file, not the `stage-*.zip` |
| binary-RE | VB.NET anti-analysis predicates (net8.0 PE32 `.dll`) | nothing; the predicates/trace JSON are not scored, raw CRC-valid HCFG |
| mobile | schema v2 `codec:"analysis"` (TT + all mastery) | DEX interpreter + x86-64 JNI emulator; days 7 and 8 not yet seen as v2 |

## Day 24 outcome (2026-10-24)

Dual-pool. The rush started 40 min after release. Script, binary and mobile passed the first pass with no code
change. Warmups and metadata were fixed by their agents. Metadata's agent filled the disk before it could commit, so the
coordinator wrote its fixtures and notes and made the commit. All 15 routes were CORRECT untimed, with no ranked attempts
spent. Details in `solver_py_scripts/day-24-*/*/NOTES.md`; run log in `orchestrator/history.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **run-length `count:char` pairs** ("String Theory") | new `codec::run_length`; the encoded char can be `,` `:` or space, so pairs are read by position, not split; all counts seen are 1. Mastery row 24 = `T::Rle` |
| metadata | `d24-evtx-001-evtx-record-count` (EVTX) | graded = **walked** complete records (size copy matches), not header claims. New `formats/evtx.rs`; other `evtx_answer` fields are ungraded guesses |
| script | `polyglot-jsp-*` + `host-jsp-webshell-jsp` | nothing; `DESCRIPTOR_<HEX>` JSP declaration matches the existing pattern. TT/mastery = Day 23 schema, mastery 8 sets |
| binary-RE | Scala patch-diffing JAR pair (vulnerable/corrected) | nothing; the diff is not scored. Untagged config fields are read from the first member (vulnerable.jar). This assumes both configs are equal, which they were |
| mobile | v1 `split` (single), v2 `analysis` (TT + 8 mastery APKs) | nothing; emulator checks are plain arithmetic. Days 7 and 8 still not seen as v2 |

## Day 25 outcome (2026-10-25)

Dual-pool. The rush started on time; the release was detected at 23:00:01 and all five lanes were done by 23:04:52.
Script, binary and mobile passed the first pass with no code change. Warmups and metadata were fixed by their
agents. All 15 routes were CORRECT untimed, with no ranked attempts spent. Details in
`solver_py_scripts/day-25-*/*/NOTES.md`; run log in `orchestrator/history.md`.

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **Unicode code points `U+XXXX`** ("You Have a Point") | new `codec::code_points` (`T::Cp`), peeled after run-length. Mastery row 25 = `T::Cp`; 123 mastery sets |
| metadata | `d25-registry-001-registry-hive-root-name` (REGF hives) | graded = name of the `nk` that the base block's root offset (u32 @0x24, +0x1000) points at, **not** the embedded file name @0x30. New `formats/regf.rs`; the other `registry_answer` fields are ungraded guesses |
| script | `polyglot-elixir-*` + `host-elixir-phoenix-elixir` (`.exs`) | nothing; `descriptor_<hex> = "..."` matches the existing pattern. Mastery now 9 sets |
| binary-RE | Clojure JAR pair (vulnerable/corrected) | nothing; untagged configs answered from the first member again (configs equal). Mastery 9 sets |
| mobile | v1 `hex` (single), v2 `analysis` (TT + 9 mastery APKs) | nothing; pure DEX arithmetic. **Day 7 seen as v2 for the first time**; day 8 still not seen as v2 |

## Day 26 outcome (warmups, 2026-10-26)

| Lane | Gimmick | What broke / what was graded |
|---|---|---|
| warmups | **Brainfuck** `+-.` deltas on one cell ("Food for Thought") | new `codec::brainfuck` (`T::Bf`, full bounded interpreter), peeled after code points, before Morse. Mastery row 26 = `T::Bf` (each fragment is its own program, from cell 0); 128 mastery sets |

## Day 26 predictions

- Warmups: another per-character notation (`&#x..;`, `\x4E`, `0x4E` tokens, quoted-printable `=4E`, `\N{...}`); QP and
  `0x` tokens are not peeled yet. Mastery row 26 joins the fixed rows.
- Metadata: a follow-up `registry-*` field ("regroup the systems": the `Marker` value; "planted hive": sequence
  mismatch, bad checksum or timestamp outlier). `registry_answer` already answers those as **ungraded guesses**.
- Script: one more polyglot language on the Day 23 TT schema; mastery 10 sets.
- Binary-RE: likely nothing; another JVM-language JAR pair. Check config equality if a pair diverges.
- Mobile: a new v2 mechanism; day 8 may finally appear as v2.

## Day 25 predictions

- Warmups: another simple text codec (run-length with counts > 1, Baudot/Bacon, morse, base-N); mastery row 25 joins the
  fixed rows.
- Metadata: a follow-up `evtx-*` field (distinct/duplicate record count, truncated flag, first/last timestamp) or a new
  container. `evtx_answer` already answers those as **ungraded guesses**.
- Script: one more polyglot family (another server-side webshell language: ASPX/PHP/CFM?) on the Day 23 TT schema.
- Binary-RE: likely nothing; if a two-member pair ships unequal configs, untagged fields need an explicit equality check.
- Mobile: a new v2 mechanism; days 7 and 8 may finally appear as v2.

## Day 24 predictions

- Warmups: another "bigger alphabet" (base58/62/91, Z85, base45, uuencode/xxencode) or undelimited Ascii85. Z85 overlaps
  base64's alphabet: try it template-checked after the cheap peels. Mastery row 24 joins the fixed rows.
- Metadata: another `sqlite-*` field (page count, text encoding, application id, truncation, embedded-header offset).
  `sqlite_answer` already answers those as **ungraded guesses**; learn the format from single-solve. Expect sorted lists.
- Script: one more polyglot family on single-solve; TT/mastery may add a new `LYR` layer or another composite kind.
  The grouped oracle is per file under hex ids (~1500 submits, ~6 min): budget for it, or group by family+kind.
- Binary-RE: likely nothing again; check the primary-member routing if the zip gains more members.
- Mobile: a new v2 token-chain mechanism. `dex_code`'s error names the missing opcode/API; Android framework values
  (`Build.*`, debugger/root checks), crypto APIs and arm64-only libs are not supported yet. Never invent framework values.

## Day 18 predictions

- Warmups: a fourth "<cipher> of hex" (Polybius/tap code, binary, Braille, semaphore, NATO alphabet?). Add the decoder
  to `T` + `fixed_codec` and try the hex reading first.
- Metadata: more `media-*` kinds on the same audio path (sample rate, channels, duration, container, an audio
  attachment inside an `.eml`, codec behind a lying extension) - `formats/audio.rs` already prints these; only
  `codec-name` is graded so far.
- Script: one more `polyglot-<lang>` family; check the language's assignment operator against `analyse_literal`.
- Binary-RE: expect more fields added to old specimens. Run the grouped oracle (it groups `dNN_` ids by day) at once,
  then probe one field per submit.
- Mobile: a new `codec` name; the generic search tries permutations whenever an `order` array is present.

## Day 16 predictions

- Warmups: another whole-string transform (Vigenere/keyed, rail fence, base85/ascii85/uuencode, XOR-hex?) - row 16 joins
  the mastery rows; keep the template-match-first loop and add the decoder to `T` + `fixed_codec`.
- Metadata: another OOXML/Office field (xlsx/pptx rels, core props, custom XML, macros `vbaProject.bin` presence,
  `docProps/app.xml` Application) - `ooxml_answer` already answers ids/counts/hyperlinks/core+app props by name; check
  list ordering with the untimed oracle (value-sorted so far).
- Script: one more `polyglot-<lang>` + `host-<lang>-*` family (descriptor literal; `analyse_literal` fallbacks cover it).
- Binary-RE: `d16` may again be a multi-file specimen (tagged variants) - the tag routing is generic; HCFG unchanged.
- Mobile: a new `codec` name; if `data` is not the answer, look in `lib/*/*.so` strings for `ANS-`.

## Day 14 predictions (verify against single-solve; implement robustly, not as a single guess)

| Lane | Challenge | Prediction | Pre-implementation |
|---|---|---|---|
| warmups | "Seven Steps" | Caesar/ROT-n layer; mastery row 14 | try all 25 shifts, keep digits/hyphens, pick the shift matching the template; don't hard-code 7 |
| script-analysis | MSBuild trusted project | `polyglot-msbuild-*` family; descriptor in XML element/attribute that `LIT2` misses | fallback on the `HUNTRESS_ANALYSIS_V1:` marker comment (duplicates the descriptor) or any token decoding to `v=` |
| binary-RE | Objective-C persistence descriptor | `d14` with another HCFG blob (raw or `SENGR` base64) | Mach-O header support in format/machine; per-day loop tolerant of absent `dNN`; settle `format` casing via oracle |
| metadata-forensics | "The Shortcut Home" | `.lnk` kind (target path) | MS-SHLLINK parser + FACTS dump of LocalBasePath, CommonPathSuffix, RELATIVE_PATH, WORKING_DIR, arguments; formats via oracle |
| mobile | "Predictable Crypto" | new codec, e.g. seeded PRNG keystream | add candidate generators (java.util.Random first) to the generic decoder; accept only `^ANS-[0-9A-F]{16}$` |
