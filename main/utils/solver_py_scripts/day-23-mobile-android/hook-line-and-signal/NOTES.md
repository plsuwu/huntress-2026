# Day 23 mobile - Hook Line and Signal

**New puzzle schema v2:** `assets/huntress_puzzle.json` = `{"schema":"huntress.mobile.puzzle.v2","day":N,"codec":"analysis","build":"<hex>"}`.
It has **no `data`**. The answer is computed by the app's own bytecode. `Release.code(Context)` in `classes.dex` takes
`token()`, runs it through a chain of long mixers (`xor/mul/add/rotate` helpers spread over `Util`, `Mix`, `HookScan`, ...)
and returns `"ANS-"` + the 16 uppercase hex nibbles (a `charAt` loop over `"0123456789ABCDEF"`). `Release.proof` is
`Long.toHexString(token())`, a decoy. Decoy classes/fields (`sealHook*`, `KPatch*`, `TrampolineMap`) are never called:
these are the "generated false positives" from the prompt.

- Single-solve was still a **v1** puzzle (`codec:"xor"`, base64 `data` + `key`). The existing decoder answered it.
- TT and **every** mastery APK are v2. Mastery (7 APKs) samples earlier days, each rebuilt as an `analysis` APK whose
  token chain uses that day's mechanism:
  - d15: `NativeLibrarySecrets.nativeShare(J)J`, a JNI function in `lib/x86_64/libarena.so` (straight-line arithmetic).
  - d16: `JNIBoundary.nativeShare(JI)J`, a native with a guard (`cmp ecx,0xffff; jle; movzx; cmp; jne`). It returns
    0 on the wrong path.
  - d20: `Assets.read("payload.dex")` -> `ByteBuffer.wrap` -> `InMemoryDexClassLoader`, then the class name taken from
    `moduleContract().substring(indexOf(':')+1)`, then `loadClass().getDeclaredMethod("share", Long.TYPE).invoke(null, Long)`.
  - d23: `Probe.s2/s3`, brute-force 4096-step loops (an `instrumentationSignals` xor of their results feeds the mix).
  - The other days (1-6, 9-14, 17-19, 21, 22) are pure DEX arithmetic and need no extra API.

## Implementation (ctf-run)

- `formats/dex.rs`: DEX parser (strings/types/protos/fields/methods, class data, code items, static values).
- `formats/dalvik.rs`: Dalvik interpreter. It covers the full integer/long/float opcode set, branches, switches,
  arrays, static and instance fields, and `<clinit>` on first use. It holds several DEX files, so secondary DEX
  files are loaded by `InMemoryDexClassLoader`/`DexClassLoader`.
  - Builtins: String, StringBuilder, Long/Integer helpers, Math, assets (`AssetManager.open`, `InputStream.read`,
    `Arrays.copyOf`, `ByteBuffer.wrap`), reflection (`Class.forName`, `loadClass`, `getDeclaredMethod`,
    `Method.invoke`, `Field.get*`, `Long.TYPE`) and `System.loadLibrary` (no-op).
  - Any other external call is an error. The interpreter never guesses a framework value.
- `formats/x64emu.rs`: a tiny x86-64 emulator for JNI leaf functions. It has ALU ops with flags, shifts, rotates,
  imul, `movabs`, `movzx`/`movsx`, `jcc`/`jmp`, `cmov`, `setcc`, push/pop and stack memory. `call()` uses the
  System V argument order; the JNI `env`/`class` arguments are passed as 0.
- `lanes/mobile.rs::dex_code`: after `decode` and the `lib/*.so` string scan fail, it runs every static method of the
  primary `*.dex` files that returns `String` and takes only reference parameters (null arguments). Natives resolve
  through the `lib/x86_64/*.so` dynsym, using the JNI-mangled symbol (`dalvik::jni_name`). It accepts exactly one
  distinct `ANS-` code; otherwise it errors and lists each method's failure.
- Each APK takes about 2-7 ms to analyse; the d23 `Probe` loops dominate.

## Graded

- Single-solve CORRECT on the first pass, with no change.
- Untimed TT (1 APK) CORRECT on the first submit after the interpreter (`ANS-E4F679E90A7D1CAE`). An independent Python
  transliteration (`check_tt_by_hand.py`) agrees with the interpreter.
- Untimed mastery CORRECT on 9 variants:
  - The first variant failed only on d15 (native) until the x86 emulator was added.
  - The second variant failed on d16 (flags/branches) and d20 (secondary DEX + reflection) until those were added.
  - The 7 variants after that were all CORRECT on the first submit.
- Days seen as v2 so far: 1-6 and 9-23. **Days 7 and 8 have not been seen as v2.**
- Fixtures: `ctf-run/fixtures/mobile/d23-{single_solve,time_trial,mastery}.json`, plus 7 extra graded mastery variants
  `d23-mastery-<attempt8>.json`, made with an ad-hoc copy of `fixtures_from_runs.py` (that tool keeps only one fixture
  per mode). The artifacts went through the fixtures symlink into the main checkout under `.../artifacts/`.
  Replay is 109/109 clean.
- Unit tests: `lanes::mobile::tests::analysis_apks_run_their_dex` (testdata `d23-mobile-{tt,v2-d16,v2-d20}.apk`),
  `formats::x64emu::tests`.

## Tools here

- `dex_dis.py <classes.dex>`: androguard disassembly (`uv run -q --with androguard python -I dex_dis.py ...`). It is
  the quickest way to see a new APK's call chain.
- `check_tt_by_hand.py`: a hand transliteration of the TT APK's chain. It was used to cross-check the interpreter.

## Next

- A new v2 day will most likely add one new mechanism to the token chain. When `dex_code` fails it names the
  unsupported opcode or external call: add a builtin or opcode to `dalvik.rs`, or an instruction to `x64emu.rs`.
- Likely next mechanisms:
  - Android framework values (`Build.*`, `Debug.isDebuggerConnected`, `File.exists("/system/bin/su")`) for the
    root/anti-debug days. Do not invent these; check what the challenge's "real" path expects.
  - arm64-only native libs. There is no aarch64 emulator yet; only `lib/x86_64` is used.
  - Crypto APIs (`MessageDigest`, `Cipher`).
