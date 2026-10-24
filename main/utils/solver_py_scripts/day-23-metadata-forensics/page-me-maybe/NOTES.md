# Day 23 metadata - Page Me Maybe (`metadata-forensics-day-23-page-me-maybe`)

Prompt: "Application databases have been copied, truncated, embedded, and mislabeled. Validate page geometry,
connect databases to parent applications, and find the impossible claim." As usual, none of the narrative became a
question: single-solve asks one new kind, TT is the cumulative bundle (28 q), and mastery has 7 sets (196 q).

## New kind: `d23-sqlite-001-sqlite-page-size-bytes`
- Graded: the SQLite header page size (big-endian u16 at offset 16, stored `1` = 65536), base-10. Single 4096,
  TT 32768.
- First-pass bug: the generic `-size-bytes` HANDLERS row matched `...-page-size-bytes` and answered the file length
  (12288 = 3 pages x 4096). Fixed by `sqlite_answer` (dispatches on `-sqlite-<field>`), which runs before HANDLERS.
- `ctf-run/src/formats/sqlite.rs`: full 100-byte header, embedded-magic offset, truncation check (header page count
  x page size vs bytes present), sqlite_master/table b-tree walk (overflow chains followed). Only `page-size-bytes`
  has been graded. The other `sqlite_answer` field names (page-count, text-encoding, application-id, truncated,
  header-offset, table-names, ...) are guesses at what the prompt implies. An unknown field fails with a full
  header+schema FACTS dump.

## Grading lesson: `zip-central-directory-member-names` is SORTED
- The d07 kind in the TT bundle had 8 members whose stored CD order was not sorted. Stored order was graded incorrect
  (grouped oracle: `d07-zip/names` was the only wrong field). A partial submit of the codepoint-sorted list was
  CORRECT. Every earlier fixture happened to be stored in sorted order, so this never showed up before. The
  `answer_format` says "exact values and order", which does not settle it.
- The handler now sorts. Mastery (196 answers) then went CORRECT first try.

## Runs (all untimed)
- single_solve CORRECT (`runs-d23/.../single_solve-0c037e32-1792826992`)
- time_trial: regular attempt incorrect (stored-order names). The single-field partial submit then completed it:
  CORRECT. The later re-run reports `already_solved`, so there is no TT fixture.
- mastery CORRECT (`mastery-abd5a872-1792827115`) -> `fixtures/metadata/d23-mastery.json`
