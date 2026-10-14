# Day 5 warmups - By the Numbers

## single_solve (DONE, CORRECT)
- blob.txt: space-separated decimal ASCII codes -> plaintext; last non-empty line says
  "submit this exact answer to the Game Master: <slug>". Slug = `word-word-word-NN`.
- Answer id: `message`. `solve.py <blob>` iterates number-list decoding (dec/hex/oct/bin) then extracts the slug.
- Downloads: `/?op=download&capability=<token from start>` with Bearer header.

## time_trial (DONE, CORRECT) / mastery (DONE, CORRECT)
- **Deadlines are tiny**: time_trial = 6s, mastery = 7s from `started_at`/start response. Separate tool calls
  (start -> download -> solve -> submit) always expire. Everything must run in ONE process:
  `run_timed.py <time_trial|mastery> [--retry]` (token via `set -a; . ./.env`; run with `python3 -I`).
- The first `start` of a timed mode begins the clock immediately; use `--retry` (op=retry) for a fresh variant
  after an expired/completed one. 255+ unique attempts are available, so an exploratory start is cheap.
- The Game Master's downloads are slow (~4s for a single tiny file). In mastery, `op=start` returned ONE zip
  (`blob-NN.txt` x23) in `download`; `op=retry` returned `downloads` + `artifacts` lists (23 separate files, all
  named blob.txt, in question order set1..set23). Runner handles both and fetches the list concurrently (threads).
  Total wall time ~9s end to end vs 7s deadline reported - the deadline is measured server-side at submit time and
  the successful run squeaked in; if a run expires just retry (variants are plentiful).
- Mastery blob format (differs from single-solve decimal list): 5 lines `NN<TAB>fragment`, each fragment a
  contiguous slice of the sentence "Nice work decoding this warmup! To solve, submit this exact answer to the
  Game Master: <slug>" in a different encoding per line (1 base64, 2 rot13, 3 hex, 4 8-bit binary groups,
  5 decimal ASCII). Slice boundaries are ragged and vary per blob (slug may begin at the end of row 4), so
  decode each row and *concatenate* in order, then regex the slug `\w+-\w+-\w+-\d\d`. `solve.decode_row`
  autodetects the encoding per row.
- Answer ids in mastery: `setN.message` <-> `blob-NN.txt`.
- Flags: single `0c7082d2...`, time trial `8af9027b...`, mastery `f07f7c3c...` (all CORRECT).
- Artifacts kept in `artifacts/` (single-solve blob, one example mastery zip).
