# Web solver templates

Built from the pattern scan in [`docs/web-solver-templates.md`](../../docs/web-solver-templates.md):
every web `solve.py` written so far was scanned for what's boilerplate versus
what's genuinely novel per family. These three files are what came out of
that scan that didn't already belong in `recovery_lib.py` proper.

| File | What it is | When to use it |
|---|---|---|
| `starter_solve.py` | The drop-in worker starter. Copy it to your worker directory as `solve.py`, fill in one function (`rec_this_family`), run it against a mode. | Every new family, every mode. This is the default starting point - read it first. |
| `graphql_helpers.py` | A thin `graphql()` POST wrapper (built on `recovery_lib.Target`) plus a standard introspection query and a helper to list a schema's Query fields. | Only if `/api/clues` (or the app itself) points you at a GraphQL endpoint. Not wired into `classify_and_recover` - only one GraphQL-shaped bug has been confirmed so far (day-18-resolver-revolt), so this is scaffolding for discovering the *next* one's shape, not a proven recoverer. |
| `poll_patterns.py` | `poll_for_proof()` - a bounded GET-and-retry loop for a genuinely asynchronous bug (something else visits a URL on its own schedule; you can't just fetch a second route synchronously). | Only if your family's bug needs to wait on an out-of-band actor (a bot, a webhook, a queued job) rather than an immediate second request. Session fixation and reflected-XSS-callback are both synchronous two-step patterns and don't need this - only `rec_stored_xss_bot`'s shape does. |

## Relationship to `recovery_lib.py`

`recovery_lib.py` is the canonical, already-proven reference: every `rec_*`
function in it has been graded CORRECT at least once, most of them (backup
exposure, IDOR, SQL injection) many times over. These templates are one level
below that: mechanisms worth having ready, not yet proven enough times to
earn a permanent slot in the dispatcher. When one of these patterns gets
confirmed against a second independent app, fold it into `recovery_lib.py`
the same way `rec_lfi` and `rec_sqli_group_close` were promoted on
2026-10-25 - don't leave a twice-proven pattern sitting in a template file.

## What's deliberately NOT templated here

Per the scan's Tier-3 findings - proven exactly once, too app-specific to
generalize usefully yet:

- **CSRF header-forge** (spoofed `Origin`/`Referer`/`Sec-Fetch-Site`) - see
  `recovery_lib.rec_csrf` for the exact header set; copy that, don't
  re-derive it.
- **Upload content-type/extension bypass** - see `recovery_lib.rec_upload_bypass`;
  which filename extension routes where is app-specific.
- **Command-injection chaining syntax** - plain `;`-chaining works, `$(...)`
  substitution gets rejected server-side on every target seen
  (`{"error":"diagnostic_failed"}`); see `recovery_lib.rec_command_injection`'s
  `INJECTION_SUFFIX`-style suffix for the exact form.

If any of these get reconfirmed against a second independent app, that's the
signal to promote them into a template here (or straight into `recovery_lib.py`,
if they're already a complete recoverer like `rec_lfi` was).

## Future: automatic hand-off

Today, a worker copies `starter_solve.py` in by hand. The natural next step
(not yet built) is for `web_rush.py`'s brief-writing step to stamp a
per-family copy of this starter directly into `workers/web/<base-id>/` at
dispatch time - pre-filled with that family's already-fetched `/api/clues`
response as a comment, so the worker doesn't even need the first GET to see
what it's working with.
