---
day: 5
slug: "administratively-speaking"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [auth-bypass, trailing-slash-bypass, backup-exposure, broken-access-control, route-authorization, django, route-discovery, randomized-paths, target-classification, classifier-ordering]
---

## Solve record

## Admin bypass — "Administratively Speaking" (web-exploitation-day-05-administratively-speaking)

App **"Django Dispatch"** — a Django workspace where the current role is `analyst` and admin actions are
meant to be off-limits. The server-side authorization check is wired to the **canonical (trailing-slash)
route only**, so the un-slashed variant of a feature route walks straight past the gate.

**Exploit (generic):**
1. `GET /api/clues` → `{"framework":"Django","feature_route":"/<route>","current_role":"analyst"}`.
2. `GET /<route>` **with no trailing slash** → `{"ok":true,"effect":"framework_authorization_bypassed",
   "evidence":"proof_<hex>"}`. The slashed path (`/<route>/`) returns `{"error":"not_found"}` (404).

Scrape `proof_<hex>` from `evidence`. Route name is randomised per variant (`/dispatch-…`, `/process-…`)
so it's always read from `/api/clues`, never hard-coded. `rec_admin` must sniff **before** `rec_idor`
because both expose `/api/` links (admin's tell is `href="/api/clues"` / a `feature_route` field).

### Solve record (2026-10-20)
- **single_solve: CORRECT** — solved by the other agent (focused solver, now
  `workers/web/past-solutions/day-05-administratively-speaking-single-solve-focused.py`).
  proof `proof_1348f04d6c9a43ad8259e955b1718ac3`.
- **time_trial: CORRECT** flag `511bf0fd18d46c41566fc913ec90ff56`. route `/dispatch-652ae5` →
  `proof_4f0c4bfe61bf49e29b0ce49763267192`.
- **mastery: CORRECT** flag `8d0957991458a21bace8c647a5b62b14`. **2 mixed sets:** set1 = backup-exposure
  (`/snapshots-75512/environment-copy-2548af.json.bak` → `proof_5f4f7e1861e4461a8ae7730c10ca06c6`);
  set2 = admin-bypass (route `/process-f0cc42` → `proof_6eabf04483e64af18e949759e8d4f7de`). Unified
  sniffer routed both with no hard-coding.

Ranked/HoF (`run_for_time`) left for the coordinator.

## Note: two different solvers for this one challenge

`single_solve` was solved by a separate, earlier agent/session using a different, single_solve-focused script: `past-solutions/day-05-administratively-speaking-single-solve-focused.py`. `time_trial` and `mastery` (mastery mixes bug types, so it needs the day-04/05 unified solver) were solved later using the shared `past-solutions/day-04-05-shared-{solve,common,gm}.py` trio. Both solved the same underlying Django trailing-slash authorization bypass; they are not the same script.

## Shared lane background

Released alongside lfi as one batch (event day 20, lane-internal days 04-05) - this section is shared verbatim across both days' notes.

## Day 20 release — web-exploitation lane (days 04 & 05)

Event Day 20 (2026-10-20) added **two** new web-exploitation challenges. The lane keeps its own day
counter (now 01–05), so they do NOT appear under `op=list&day=20` — only via `op=list&all=true`
(`lane="web-exploitation"`). 6 new routes = 2 challenges × 3 modes.

| slug | id | title | prompt gist | bug class |
|---|---|---|---|---|
| lfi | `web-exploitation-day-04-lfi` | 🌐 Stay in Your Directory | viewer should stay in its pages dir; proof mounted at `/run/huntress/objective.txt` — cross the boundary | **LFI / path traversal** |
| administratively-speaking | `web-exploitation-day-05-administratively-speaking` | 🌐 Administratively Speaking | "staff-only door with no lock — just a sign; walk through like you own the place" | **Django per-route authz bypass** (trailing-slash gap) |

> Scope note: day-04-lfi (all 3 modes) and day-05 admin-bypass TT + mastery were solved here. Day-05
> **single_solve** was solved by a separate agent whose focused solver now lives at
> `workers/web/past-solutions/day-05-administratively-speaking-single-solve-focused.py` (at the time
> this was written, a different dir using the lane's own day counter `day-05`, vs. this writeup's
> event day `day-20` - both now consolidated here and named by the challenge's actual day, 05). That
> focused solver does the admin-bypass only; **mastery needs the unified `solve.py` here** because
> mastery mixes bug types.

Mode config (both challenges, identical to day 19): single_solve `rft=false` reg_pool=1; time_trial
`rft=true` reg/ranked_pool=16, 1 set, 120 s ranked clock; mastery `rft=true` pool=8, **2 sets**, 120 s.
Day-17 `prerequisite_required` gating applies (single must be CORRECT before TT/mastery).
### Solver layout (Python-only, stdlib — no `requests`/venv needed)
- `common.py` — GM client + paced urllib target client + per-bug recoverers
  (`rec_lfi`/`rec_admin`/`rec_sqli`/`rec_idor`/`rec_backup`) + `sniff()` landing-page classifier. The
  backup/idor/sqli recoverers are ported from day-19's `solve_lane.py`; `rec_lfi` + `rec_admin` are new
  this day. **Mastery's cumulative mixing is covered** (a day-05 mastery target can be any of
  backup/idor/sqli/lfi/admin). Pacing 0.8 s, 20 s timeouts, no wordlist fuzzing.
- `solve.py <start|status|solve> <challenge-id> [--run-for-time]` — starts (untimed regular), polls while
  `preparing`, sniffs + recovers each `targets[]` entry per question, submits all proofs in one batch under
  the exact `answer_ids`. `run/` holds the raw GM json + per-target fetch dumps.
- Run under `python3 -I` (untrusted-target hygiene): `solve.py` inserts its own dir on `sys.path` so
  `import common` still resolves.

#### `sniff()` priority (landing page)
`admin` (`href="/api/clues"` or a `feature_route` field) → `lfi` (a `.php?param=` link, or `href=…reader…`)
→ `sqli` (`<form action>`) → `idor` (`href="/api/"`) → `backup` (fallback: robots.txt → open dir index →
`*.bak`). `admin` must precede `idor` — both expose `/api/` links, but admin's tell is the `/api/clues`
notes endpoint.

### Carry-forward for future web days
- Lane day counter is independent of the event day; always find new web routes via `op=list&all=true`.
- Mastery is cumulative over all bug types up to the challenge's day → keep extending `common.sniff` +
  recoverers, don't write per-bug-only mastery solvers.
- Per-variant randomisation seen so far: directory/file names (backup), API/path names (idor), form
  action/param (sqli), **viewer endpoint name + traversal depth (lfi)**. Derive everything from the live
  response; the proof file path comes from the prompt text.
- Network from this environment is slow-ish; use generous curl/urllib timeouts (≥30 s) and expect the
  occasional read timeout on the GM host.
