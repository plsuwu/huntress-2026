# Day 19 metadata - Little Endian on the Prairie

New kind: `elf-e-machine-u16-hex` (`d19-elf-001-elf-e-machine-u16-hex`) on `collector-001.elf` (ELF64 LSB EXEC,
x86-64, 14408 B, entry 0x401080). Single-solve: **`0x003e` CORRECT**, graded as `0x%04x` lowercase (format string
"lowercase hexadecimal with a 0x prefix"), same padding as `pe-coff-machine-u16-hex`.

The first pass refused: the old `elf-machine-u16-hex` HANDLERS suffix does not match `elf-e-machine-...`.
Fix: `lanes/metadata.rs::elf_answer` routes any `-elf-<field>` key (field after the last `-elf-`, not starting
with a digit, so `d19-elf-001-sha256` still reaches the hash handlers) and reads the field from the **header
alone**: byte order from EI_DATA (1 LSB, 2 MSB; anything else is an error), `e_machine` at offset 18, `e_type` at
16, `e_entry` at 24. It does not use `Elf::parse`, which needs valid section headers. A non-ELF file answers `none`
(ungraded guess; the prompt mentions "Windows executables and renamed data" pooled with the ELFs). Only
`e-machine-u16-hex` is graded; `e-machine` (decimal), `e-machine-name`, `e-type[-u16-hex]`, `e-entry-hex`,
`ei-class` (`ELF32`/`ELF64`), `ei-data` (`little`/`big`), `ei-osabi` are guesses.

Prompt: "Linux binaries from several systems have been pooled with Windows executables and renamed data. Determine
architecture directly from the ELF header and rebuild deployment groups." The title hints at big-endian ELFs (MIPS,
PPC, s390x...) where reading `e_machine` little-endian gives the wrong value (`0x0800` instead of `0x0008`), plus
PE/data files renamed `.elf` and a "group files by architecture" question. None of that showed up on d19: TT (24
questions) and mastery (120 questions, 5 sets) = cumulative bundle + this kind, and every set had the same x86-64
`collector-001.elf` -> `0x003e`. All CORRECT first try (untimed regular attempts). Route: dual-pool, ranked 32 /
regular 32; TT 50 s, mastery 170 s. Fixtures `ctf-run/fixtures/metadata/d19-*.json`.

Next-day guess: a big-endian / 32-bit ELF, a lying extension (PE named `.elf` -> `none`, or ELF named `.dat`), or a
grouping answer (compact JSON of filenames per `e_machine`, ordering to settle with the untimed oracle).
