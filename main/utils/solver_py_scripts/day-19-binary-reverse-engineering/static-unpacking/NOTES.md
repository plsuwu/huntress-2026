# Day 19 binary RE - Static Unpacking ("Jack in the Box")

d19 = stripped x86-64 ELF (11 KB, dynamically linked) with a decoder stub and a **packed `PK19` overlay** appended at
the end of the file. No HCFG anywhere in the plain bytes; the config only appears after unpacking. Layout (spelled out
in the question):

```
+0  "PK19"   +4 ver(01)   +5 k1   +6 k2   +7 pad   +8 packed_len u32le   +12 sha256(unpacked)[32]   +44 packed bytes
unpacked[i] = ror8(packed[i] ^ k2, 3) ^ k1          -> HCFG record at unpacked offset 0 (usual HCFG rules)
```

variant-000: overlay at 10808, k1 0x21, k2 0xa4, 380 packed bytes, overlay ends exactly at EOF (size 424).
`unpack_overlay.py` here is the Python reference; the Rust lives in `ctf-run/src/formats/overlay.rs`
(`PK` + two digits, last magic that validates by SHA-256; rotation searched 0..8 so a sibling scheme still unpacks).

New d19 questions (12): `artifact_sha256_1`, `architecture`, `format`, `machine`, `config_{campaign,domain,mutex,url}`,
`config_crc32_hex`, `hcfg_version`, **`overlay_offset`** (zero-based offset of the final magic) and
**`overlay_size`** (member length - that offset). No `xor_key_hex` / `hcfg_offset` / `hcfg_storage` on d19 (if asked:
`hcfg_offset` should be 0 = "unpacked offset 0"; `hcfg_storage` returns Err for source `overlay` - probe it).
The overlay question text also says "PK19 for Day 19 or **RS20 for Day 20**": expect a second overlay scheme tomorrow,
with its own magic (`RS20`) - `overlay::is_magic` only accepts `PK<dd>`, extend it and the decode from the question.

**Every old question was reworded** (precise per-field instructions instead of "What is the exact ... value"), and some
formats changed. What broke:

| id | change | fix |
|---|---|---|
| `dNN_analysis_format` d12..d19 (incl. d15/d16 tagged) | now `enum` `[elf, pe, pe_cli, jar, wasm, neko_bytecode]` (we sent `ELF`) | `container_label`: enum -> lowercase token, .NET PE -> `pe_cli`; generic `enum_checked` guard on every answer (unique case-insensitive match, else Err) |
| `d01_analysis_recover_code` (stripped d01, TT variant-014) | now `pattern` "unsigned base-10 integer, or the literal token none" (was `exact_text`) | `none` also when the syntax names `literal token none` |
| `d01_analysis_marker` | "Read the .huntress section bytes as UTF-8 and remove trailing NUL bytes only" | marker = `.huntress` minus trailing NULs (same value as the symbol string on every fixture) |

`neko_bytecode` in the format enum hints at a future NekoVM (Haxe) specimen: container magic would need a `Container`
variant. d01 `elf_class` is now enum `ELF32`/`ELF64`; booleans are enum `true`/`false` (already matched).

Graded: single CORRECT (2nd submit, after the enum fix; first pass was refused on the missing overlay), TT CORRECT
(regular attempt; 1st refused on stripped d01 recover_code, no submit spent), mastery CORRECT first submit (5 sets,
1140 answers). Route: dual-pool, TT ranked 64 / regular 32 (80 s), mastery ranked 16 / regular 16 (325 s).
Fixtures: `ctf-run/fixtures/binary_re/d19-{single_solve,time_trial,mastery}.json` (replay 48/48 clean).
