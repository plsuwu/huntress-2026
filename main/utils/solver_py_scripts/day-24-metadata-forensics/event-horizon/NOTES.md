# Day 24 metadata - Event Horizon (`metadata-forensics-day-24-event-horizon`)

Prompt: "Windows event logs from several hosts have been duplicated, truncated, and bundled with reports. Count
records accurately and find the selectively shortened timeline." As usual only one new question kind came out of it:
single-solve asks it once, TT is the cumulative bundle (29 q), and mastery has 8 sets (232 q).

## New kind: `d24-evtx-001-evtx-record-count`
- Graded: the number of **walked** records, i.e. complete `**\0\0` records whose trailing size copy matches. It is
  not the count the file/chunk headers claim. Single 3, TT 88, mastery sets 33/37/66/73/5/89/60/95.
- `ctf-run/src/formats/evtx.rs`: file header (`ElfFile\0`, 4096 B), 64 KiB chunks (`ElfChnk\0`, 512-byte header,
  CRC32s checked), record walk, `claimed_records()`, `distinct_ids()` and `truncated()`.
- `lanes/metadata.rs::evtx_answer` dispatches on `-evtx-<field>` before HANDLERS (same pattern as `sqlite_answer`).
  Only `record-count` has been graded. The other field names (distinct/claimed counts, chunk count, next record id,
  first/last record id and timestamp, truncated) are guesses at what the prompt implies. An unknown field fails with an
  `evtx_facts` dump.
- Not checked: whether the TT/mastery files actually used the truncation or duplication tricks. The agent's survey
  was cut short when the disk filled. A `distinct-record-count` kind could appear if duplicated logs ever matter.

## Runs (all untimed, all CORRECT on the first submit)
- `runs-d24/.../single_solve-e90afe1b-1792849371`, `time_trial-5829d1b4-1792849381`, `mastery-8016523b-1792849395`
  -> `fixtures/metadata/d24-{single_solve,time_trial,mastery}.json` (replay 64/64 clean).

## Incident
The lane agent unpacked the TT/mastery evidence zips into its scratchpad on the nix-shell tmpfs for that survey, and
the disk filled up (ENOSPC). The coordinator wrote these fixtures and notes and made the commit. Write survey output
under the worktree's `runs-d24/`, not a tmpfs scratchpad, and avoid unpacking whole mastery bundles.
