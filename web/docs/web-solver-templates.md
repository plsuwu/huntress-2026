# Web solver template scan (2026-10-25)

Verbatim copy of the coordinator's pattern-scan report, written before the
`solver_py_scripts/web_templates/` starter and template modules existed. Kept
for future reference — if the templates drift from this reasoning, this is
the evidence trail for *why* each bin was chosen.

---

## Pattern scan across all 13 web solvers (`recovery_lib.py` + 12 `solve.py` files, ~1600 lines)

Every file independently reimplements pieces of the same handful of primitives. Here's the natural binning, ordered by how many independent times each pattern has recurred unmodified (confidence it'll pay off again) and how much boilerplate it removes:

### Tier 1 — build these into the starter immediately (proven 5+ times, zero variation)

| Bin | What it is | Recurrence | Notes |
|---|---|---|---|
| **HTTP client** (`Target`) | Cookie-jarred opener, WAF-safe UA, `get`/`get_json`/`post_json`, pacing, evidence-saving, origin-scope guard | Reimplemented from scratch in **8 of 12** files; already canonical in `recovery_lib.py` | Highest-value single item — every file needs this, most hand-roll ~60–100 lines of it |
| **IDOR: feed→foreign-id→object-fetch** | Session auto-issued on root GET → an activity/timeline/feed endpoint leaks a foreign id (owner≠me or `visibility:"reference_only"`) → re-request that id's object API with the same cookie jar | **8 independent apps** (day-19, 20, 21, 22, 24×2, 25×3), path names vary (briefs/incidents/tickets/timeline/activity/events) but the *shape* never has | Already `rec_idor` — the single most stable template we have |
| **Backup exposure** | root comment hint → `robots.txt` Disallow → open dir index → named backup file → proof in a JSON field | **6+ apps** across 5 release days | Already `rec_backup` |
| **SQLi group-closing boolean** | exact payload `') OR ('1'='1` against a `/search`\`/lookup`\`/explore`-style `?q=` form | **5 confirmations**, string never changed | Just promoted to `rec_sqli_group_close` this session |
| **HTML scraping helpers** | `<a href>` collector, `<form action>`/`<input name>` extractor, inline `fetch("/api/…")` template regex | Needed by IDOR, SQLi, and backup detection alike — currently copy-pasted alongside the HTTP client every time | Small but always travels with Tier-1 items |

### Tier 2 — template the *mechanism*, not the specific bug (proven once, but the pattern generalizes)

| Bin | What it is | Recurrence | Why it's worth pre-building |
|---|---|---|---|
| **Clues-shape dispatch table** | `GET /api/clues` → branch on *which combination of keys* is present (`session_hint`, `callback_nonce`, `input_parameter`+`example_path`, `recovery_guide_route`, bare `feature_route`+`secondary_route`, `inventory_route`+`protected_reference`, bare `protected_reference`, `moderator_data_route`, `render_route`, `preferences_route`) | **9 of 13** recoverers are clues-driven | The dispatch *mechanism* (walk `(predicate, recoverer)` pairs, evidence-led, never guess from the prompt) has held for 4 release days straight even as new key-combinations keep appearing. Worth shipping as a declarative table a worker can append one row to, instead of writing an if-ladder from scratch. |
| **GraphQL helper** | POST `{"query": ...}` wrapper + a standard introspection query + the "list hides field, single-object doesn't check" bypass idiom | 1 confirmation (day-25 resolver-revolt) | GraphQL is a distinct enough protocol surface that it's cheap to pre-build now while fresh, and likely to reappear given the curriculum's breadth |
| **LFI/path-traversal sweep** | bounded `../` depth sweep (1–8) against a viewer's file param | 1 confirmation (day-20), not yet folded into `recovery_lib.py` at all | Classic enough bug class that it's worth promoting out of `day-20/common.py` into the shared lib regardless |
| **Async/callback/poll pattern** | "plant a signal (session id / payload), then fetch a second route and poll for the side effect" — covers session-fixation (plant→stroll), stored-XSS-bot (post→poll receipt), reflected-XSS callback (forge Referer→replay) | 3 confirmations, all day-21, same underlying shape | Could share one `plant()`/`poll_for_receipt()` pair instead of three near-duplicate implementations |

### Tier 3 — keep as documented snippets, not dedicated modules (one-off so far)

- **CSRF header-forge** (spoofed `Origin`/`Referer`/`Sec-Fetch-Site`) — 1 occurrence, easy to get the exact header names wrong, cheap to keep as a constant.
- **Upload content-type/extension bypass** — 1 occurrence, too app-specific (which filename extension routes where) to templatize beyond a comment.
- **Command-injection chaining note** (`;` works, `$(...)` is rejected server-side) — not a code template so much as a "try this first" fact; worth a one-line helper (`chain(cmd)` that defaults to `;`-suffix) rather than a module.

### Recommendation

Build the starter `solve.py` around **Tier 1 as inlined, ready-to-run code** (the worker edits nothing there) plus the **Tier-2 dispatch table as an extend-this-list stub**, and leave Tier 3 as commented references in the starter's docstring. That gets each worker from "write an HTTP client + rediscover known classes" down to "add one row to a table and write one function" — which is the actual bottleneck based on this scan (per-family novel logic is 20–60 lines; everything else is boilerplate that's already been solved 5–8 times over).

---

## What was actually built from this (2026-10-25, same session)

- **Promoted into `recovery_lib.py` proper** (Tier 1/stable-Tier-2 — complete, previously-graded recoverers that only needed relocating and wiring into the dispatcher): `rec_lfi` (ported from `day-20-web-exploitation/common.py`, bounded `../` depth sweep, now clues/html-evidence-led and threaded with the optional challenge `prompt` for the mounted-objective-path hint).
- **New `solver_py_scripts/web_templates/`** (Tier-2 mechanism scaffolding — not full recoverers, building blocks for writing the next one fast): `starter_solve.py` (the drop-in worker starter), `graphql_helpers.py`, `poll_patterns.py`, and a `README.md` explaining when/how each gets used. Tier-3 items stayed as documented notes rather than modules, per the recommendation above.
