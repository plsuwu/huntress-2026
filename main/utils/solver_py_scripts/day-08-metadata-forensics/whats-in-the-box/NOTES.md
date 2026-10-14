# What's in the Box? (day 08, metadata-forensics)
Challenge id: `metadata-forensics-day-08-whats-in-the-box` (+ `/time_trial`, `/mastery`). Prompt: "inventory every stored
path, preserve directory context, identify the archive whose names reveal an impossible provenance claim".

## Status
| mode | how | flag |
| --- | --- | --- |
| single_solve | regular | 725407343334f407447728165c8d0b1f |
| time_trial | regular untimed start (no run_for_time) | 305ee4fe8e66a8a519dfacf5634f753d |
| mastery | regular untimed start, 5 sets | 3797f689f154de6fc752ed9ed1beefe1 |
Untimed regular start of time_trial/mastery completes the mode (run_for_time not required). Timed pool: 32, deadlines 8 s (TT) / 10 s (mastery).

## Observations
- Bundle recap of days 04-07: 12 questions per set: evidence-note.txt (filename/size/md5/sha1/sha256/magic16/utf8-bom),
  jpeg SOF dims (`WxH`), png IHDR dims (`WxH`), pdf header version, zip EOCD total entries, zip central-directory member names
  (compact JSON array, stored order, e.g. `["room-01/item-1.txt","room-02/item-1.txt"]`).
- Every variant seen (single, TT, 5 mastery sets) had identical media/zip content (257x146, 269x153, 1.4, 2 entries, room-0N/item-1.txt);
  only the evidence note differs ("Case <hex>", 33 B). No traversal/"impossible provenance" names appeared yet - a later variant might
  (e.g. `../`, absolute, or duplicate names); `cd_names()` reads raw CD names without normalisation, nested_zip drops dirs for outer members only.
- Mastery ids are `setN.<id>`; stripped in `answer()`.
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--timed] [--retry] [--submit]`.
- Timed time_trial (retry + run_for_time) also CORRECT in one invocation (~1s); 31 attempts remain. Mastery timed not run.
