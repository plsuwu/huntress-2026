# Day 28 binary RE - Family Clustering ("Family Resemblance")

d28 = **three x86-64 ELF builds** (CD order): `d28--sample-family-a.elf`, `d28--sample-family-b.elf`,
`d28--sample-false-friend.elf` (22.9 KB each, raw HCFG). family-a/b share one config (`BR28-000`); the false friend
carries its own (`FF28-000`).

New d28 questions: `artifact_sha256_{1,2,3}`, tagged `analysis_{family_a,family_b,false_friend}_{format,machine}`
(enum `elf`, integer `62`), untagged `config_{campaign,domain,mutex,url}` ("Decode family-a.elf and family-b.elf and
require their config objects to be byte-for-byte equal ...; do not use false-friend.elf"), and
`false_friend_config_campaign`.

**The big change: ~25 new fields on old days** (single 392 answers vs 324 on Day 27). All graded CORRECT on the first
submit after the fix:

| Field | Rule (graded) |
|---|---|
| `d01_structural_artifact_count` / `_ordered_set_sha256` | members of the day in CD order; sha256 of the members' hex digests joined by `:` |
| `d02_analysis_vtable_symbol` | first SHT_SYMTAB name matching the question's `/regex/`; **PE variant**: primary COFF symbols in table order (aux records skipped), verbatim incl. `.rdata$` prefix |
| `d03/d22/d23_analysis_cli_runtime` | CLI header MajorRuntimeVersion.MinorRuntimeVersion (`2.5`) |
| `d03/d23 has_decoy_branch`, `d09 has_nim_runtime`, `d12 has_fpc_runtime`, `d17 has_gnat_runtime` | "true exactly when the final bytes [as Latin-1 or UTF-16LE] contain X [or Y] [or any nonempty ELF symbol name contains Z]", parsed from the text; byte needles must be in `NEEDLES` (scanned at analysis, ~3 ms on the 67 MB d08), else Err |
| `d05_analysis_symbol_count` | nonempty SHT_SYMTAB names (symtab only; dynsym not added) |
| `d06/d26_analysis_class_count`, `d06_analysis_class_name` | CD entries ending `.class`; Main-Class `this_class` |
| `d10_analysis_section_count` | ELF e_shnum (index 0 included); **PE variant** (d10 `.exe` in TT): NumberOfSections |
| `d11 d_symbol_count`, `d13 crystal_symbol_count`, `d18 handler_symbol_count` / `_symbols_sha256` | regex from the question over nonempty symtab names; sha = sorted, LF-joined |
| `d17_analysis_conditional_branch_opcode_count` | `.text` bytes 0x70..0x7f + 0x0f 0x80..0x8f pairs |
| `d19_analysis_packing_key_{1,2}_hex` | k1/k2 of the final validated PK19 overlay |
| `d26_analysis_method_names_sha256` | unique Main-Class method names, sorted, LF-joined |

Code (`lanes/binary_re.rs`): tags now match multi-word suffixes (`false-friend` -> `false_friend_`, longest wins);
the untagged "byte-for-byte equal" config fields now **check** equality over the named members (else all of the day,
as on d27) minus the "do not use" ones, and refuse if they differ. `Set::members` keeps each day's (name, sha256) in
CD order.

d02 and d10 alternate ELF/PE across sets (seen in the d27 artifacts too): every new field on those days must handle both.
Mastery is **12 sets** (4704 answers). All three modes CORRECT untimed; no ranked attempts spent.
Fixtures: `ctf-run/fixtures/binary_re/d28-{single_solve,time_trial,mastery}.json` (replay 75/75 clean).
