# Lane patterns and per-day predictions

How each lane evolves from day to day, written by the lane agents after each release and read by them before the
next one. A good prediction lets an agent pre-support tomorrow's twist before the release.

## Web lane

Live list/status/OpenAPI metadata was inspected read-only on 2026-10-22; see
docs/live-web-catalog.md. Eight families span release Days 19–21, with curriculum
numbers 01–08. No target pages or newly started assignments were inspected.

The restored historical solutions now supply graded observations for curriculum
01–05. Read [web-worker-lessons.md](web-worker-lessons.md) for the worker brief
and source map. Mastery is cumulative across earlier bug types: inspect every
target, rather than using only the method named by the worker's challenge route.
Known earlier families are backup exposure, IDOR, SQLi, PHP LFI and Django
per-route authorization bypass. Admin clues must be distinguished before generic
API/IDOR signs; a form alone is insufficient to classify a new target as SQLi.

The web lane uses assigned live sites in targets[], with no downloadable
artifacts. Workers must document observed formats and exploit families in each
job's NOTES.md; summarize reusable patterns here after genuine grading evidence.
Mode names do not establish timing: all non-single modes require an advertised
untimed pool, and existing timed attempts are held. See docs/api-contract.md for
the live catalog/status contracts and the restored historical ready-response
shape (active; targets have id/url). No new targets were started for this review.

## Per-lane summary

<!-- ### <lane>: <one-line character>
- Single/TT: what the artifact is and what is asked.
- Mastery/multi-set: how sets are built.
- Day-by-day pieces: d1 ..., d2 ..., ...
- Gotchas: ...
- Prediction for the next day: ... -->

## Day N outcome (YYYY-MM-DD)

<!-- | lane | the day's twist | what the solver needed | -->
