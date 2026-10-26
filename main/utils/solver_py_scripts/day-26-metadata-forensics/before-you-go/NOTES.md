# Day 26 metadata - Before You Go (`metadata-forensics-day-26-before-you-go`)

Prompt: "Prefetch evidence records what Windows prepared to execute even after filenames changed. Recover executable
identities and connect execution traces to the correct hosts." As usual only one new question kind came out of it:
single-solve asks it once, TT is the cumulative bundle (31 q), and mastery has 10 sets (310 q).

## New kind: `d26-prefetch-001-prefetch-executable-name`
- Graded: the SCCA header's executable name (UTF-16LE, 60 bytes @0x10, up to the first NUL) verbatim, uppercase as
  stored (`CMD.EXE`, `BROKER.EXE`, `SAFFRON71.EXE`). The `.pf` files are renamed `trace-NNN.pf`, so the file name
  says nothing.
- Single-solve shipped a **real Windows 10 compressed prefetch** (`MAM\x04` + u32 size + LZXPRESS Huffman, MS-XCA
  2.2.4); TT and all 10 mastery sets shipped **uncompressed** SCCA v30 (`1e 00 00 00 SCCA`). Both paths are needed.
- New `ctf-run/src/formats/prefetch.rs`: `xpress_huffman` (64 KiB blocks, 256-byte nibble tables; `MAM\x84` skips
  the CRC32), `Prefetch::parse` (versions 17/23/26/30; file-info block @0x54, last-run times, run count @0xD0 or 0xC8
  when metrics start at 0x128, file-name strings, volume records). Unit tests use `testdata/d26-prefetch-001.pf`.
- `lanes/metadata.rs::prefetch_answer` dispatches on `-prefetch-<field>` before HANDLERS. Only `executable-name`
  has been graded. Other fields (hash `%08X`, run-count, last-run-time ISO, volume serial `%08X`/device path,
  filenames sorted JSON, executable-path = the file-name string ending in the exe name) are **ungraded guesses**.
  An unknown field fails with a `prefetch_facts` dump.
- Predicted follow-ups ("connect execution traces to the correct hosts"): volume serial / device path / volume
  creation time as the host link, the prefetch hash, run count, last run time.

## Runs (all untimed, all CORRECT on the first submit)
- `ctf-run/runs-d26/.../single_solve-986a5baa-1793019834`, `time_trial-d974a24e-1793019849`,
  `mastery-f77250a1-1793019862` -> `fixtures/metadata/d26-{single_solve,time_trial,mastery}.json` (replay 70/70 clean).
