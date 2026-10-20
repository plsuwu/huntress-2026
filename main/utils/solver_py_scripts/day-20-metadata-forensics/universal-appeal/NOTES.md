# Day 20 metadata - Universal Appeal

New kind: `macho-cpu-type-set` (`d20-macho-001-macho-cpu-type-set`) on `macos-001.bin`, a 56-byte thin Mach-O
(`cffaedfe`, MH_MAGIC_64 LE, cputype `0x0100000c` arm64, cpusubtype 0, MH_EXECUTE, one LC_UUID). Answer format
`json`: "compact valid JSON array with exact values and order". Free single-solve probes: `["arm64"]` incorrect,
`["CPU_TYPE_ARM64"]` incorrect, **`["0x0100000c"]` CORRECT**, so the format is `0x%08x` lowercase in a compact JSON array.

The first pass refused because there was no Mach-O kind. Fix:
- `formats/macho.rs::slices` (additive): a thin file gives one slice, and a fat file (`cafebabe` 32-bit or
  `cafebabf` 64-bit fat_arch, big-endian, table order) gives one slice per entry, each with the table's
  (cputype, cpusubtype) claim and the slice's own header. The nfat_arch check (<= 64) keeps Java class files out.
- `lanes/metadata.rs::macho_answer` handles any `-macho-<field>` key. The field must not start with a digit, so
  `d20-macho-001-sha256` still reaches the hash handlers. It runs before HANDLERS. A slice without a Mach-O header,
  or a table entry whose cputype disagrees with its slice header ("what truly supports"), is an **error**, not
  a guess. A non-Mach-O file is also an error (the prompt says archives are mixed in, and the graded answer is
  unknown: `[]`? members?). `cpu-type-set` is unique values sorted, which matches table order on typical
  x86_64+arm64 universals. The ordering on a multi-slice file is **unverified**. `cpu-type-name-set`,
  `slice-count`, and `is-universal` are ungraded guesses.

TT (25 questions) and mastery (125, 5 sets) are the cumulative bundle plus this kind. Every set used the same thin
arm64 `macos-001.bin` -> `["0x0100000c"]`. All CORRECT on the first try (untimed regular attempts). Fixtures:
`ctf-run/fixtures/metadata/d20-*.json`. The single-solve fixture is hand-written: the ctf-run re-run came after the
curl probe solved it, so it got `already_solved`.

Prompt: "A macOS deployment includes thin and universal binaries mixed with archives. Enumerate every CPU type
and identify what truly supports the advertised fleet." Next-day guesses: fat/universal files (possibly with a
fat table that lies about a slice, so compare against the slice header), `ar` static archives (`!<arch>`) of
Mach-O objects, zips containing binaries, arm64e (cpusubtype 2) vs arm64, and a "supports fleet" boolean or
missing-architecture question. The errors above dump the full slice table to FACTS, so settle the graded rule
with free single-solve probes.
