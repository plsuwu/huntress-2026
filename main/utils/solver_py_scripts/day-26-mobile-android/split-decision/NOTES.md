# Day 26 mobile - Split Decision

New container: **split APK sets** (`.apks` = a zip of `base.apk` + `feature.apk`; the base has the puzzle,
the feature split has a manifest (`split="arena_feature"`) and an asset, no code). The prompt ("reassemble evidence
distributed across base and configuration splits") is literal this time: part of the answer lives in the split.

- Single-solve (v1 `split`): the base's `parts` has an empty slot (`["","ANS-7","E37D13"]`, order `[2,0,1]`);
  `feature.apk:assets/huntress_feature.json` = `{"part":"4754DEA01","part_index":0,..}` fills `parts[0]`.
  The first pass failed here (11-char result).
- TT (v2 `analysis`, `.apks`): `Util.mergeFeature41` calls `Assets.readSplit("huntress_feature.bin")`, which walks
  `ctx.getApplicationInfo().splitSourceDirs`, opens each with `java.util.zip.ZipFile`, `getEntry("assets/"+name)`,
  reads 16 bytes (two big-endian longs mixed into the token).
- Mastery: **10 sets**: nine bare v2 APKs (days 2, 3, 6, 11, 13, 16, 19, 21, 25) plus set 10 = the d26 `.apks`.
  **Day 8 still not seen as v2.** Mastery also ships `.apk` and `.apks` side by side; artifact names match the
  delivery names.

Code (`ctf-run`):
- `lanes/mobile.rs::collect`: a container with exactly one puzzle APK plus manifest-only APKs is a split set,
  answered as one item named after the container (the question's artifact). `solve_apk(bytes, splits)` merges
  `part`/`part_index` fragments from the splits' `assets/*.json` into empty `parts` slots (conflicts error out), and
  `dex_code` sees split assets/native libs too (AssetManager merges splits).
- `formats/dalvik.rs`: `Context.getApplicationInfo()` (`splitSourceDirs`, `sourceDir`), `ZipFile.<init>(String)`,
  `getEntry`, `getInputStream`, `close`. The `files` callback carries split access: `dalvik::SPLITS` lists split
  paths, `<path>!/<member>` reads a member.
- DEX failure messages now keep the tail of the call chain (`ends()`), so the unsupported API is visible.

Graded: single, untimed TT and untimed mastery all CORRECT on the first submit after the fix. Fixtures
`fixtures/mobile/d26-*.json`; unit test `split_apk_sets` (`testdata/d26-mobile-{single,tt}.apks`). Replay 118/118.

Disassembly: `uv run -q --with androguard python -I ../../day-23-mobile-android/hook-line-and-signal/dex_dis.py classes.dex`.
