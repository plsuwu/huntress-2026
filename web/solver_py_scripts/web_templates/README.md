# Web solver templates

Built from the pattern scan in [`docs/web-solver-templates.md`](../../docs/web-solver-templates.md):
every web `solve.py` written so far was scanned for what's boilerplate versus
what's genuinely novel per family. These three files are what came out of
that scan that didn't already belong in `recovery_lib.py` proper.

| File | What it is | When to use it |
|---|---|---|
| `starter_solve.py` | The shared all-mode worker solver. `web_rush.py` copies it to each new worker directory as `solve.py`; the worker fills in one function (`rec_this_family`) and reuses the same file for every mode. | Every new family, every mode. This is the default starting point - read it first. |
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

## Automatic hand-off

`web_rush.py` stamps this starter into `workers/web/<base-id>/solve.py` at worker-directory
creation, without overwriting an existing worker-authored solver. The starter reuses root and
clues recon, imports the canonical recovery library, and handles every mode through the same
local `rec_this_family` hook. First-pass partial answers are reused only when the accompanying
metadata matches the current attempt ID and complete target-URL assignment; otherwise they are
re-derived. Workers should not create another HTTP client or mastery dispatcher.
