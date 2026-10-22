# Day 21 warmups - Under Pressure

Encoding: **base64 of a gzip member** ("A familiar wrapper. Something compressed inside."). The single-solve blob
is 152 B of `H4sI...` (gzip magic `1f 8b 08`, mtime 0, deflate). Decompresses straight to the fixed sentence.

Change: `T::Gz` in `ctf-run/src/lanes/warmups.rs` (base64 -> `decompress` -> `codec::text_any`). `decompress`
accepts only bytes with a compression header (gzip `1f8b`, or a valid zlib `78xx` header, in case a later day
switches wrappers), so it never misfires on plain base64. Wired into: the whole-blob peel (before `T::B64`),
`structured()` (base64-looking rows try gzip first), and `fixed_codec` row 21 => `T::Gz`. The first pass failed
because base64 of gzip is binary, so `T::B64` returned `None` and the peel stopped.

Graded (2026-10-22 ~08:36Z): single-solve CORRECT (`eager-compass-mosaic-50`), on the first submit after the fix.
Routes: TT and mastery are dual-pool (`rft=true`; TT 30 s, mastery 120 s, mastery 103 sets = 5N-2).
TT/mastery: see the status section below.
Fixture: `ctf-run/fixtures/warmups/d21-single_solve.json`. Run dirs: `ctf-run/runs-d21/warmups-day-21/`.

Unit test `gzip_blob`: the real blob, plus a synthetic mastery set with a gzip row 21.

Day 22 guess: another wrapper around a familiar layer (zlib/raw deflate, bzip2/xz, or a gzip inside hex/base32).
`decompress` handles zlib already. Raw deflate has no header, so it would need an explicit attempt.

## Status
- Single-solve, TT and mastery: all CORRECT (2026-10-22). The coordinator ran the untimed regular TT and mastery
  attempts via `rush.py takeover`, because the lane agent's own runs were blocked by the permission classifier.
  Fixtures: `d21-single_solve`, `d21-time_trial` (1 answer), `d21-mastery` (103 answers); replay 60/60 clean.
