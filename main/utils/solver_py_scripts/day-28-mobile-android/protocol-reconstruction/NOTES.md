# Day 28 mobile - Protocol Reconstruction

The "reconstruct the app-to-backend protocol, preserve nonce and ordering" story is decoration
(`ProtocolReconstruction`, `FrameKit`, `WireState` classes). The answer is the usual `ANS-` + 16 hex.

- Single-solve: v1 **new codec `"packet"`**: `data` is hex of a framed packet
  `48 41 | 14 | "ANS-601D3906BB5885AC" | 11` (magic `HA`, length byte 0x14 = 20, payload, one trailer byte).
  No `packet` branch in `decode`: the generic search's hex step exposes the plain `ANS-` payload (unique hit).
  If a later day encodes the payload inside the frame, add a real `packet` branch (strip `HA` + length byte).
- TT: v2 `analysis` again (d27 TT was v1); `Release.code()` runs through `FrameKit`/`WireState`/`Mix` in the
  existing interpreter with no new API.
- Mastery: **12 sets, all v2**: days 1, 4, 7, **8**, 11, 14, 17, 18, 22, 24, set 11 = the d26 `.apks`
  (v2, `huntress_feature.bin` in the split), set 12 = d28. **Day 8 seen as v2 for the first time**
  (`CleartextSession`/`Core`/`Guard`/`Probe`), pure DEX arithmetic.

Graded: the rush first pass got single-solve, untimed TT and untimed mastery CORRECT on the first submit, with
**no code change**. Fixtures: `ctf-run/fixtures/mobile/d28-{single_solve,time_trial,mastery}.json` (artifacts went
through the fixtures symlink into the main checkout under `.../artifacts/`). Replay is 124/124 clean.
