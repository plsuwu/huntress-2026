# Day 30 mobile - Layered Protections

The "obfuscation, native indirection, runtime decryption, integrity checks" story is decoration
(`LayeredProtections` class, `DAY-30:layered_protections`). The answer is the usual `ANS-` + 16 hex.

- Single-solve: v1 **new codec `"layered"`**: `data` is base64; decoded byte `j` XOR `(key + j)` (key = 12 in the
  puzzle), then the whole buffer **reversed**, gives the answer. The first pass refused it (no branch, and the search
  had no XOR-ramp step). Found with a brute force over two-layer compositions: one unique hit.
  - `lanes/mobile.rs::unlayer` is the direct branch.
  - `unxor_ramp` (the XOR counterpart of d29's `untable`) is a new generic search step. It recovers the ramp start
    and step from the `ANS-` prefix, so base64 -> reverse -> ramp is found at depth 3 without the key or the codec name.
- TT: v2 `analysis` (day 30); the existing interpreter answered it, no new API.
- Mastery: **14 sets, all v2**: days 2, 4, 5, 7, 9, 13, 15 (`libarena.so`), 17, 19, 21, 23, set 12 = the d26 `.apks`,
  29, and set 14 = d30. **Days 2 and 5 seen as v2 for the first time** (no new mechanism needed).

Graded: after the `layered` branch, single-solve, untimed TT and untimed mastery were all CORRECT on the first
submit. Fixtures: `ctf-run/fixtures/mobile/d30-{single_solve,time_trial,mastery}.json` (written through the fixtures
symlink into the main checkout). Replay is 130/130 clean.

If a later v1 day stacks more layers (depth > 3), raise the search depth in `generic` or add a direct branch.
Brute-forcing compositions in Python first (xor/sub with constant or ramp keys, reverse, base64/hex) is the fastest
way to find the order.
