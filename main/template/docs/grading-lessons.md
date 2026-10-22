# Grading lessons

Game Master and grader behaviour that cost attempts, and the procedure that came out of each case. Per-challenge
details live in the notes files. Each new lesson: what happened, the evidence, the rule, the code that implements it.

## Lessons that held across events (keep unless disproven)

- **Partial submits are an oracle on untimed attempts only.** On a timed attempt a wrong full submit returns just
  "incorrect"; reproduce on an untimed variant or the practice continuation of the same artifact.
- **Group by family first, then by field.** One partial submit per question family localises a wrong kind in a
  handful of submits. A wrong field masks the others in a partial submit, so probe vocabularies one field at a time.
- **The artifact's own instruction beats the declared answer format.** If decoded text says "submit this exact answer:
  X", submit X whole, even when it breaks the stated pattern. Truncating to fit the format has been graded incorrect.
- **Keep whitespace verbatim.** Strip only line terminators; leading spaces have been significant.
- **Single-solve undersamples the vocabulary.** Pre-support every variant seen on earlier days. Treat the first timed
  run as a likely fixture-collection run.
- **Ranked and regular pools can differ.** A quirk may appear only in ranked variants and never reproduce untimed.
- **Graders change without notice.** Gate the new rule on the attempt's start time, not the calendar day, so old
  fixtures keep replaying. Old notes are hints, not truth.
- **Prerequisites:** timed modes may refuse to start until single-solve (or an earlier mode) is CORRECT. A refused
  start spends nothing.
- **Investigating a wrong ranked result:** replay it with the old and new binaries (identical answers rule out a
  regression), run an independent checker over the failure and every saved graded run of that family, and look for a
  property present only in the failures.

## Event-specific lessons

<!-- ### <Lane/kind>: <one-line summary> (YYYY-MM-DD)
     - Evidence: ...
     - Rule: ...
     - Code: ... -->
