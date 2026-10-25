# Day 25 mobile - Integrity Signals

The integrity/attestation story is decoration. The new d25 v2 classes (`IntegritySignals`, `VerdictMap`, plus a
`SealCheck` decoy in the TT APK) are pure DEX arithmetic in the token chain. The DEX references no framework APIs
beyond the usual Activity/WebView/TextView scaffolding, so no Android framework values were needed.

- Single-solve: v1 `codec:"hex"`, answered by `decode`'s `hex` branch.
- TT: v2 `codec:"analysis"` (day 25), answered by `dex_code` with the existing interpreter.
- Mastery: **9 APKs** (8 on d24), all v2: days 1, 4, **7** (`WebViewBridge`), 10, 14 (`PredictableCrypto`),
  16 (JNI `libarena.so`), 19 (`ReflectionMaze`, reflection), 22 and 25.
  **Day 7 appears as v2 for the first time** (plain DEX arithmetic). Day 8 has still not been seen as v2.

Graded: the rush first pass got single-solve, untimed TT and untimed mastery CORRECT on the first submit, with
**no code change**. Fixtures: `ctf-run/fixtures/mobile/d25-{single_solve,time_trial,mastery}.json` (artifacts went
through the fixtures symlink into the main checkout under `.../artifacts/`). Replay is 115/115 clean.
