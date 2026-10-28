# Day 28 metadata - Where It All Began (`metadata-forensics-day-28-where-it-all-began`)

Prompt: "Disk images hold every previous artifact family in context. Recover partition layouts, locate evidence by
offset, and prove which image was rebuilt from an incomplete source." As usual only one new question kind came out
of it: single-solve asks it once, TT is the cumulative bundle (33 q), and mastery has **12 sets** (396 q).

## New kind: `d28-disk-001-disk-partition-start-lba-sequence`
- Graded: the GPT partition entries' `StartingLBA` (u64 @+32 of each 128-byte entry) **in table order, as a JSON
  array of integers** (`[34]`, `[43,110]`, `[42,64,89,106,140,171]`). answer_format `json` ("exact values and order").
- Every image seen (single, TT, 12 mastery) is a small (116-310 KiB) GPT disk: protective MBR (`0xEE`), primary
  header at LBA 1, entries at LBA 2 (128 x 128 B), backup header at the last LBA, 512-byte sectors, partition names
  like `HUNTRESS EVIDENCE`. Entries were always contiguous from slot 0 and ascending, so table order vs sorted is
  **not yet distinguished**. Single-solve's partition was all zeros (no nested evidence yet).
- New `ctf-run/src/formats/disk.rs`: MBR (4 primary + EBR chain for logicals), GPT (header/entry CRC checks, 4096-byte
  sector fallback, backup header via AlternateLBA or last LBA, backup entry array kept separately).
  `lanes/metadata.rs::disk_answer` dispatches on `-disk-<field>` after pcap. Only `partition-start-lba-sequence` is
  graded; the rest (end-LBA / sector-count / byte-size / byte-offset sequences, type/unique GUID and name sequences,
  count, scheme, sector size, disk GUID, usable LBAs, backup LBA, header CRC valid, MBR disk signature) are guesses.
  An unknown field fails with a `disk_facts` dump (MBR entries, both GPT headers and both entry arrays).
- `survey_disks.py <mastery zip>`: quick Python survey of every `.img` in a (nested) delivery zip.
- Predicted follow-ups: "locate evidence by offset" (an earlier artifact family embedded inside a partition: answer
  a byte offset = LBA x 512, or that artifact's own field, so the lane may need to carve partition data and re-run
  the other handlers on it - `Disk::data` returns a partition's bytes); "rebuilt from an incomplete source" (primary
  vs backup GPT disagreement, bad header/entry CRC, or an image whose backup header is missing/truncated).

## Runs (all untimed, all CORRECT on the first submit after adding the kind)
- `ctf-run/runs-d28/.../single_solve-d42d2f29-1793192538`, `time_trial-d7768d11-1793192553`,
  `mastery-0b775549-1793192565` -> `fixtures/metadata/d28-{single_solve,time_trial,mastery}.json` (replay 76/76 clean).
