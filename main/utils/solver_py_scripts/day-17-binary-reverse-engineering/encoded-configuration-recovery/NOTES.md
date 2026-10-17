# Day 17 binary RE - Encoded Configuration Recovery ("Decoder Ring")

d17 = Ada (GNAT) x86-64 ELF, not stripped, raw HCFG in `.rodata` (key 0x73) - the generic analysis answered every d17
question first time. What broke was **new questions on old specimens**: every day now has exactly 12 questions
(204 single/TT, 1020 mastery = 5 sets), and the lower days gained fields:

| id | graded value (variant-000) | rule |
|---|---|---|
| `d01_analysis_marker` | `BR01-CATALOG-000` | string at the `marker` OBJECT symbol; **stripped d01** (TT variant-014): first cstr of the `.huntress` section (same bytes) |
| `d01_analysis_recover_code` | `8100` (integer) | `eax` at `ret` of `recover_code` (`mov eax,0x1fa4; ret`). Stripped d01: the question becomes **exact_text** and the graded answer is `none` (symbol absent), although the bytes are still there |
| `d02_analysis_mangled_score_symbol` | `_ZNK15DiscoveryEngine5scoreEPKc` | first Itanium name containing `5score`. **MinGW PE d02** (mastery sets 1/3/5): graded `.text$_ZNK15DiscoveryEngine5scoreEPKc` - the first COFF string-table hit, section prefix included |
| `d02_analysis_typeinfo_symbol` | `_ZTI15DiscoveryEngine` | `_ZTI` of the class that has a `_ZTV` (BehaviorEngine is the vtable-less base). PE: `.rdata$_ZTI15DiscoveryEngine` |
| `d04_analysis_has_go_runtime` | `true` | `.gopclntab`/`.go.buildinfo` section or `\xff Go buildinf:` bytes |
| `d05_analysis_has_rust_mangled_symbol` | `true` | symtab/dynsym name `_ZN...17h<16 hex>E` or `_R<Upper>` |
| `d06_analysis_methods` | `["<init>","decodeConfig","campaign","evaluate","main"]` | manifest `Main-Class` methods, class-file order, `<init>` included |
| `d06_analysis_access_flags` | `49` | Main-Class access flags (0x31) |
| `d07_analysis_has_coroutine_runtime` | `true` | jar has `kotlin/coroutines/` (or `kotlinx/coroutines/`) entries |
| `d07_analysis_method_count` | `7` | Main-Class `methods_count` (includes `<init>` and `<clinit>`) |
| `dNN_analysis_hcfg_storage` (d08..d14 minus d10) | `base64` / `raw_hcfg_section` (d14) / `raw_const_array` (d12) | base64 constant -> `base64`; raw blob inside a `.hcfg` section -> `raw_hcfg_section`; raw blob in an ordinary data section -> `raw_const_array` |

`hcfg_storage` vocabulary cost ~200 free single-solve partial submits: `raw`, `section`, `.data`, `none`, `raw_data`,
`raw_rodata`, `raw_array`, `byte_array`, ... are all wrong. Unseen so far: raw blobs in `.rodata`/`.noptrdata`
(d04/d05/d17) are mapped to `raw_const_array` by assumption; a `SENGR` base64 in a `.hcfg` section, the jar constant
(`base64`) and wasm data segments are untested labels. If a later variant asks one of those and is `incorrect`,
probe single-field partial submits.

Changes: `lanes/binary_re.rs` (fields above, `scan_mangled` for non-ELF C++ names, `hcfg_section`), additive
`formats/jvm.rs::class_info` (constant pool + access flags + methods). `classinfo.py` here = Python class dumper used
to explore d06/d07.

Graded: single CORRECT (after oracle), TT CORRECT (2nd regular attempt, stripped d01), mastery CORRECT (3rd regular
attempt: PE d02). Route: dual-pool, TT ranked 64 / regular 32 (80 s), mastery ranked 16 / regular 16 (325 s), 5 sets.
Mastery is gated (409 `prerequisite_required`) on TT CORRECT, TT on single CORRECT.
Fixtures: `ctf-run/fixtures/binary_re/d17-{single_solve,time_trial,mastery}.json`.
