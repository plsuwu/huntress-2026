# Picture This (day 04, metadata-forensics)
- single_solve: 1 question, `jpeg.sof_dimensions_pixels` for camera-001.jpg -> `257x146` (W x H from SOF0; solved, CORRECT).
- Artifact zip is NOT password protected. `artifacts/` holds zip + extracted jpg.
- Prompt hints later modes: a dozen images, renamed/shuffled, find the one whose presentation (EXIF/thumbnail/orientation?) mismatches structure (SOF). Time trial deadline 10s -> needs Rust-speed/auto solver.

## time_trial (solved)
- Same single question per variant (camera-NNN.jpg SOF dims, `WxH`); IDs/image name change per variant, so derive both from the start response.
- Deadline is only 10s: start/retry -> download -> parse -> submit runs in one script (~1s). Do not poll by hand.
- `python3 -I solve.py time_trial --retry --submit` (needs HUNTRESS_GM_TOKEN). Use `start` (no --retry) only to reconnect to a live attempt.
- First `start` was used just to read the question format and expired (cost 1 of 32 variants; not HoF-relevant).
- Quirk: a `retry` right after expiry returned the old attempt's data once (stale deadline, submit => `expired`); a second `--retry` advanced. If output shows a past deadline, retry again.
- Script prints only the first ~300 chars of the submit response; flag is truncated away - widen if the flag is needed.
- Mastery: not yet looked at; list shows 2 sets. Prompt hints at "dozen images, find the one whose presentation mismatches structure" (likely EXIF orientation/dimension vs SOF).

## mastery (solved, flag 50f55bf0e24873374bfc49d25ac85557)
- Question set is NOT fixed: early variants were 2 questions (camera SOF dims, one per set); a later variant had 16 (per set: evidence-note filename/size/md5/sha1/sha256/magic-16/utf8-bom + camera SOF dims). Plan for varying kinds; `solve.py` dispatches on question-id suffix and prints UNHANDLED for unknown kinds.
- Archive is nested: outer zip -> metadata-day-04-evidence-0N.zip -> evidence-note.txt, camera-001.jpg. `artifact` field may be prefixed "delivery 1: ".
- Question ids are prefixed `setN.`; use the exact ids from the response.
- `retry` returns the next variant with a live 10s clock but the response can lack `download` (when the previous attempt was already expired/just replaced) - re-run `--retry`. I burned ~3 mastery variants discovering the format; solver now runs end-to-end in ~1s.
- Run: `python3 -I solve.py mastery --retry --submit`. Latest artifact saved to `artifacts/<mode>-latest.zip`.
- jpeg SOF `none` is the literal answer if no SOF marker; BOM = bytes start EF BB BF.
- Open question for the "find the mismatching image" prompt text: never appeared as a question in any variant seen (only SOF dims).
