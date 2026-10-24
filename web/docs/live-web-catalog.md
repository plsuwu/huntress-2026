# Live web catalog observations (2026-10-22)

Read-only authenticated `op=list&lane=web&all=true` returned exactly **24 routes**
for **8 challenge families**, all with lane `web-exploitation`. The OpenAPI
description confirms both spellings as accepted filters. The initialized client
now uses the canonical `web-exploitation` spelling. No all-lane query ran.

| Release day (`day`) | Curriculum (`curriculum_day`, id) | Title | Base id |
|---|---|---|---|
| 19 | 01 | Back Up a Minute | web-exploitation-day-01-backup-exposure |
| 19 | 02 | Permission to Slip | web-exploitation-day-02-idor |
| 19 | 03 | Query Me This | web-exploitation-day-03-sqli |
| 20 | 04 | Stay in Your Directory | web-exploitation-day-04-lfi |
| 20 | 05 | Administratively Speaking | web-exploitation-day-05-administratively-speaking |
| 21 | 06 | Reflect Upon This | web-exploitation-day-06-reflect-upon-this |
| 21 | 07 | Stored for Later | web-exploitation-day-07-stored-xss |
| 21 | 08 | Session Impression | web-exploitation-day-08-session-impression |

Each base has single_solve, /time_trial and /mastery routes. All observed trial
and mastery rows advertise run_for_time_supported=true, permitting regular
untimed play. Single rows explicitly declare single_solve and have no ranked
pool. Proof flags and player identity were removed from the permanent fixture.
Solved booleans remain as catalog metadata; they are never solver answers.

The release day is not the curriculum day. For example, a lane-filtered
`op=list&day=21&lane=web-exploitation` returns nine routes in three families,
whose ids contain 06, 07 and 08. Multiple families may also share the same
curriculum number with different slugs; the full base id remains the key.

Live statuses confirmed a solved response with attempt_id, answer_ids,
accepted_ids, questions, deadline=null and artifacts=[], and an HTTP 409
start_required response that still includes the public questions. Questions
identify themselves with id (for example proof-1), question, artifact="Target 1",
answer_format and normalization. Never derive answer keys from those labels:
the current start response's answer_ids remain authoritative.

Catalog guidance says that web attempts return live sites in targets[], have no
downloadable artifact, report preparing until the target is ready, and stop an
untimed target after 30 minutes without activity. The OpenAPI schema documents
switch_target and reset, both excluded here to protect other workers' targets.
We did not issue any POST, start a target, inspect target pages, submit answers,
or run a real model worker during this validation.

This page is a dated observation record. The live catalog refreshes through the
driver's route cache (`workers/web/routes-dNN.json`); the sanitized captures and
the test suite that consumed them were removed with the old pipeline on
2026-10-24.
