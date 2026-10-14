# Lane patterns & per-day "gimmick" predictions (as of Day 14, 2026-10-15)

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

### Warmups - single = one layer, mastery = cumulative
- Single/TT: the whole blob in the day's encoding, decoding to the fixed template; answer = slug `word-word-word-NN`.
- Mastery: `NN<TAB>fragment` rows, raggedly sliced from the template; **row N uses day N's encoding**:
  01 base64, 02 rot13, 03 hex, 04 8-bit binary, 05 decimal, 06 octal, 07 %-hex, 08 `&#dec;`, 09 `"\uXXXX"`, 10 base32,
  11 base64url (unpadded), 12 reversed (+ `-` right padding to 13 chars), 13 atbash.
- Mastery set count = **5N - 2** (23 on d5 ... 63 on d13; d14 route lists 68).
- Parse rows whitespace-faithfully (leading spaces matter), use the fixed index->codec mapping first, enumerate only as fallback.

### Metadata forensics - one new kind per day, sometimes a recap bundle
- Single-solve asks 1 question of the day's new kind. Kinds so far: jpeg SOF dims (`WxH`), png IHDR dims, pdf header
  version, zip EOCD total entries, zip CD names (compact JSON), PE COFF machine (`0x014c`), PE entry RVA (`0x%08x`),
  PE OriginalFilename (VS_VERSION_INFO), .NET MVID (lowercase bare GUID), MSI ProductCode (`{UPPER}` with braces),
  plus evidence-note filename/size/md5/sha1/sha256/magic16/utf8-bom and "synthesis order" (sha256-sorted JSON array).
- TT/mastery: days 8, 9, 13 (regular) asked the whole cumulative bundle (12 -> 18 questions); days 10-12 (ranked) asked
  only the day's kind. Build the solver to handle either. `-filename` suffix collides between kinds - match carefully.

### Mobile Android - not cumulative
- Same tiny hand-built APK every day; answer = `assets/huntress_puzzle.json` decoded, `ANS-` + 16 uppercase hex. The
  Android story (PendingIntent, providers, WebView, cleartext, trust, keystore) is pure decoration.
- Codecs seen (by day): 4 xor, 5 split(parts/order), 6 hex, 7 caesar(key), 8 reverse, 9 base64, 10 split, 11 xor,
  12 hex, 13 base64. Per-APK keys differ - read them from the JSON. Mastery = N bare APKs, ids `setN.answer`.

## Day 14 predictions (verify against single-solve; implement robustly, not as a single guess)

| Lane | Challenge | Prediction | Pre-implementation |
|---|---|---|---|
| warmups | "Seven Steps" | Caesar/ROT-n layer; mastery row 14 | try all 25 shifts, keep digits/hyphens, pick the shift matching the template; don't hard-code 7 |
| script-analysis | MSBuild trusted project | `polyglot-msbuild-*` family; descriptor in XML element/attribute that `LIT2` misses | fallback on the `HUNTRESS_ANALYSIS_V1:` marker comment (duplicates the descriptor) or any token decoding to `v=` |
| binary-RE | Objective-C persistence descriptor | `d14` with another HCFG blob (raw or `SENGR` base64) | Mach-O header support in format/machine; per-day loop tolerant of absent `dNN`; settle `format` casing via oracle |
| metadata-forensics | "The Shortcut Home" | `.lnk` kind (target path) | MS-SHLLINK parser + FACTS dump of LocalBasePath, CommonPathSuffix, RELATIVE_PATH, WORKING_DIR, arguments; formats via oracle |
| mobile | "Predictable Crypto" | new codec, e.g. seeded PRNG keystream | add candidate generators (java.util.Random first) to the generic decoder; accept only `^ANS-[0-9A-F]{16}$` |
