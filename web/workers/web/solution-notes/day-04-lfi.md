---
day: 4
slug: "lfi"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [lfi, path-traversal, backup-exposure, php-include, arbitrary-file-read, traversal-depth-sweep, absolute-path-fallback, endpoint-discovery, randomized-paths, target-classification]
---

## Solve record

## LFI — "Stay in Your Directory" (web-exploitation-day-04-lfi)

App **"Runbook Nook"** — plain PHP (`X-Powered-By: PHP/8.3.26`) on real Apache. `GET /` 302-redirects to
a viewer page (`/reader.php`, name randomised per variant — also seen `/notes.php`). The viewer takes a
`doc` query param and **includes a file from its pages directory**, e.g. `/reader.php?doc=about.php`. Runbook
links in the landing HTML disclose both the viewer endpoint and the param name.

**Exploit (generic):** traverse out of the pages dir to the mounted proof file named in the prompt:

```
GET /reader.php?doc=../../../run/huntress/objective.txt
```

The included plain-text file is echoed into the `<article class="card">` document pane. Scrape
`proof_<32 hex>`. No filtering observed — raw `../` works, no need for encoding tricks / php filters /
null bytes. **Pages-dir depth varies per variant**, so the solver sweeps `../`×1..8 (+ an absolute-path
fallback) and stops on the first hit — never hard-code a depth. Proof file path is derived from the
start-response prompt (`mounted at (/\S+)`), not hard-coded.

### Solve record (2026-10-20) — all three modes CORRECT (untimed regular)
- **single_solve: CORRECT** flag `5594e708d4bb4f5343ae2489160649a0`. Endpoint `/reader.php`, depth 6,
  proof `proof_9f7ab6b596de470d8ed1294bdee12aac`. (Confirmed by hand first, then codified.)
- **time_trial: CORRECT** flag `396a9edf260b886bb3a976e4b0257545`. depth 3, proof
  `proof_38f8678273304771a32f65d3905c57b1`.
- **mastery: CORRECT** flag `b4ee63aea6ff4983db65f9a2d0e4bd4c`. **2 mixed sets (cumulative, confirms the
  day-19 finding):** set1 = backup-exposure target (`/deploy-444d0/site-config-7d716f.json.bak` →
  `proof_33562c7377ff4311a7b7ce78c35b89e4`); set2 = LFI target with endpoint randomised to `/notes.php`
  (depth 3 → `proof_70b309930a664add93c3c6f874db39cf`). The generic endpoint/param derivation + sniffer
  routed both with no hard-coding.

Ranked/HoF (`run_for_time`) left for the coordinator, matching day-19 convention.

## Shared lane background

Released alongside administratively-speaking as one batch (event day 20, lane-internal days 04-05) - this section is shared verbatim across both days' notes.

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
