# Day 24 mobile - Emulator Alibi

The emulator-detection story ("cross the emulator environment detection boundary") is decoration. The new d24 v2
classes (`EmulatorAlibi`, `BoardCheck`, `SensorMap`, `Probe`; members such as `emulatorScore`, `mixBoard*`,
`mixSensor*`, `scanAlibi*`, decoy `seal*`) are pure DEX arithmetic in the token chain. The DEX references no
`android.os.Build`, sensor or file APIs, so no framework values were needed.

- Single-solve: v1 `codec:"split"` (`parts` + `order`, empty `data`), answered by `decode`'s `split` branch.
- TT: v2 `codec:"analysis"` (day 24), answered by `dex_code` with the existing interpreter.
- Mastery: **8 APKs** (7 on d23), all v2: days 3, 6, 9, 10, 15 (JNI `libarena.so`), 17, 23 and 24.
  Days 7 and 8 have still not been seen as v2.

Graded: the rush first pass got single-solve, untimed TT and untimed mastery CORRECT on the first submit, with
**no code change**. Fixtures: `ctf-run/fixtures/mobile/d24-{single_solve,time_trial,mastery}.json` (artifacts went
through the fixtures symlink into the main checkout under `.../artifacts/`). Replay is 112/112 clean.
