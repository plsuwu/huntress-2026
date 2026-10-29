# Web solution index

Read `docs/web-worker-lessons.md` first for the accumulated bug-class curriculum and
operational gotchas. Use this index to find a prior note and solver script worth
copying or adapting. Notes live in this directory; solver scripts live in
`../past-solutions/`. Shared scripts are listed under every day they cover.

Search note tags from this directory with:

```sh
rg -n -g 'day-*.md' '^tags:.*(ssrf|traversal|policy|audit)' .
```

| Curriculum day | Slug | Note file | Primary tags | Solver scripts |
|---:|---|---|---|---|
| 1 | backup-exposure | `day-01-backup-exposure.md` | backup-exposure, directory-listing, robots-txt | `day-01-backup-exposure-solve.py`, `day-01-02-03-shared-solve_lane.py` |
| 2 | idor | `day-02-idor.md` | idor, ownership-bypass, activity-feed-disclosure | `day-02-idor-solve.py`, `day-01-02-03-shared-solve_lane.py` |
| 3 | sqli | `day-03-sqli.md` | sqli, boolean-sqli, group-closing | `day-03-sqli-solve.py`, `day-01-02-03-shared-solve_lane.py` |
| 4 | lfi | `day-04-lfi.md` | lfi, path-traversal, php-include | `day-04-05-shared-common.py`, `day-04-05-shared-solve.py`, `day-04-05-shared-gm.py` |
| 5 | administratively-speaking | `day-05-administratively-speaking.md` | auth-bypass, trailing-slash-bypass, django | `day-05-administratively-speaking-single-solve-focused.py`, `day-04-05-shared-common.py`, `day-04-05-shared-solve.py` |
| 6 | reflect-upon-this | `day-06-reflect-upon-this.md` | reflected-xss, browser-callback, header-spoofing | `day-06-reflect-upon-this-single_solve.py`, `day-06-reflect-upon-this-time_trial.py`, `day-06-reflect-upon-this-mastery.py`, `day-06-07-08-shared-solve.py` |
| 7 | stored-xss | `day-07-stored-xss.md` | stored-xss, moderator-bot, same-origin-exfiltration | `day-07-stored-xss-single_solve.py`, `day-07-stored-xss-time_trial.py`, `day-07-stored-xss-mastery.py`, `day-06-07-08-shared-solve.py` |
| 8 | session-impression | `day-08-session-impression.md` | session-fixation, client-chosen-session, cookie-replay | `day-08-session-impression-single_solve.py`, `day-08-session-impression-time_trial.py`, `day-08-session-impression-mastery.py`, `day-06-07-08-shared-solve.py` |
| 9 | forged-with-care | `day-09-forged-with-care.md` | csrf, state-changing-get, header-spoofing | `day-09-forged-with-care-single_solve.py`, `day-09-forged-with-care-time_trial.py`, `day-09-forged-with-care-mastery.py`, `day-09-10-shared-solve.py` |
| 10 | uploading-expectations | `day-10-uploading-expectations.md` | file-upload-bypass, template-injection, extension-bypass | `day-10-uploading-expectations-single_solve.py`, `day-10-uploading-expectations-time_trial.py`, `day-10-uploading-expectations-mastery.py`, `day-09-10-shared-solve.py` |
| 11 | template-temper | `day-11-template-temper.md` | ssti, spel-injection, spring-boot | `day-11-template-temper-single_solve.py`, `day-11-template-temper-mastery.py` |
| 12 | ask-again-later | `day-12-ask-again-later.md` | second-order-sqli, stored-input, suppressed-errors | `day-12-ask-again-later-single_solve.py`, `day-12-ask-again-later-time_trial.py`, `day-12-ask-again-later-mastery.py` |
| 13 | posted-too-much | `day-13-posted-too-much.md` | mass-assignment, privilege-escalation, unsafe-binding | `day-13-posted-too-much-single_solve.py`, `day-13-posted-too-much-time_trial.py`, `day-13-posted-too-much-mastery.py` |
| 14 | the-api-has-opinions | `day-14-the-api-has-opinions.md` | field-level-authorization, action-authorization, export-bypass | `day-14-the-api-has-opinions-solve.py` |
| 15 | fetch-quest | `day-15-fetch-quest.md` | ssrf, loopback-access, port-discovery | `day-15-fetch-quest-solve.py` |
| 16 | shell-company | `day-16-shell-company.md` | command-injection, shell-command-chaining, arbitrary-file-read | `day-16-shell-company-solve.py` |
| 17 | reset-expectations | `day-17-reset-expectations.md` | password-reset, token-account-confusion, missing-token-binding | `day-17-reset-expectations-solve.py` |
| 18 | resolver-revolt | `day-18-resolver-revolt.md` | graphql, field-level-authorization, resolver-authorization | `day-18-resolver-revolt-solve.py` |
| 19 | trust-the-webhook | `day-19-trust-the-webhook.md` | webhook-forgery, unsigned-webhook, payload-minimization | `day-19-trust-the-webhook-solve.py`, `day-19-trust-the-webhook-mastery.py` |
| 20 | serialized-offender | `day-20-serialized-offender.md` | unsigned-state-tampering, client-side-state, role-tampering | `day-20-serialized-offender-solve.py`, `day-20-serialized-offender-mastery.py`, `day-20-serialized-offender-recon.py` |
| 21 | double-spent | `day-21-double-spent.md` | race-condition, double-spend, one-shot-state | `day-21-double-spent-solve.py`, `day-21-double-spent-mastery.py` |
| 22 | socket-to-me | `day-22-socket-to-me.md` | websocket, unauthorized-subscription, parameter-discovery | `day-22-socket-to-me-solve.py`, `day-22-socket-to-me-mastery.py` |
| 23 | forwarded-misgivings | `day-23-forwarded-misgivings.md` | header-spoofing, reverse-proxy-trust, x-forwarded-for | `day-23-forwarded-misgivings-solve.py` |
| 24 | fail-open-friday | `day-28-fail-open-friday.md` | fail-open, policy-decoder, error-handling | `day-28-fail-open-friday-solve.py` |
| 25 | if-a-breach-falls-in-a-log | `day-28-if-a-breach-falls-in-a-log.md` | missing-audit-log-event, broken-logging, security-event-suppression | `day-28-if-a-breach-falls-in-a-log-solve.py`, `day-28-if-a-breach-falls-in-a-log-mastery.py` |
| 26 | trust-the-build | `day-29-trust-the-build.md` | unverified-update, dependency-tampering, integrity-check-bypass | `day-29-trust-the-build-solve.py`, `day-29-trust-the-build-mastery.py` |

Note: the Day 28 and Day 29 file names use the release day, while their frontmatter and this
index use the embedded curriculum day (24 through 26). Always trust the note's
frontmatter `day`/`slug` and the challenge's own route id over the file name.
