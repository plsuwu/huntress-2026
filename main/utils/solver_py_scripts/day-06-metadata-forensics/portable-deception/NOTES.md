# Portable Deception (day 06, metadata-forensics)
Challenge id: `metadata-forensics-day-06-portable-deception` (+ `/time_trial`, `/mastery`). Prompt: "document bundle mixes real
PDFs, renamed impostors, and multiple declared versions - what the files promise, what they contain, which promise is false".

## Status: all three modes CORRECT
| mode | flag | question(s) seen |
| --- | --- | --- |
| single_solve | 5824dfea3582a58fe316ae850b847d5d | `d06-pdf-001-pdf-header-version` -> `1.4` |
| time_trial | 5e79af503b572ba181da113d89faf78e | `d06-pdf-002-pdf-header-version` -> `1.5` |
| mastery | 36ac8c01b7edaa4f0710653c45360ae1 | `set1.d06-pdf-003-...` -> `1.6`, `set2.d06-pdf-004-...` -> `1.7` |

## Observations
- Only kind seen: `pdf.header_version`, answer_format exact_text, normalization trim. Answer = `x.y` from the first `%PDF-x.y`
  (no `%PDF-` prefix). `none` is the literal for absent. Each artifact is one tiny (~1.5 KB) clean structured PDF
  (`document-NNN.pdf`, producer `structured_pdf.v1`), no /Version override, one %%EOF - the "impostor / false promise" framing
  never became a question in the variants seen.
- Zip not password protected; single/time-trial download is a plain zip named `metadata-day-06-evidence.zip`, mastery nested -
  `nested_zip.py` handles both.
- Deadlines: time trial 8s, mastery 9s (pool 32 each). `solve.py <mode> --submit` goes straight from first `start` to submit
  in ~1s, so no exploratory start was needed; all modes passed on the first attempt (0 variants burned).
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--retry] [--submit]`.
  Submit is only sent when every question had a handler.

## If later variants add kinds
`HANDLERS` (suffix match after stripping `setN.`) covers header version, catalog /Version, page count, EOF count, Info
fields, size, md5/sha1/sha256, magic16 - the non-header ones are GUESSES for plausible kinds, unverified. Unknown kinds print
`UNHANDLED` plus a `FACTS` dump (header, catalog versions, EOF count, startxref, Info dict) to stderr. Likely follow-ups:
header-vs-catalog version disagreement (last /Version wins after incremental update), renamed impostor (e.g. non-%PDF magic
inside a .pdf name -> header_version `none`). Extend HANDLERS rather than rewriting. Latest artifacts in `artifacts/`.
