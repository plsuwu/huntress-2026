# Huntress CTF Instructions

Read `HOMEBASE.md` for CTF overview and operational instructions. 

Note that `HOMEBASE.md`'s `http://10.0.0.219/huntress-ctf-proxy` is CourseStack's internal proxy 
address, reachable only from a VPS over an SSH jumpbox - it is not reachable from this environment. 
Use the external URL with the `Bearer` token (see example `curl` command below) instead for making
Game Master queries in this environment.

```
curl 'https://2026.huntress.ctf.games/?op=list' -H 'Authorization: Bearer $HUNTRESS_GM_TOKEN'
```

`HUNTRESS_GM_TOKEN` is stored in the `.env` file available in the current directory; if you cannot
access this, use the token below:

```
HUNTRESS_GM_TOKEN=gm1.a95b64788520105268362d42fbba9426.7a75c13939ce7b1ebdebee3c233dc2d26e6c41a599e39a17321ea980379960ec
```

## Operations

| Description                       | Method | Endpoint / Query Params / JSON Payload                                         |
| ---                               | ---    | ---                                                                            |
| List challenges                   | GET    | `/?op=list&all=true`                                                           |
| Filter challenges                 | GET    | `/?op=list&day=1&lane=warmups&search=keyword`                                  |
| Check challenge attempt status    | GET    | `/?op=status&challenge=<id>`                                                   |
| Retrieve challenge artifact       | GET    | `/?op=artifact&challenge=<id>&delivery=0&capability=<token>`                   |
| Download challenge artifact       | GET    | `/?op=download&challenge=<id>&delivery=0&capability=<token>`                   |
| Pair player / obtain bearer token | POST   | `/?op=pair` - `{}` (via authenticated proxy)                                   |
| Reset player pairing              | POST   | `/?op=pair` - `{"op":"pair","reset":true}`                                     |
| Start / resume challenge          | POST   | `/` - `{"op":"start","challenge":"<id>"}`                                      |
| Start timed attempt               | POST   | `/` - `{"op":"start","challenge":"<id>","run_for_time":true}`                  |
| Submit challenge answers          | POST   | `/` - `{"op":"submit","challenge":"<id>","answers":{"<answer_id>":"<value>"}}` |
| Retry challenge                   | POST   | `/` - `{"op":"retry","challenge":"<id>"}`                                      |
| Continue legacy practice          | POST   | `/` - `{"op":"practice","challenge":"<id>"}`                                   |
| Reset a live target (guarded)     | POST   | `/` - `{"op":"reset","challenge":"<id-or-id/mode>"}`                           |

Hall of Fame / ranked campaigns are out of scope for this pipeline (see `docs/policies.md`) -
there is no code path for `run_for_time`, `retry`, `practice`, or `switch_target` here.
Workers use `op=reset` only through `tools/wk.py reset <mode>` (own family/mode, bounded),
then re-run `begin <mode>` and re-derive answers.

- Send `POST` requests with `Content-Type: application/json`.
- The `run_for_time` property only applies to dual-pool challenges.
- The `start` response returns `answer_ids`. Use those exact values as the keys in answers.

Submit returns `"CORRECT"`, `"incorrect"`, or `"accepted"` (partial progress - check `remaining`).

Avoid executing challenge scripts and binaries locally. Anything you write is fine to execute - 
so long as they do not invoke challenge scripts/binaries.

Password-protected archives will almost always have the password `infected`. You should never
need to crack a password-protected archive - if `infected` does not work and you have not been
provided with additional instructions, cease working and clarify with the user.

## Solutions

When creating solutions to challenges, attempt to make your solutions "generic" and easy to modify
- avoid hard-coding values (note that challenges might call for some amount of hardcoding); any
scripts or code you write should be able to derive answers directly from challenge material.

You should work incrementally, solving the simplest `single_solve` first, then iterating to solve
`time_trial`, and finally `mastery`. `TIMETRIAL-MASTERYMODE.md` contains further instructions on 
how to interpret various flags and the gameplay around the `time_trial` and `mastery` modes, along
with the 'Hall of Fame' leaderboard.

This repo is web-exploitation only (see `README.md`), so previous solutions are always Python.
Before writing a new one, read `docs/web-worker-lessons.md` (the single key-info document:
recognized bug-class table, patterns, gimmicks and gotchas) and
`workers/web/solution-notes/INDEX.md` (day/tag -> note -> solver script). Check
`workers/web/solution-notes/` for the full notes - every file is `day-NN-slug.md`, one per
curriculum day, with a YAML frontmatter `tags` list (short kebab-case bug-class/technique
tags). `workers/web/past-solutions/day-NN-slug-*.py` holds the matching scripts, kept
verbatim; reference, copy from, or adapt them when it speeds up a correct, generic solution.

### Searching note frontmatter

From the repository root, use `rg` against the frontmatter's single-line `tags:` lists. Search for
approaches, challenge mechanics, and workarounds as well as vulnerability classes; read matching
notes to understand the context and confirm applicability.

Each note corresponds to Python solution material in the sibling `workers/web/past-solutions/`
directory. A challenge may have multiple solution files: for `day-xx-challenge-name.md`, look for
`workers/web/past-solutions/day-xx-*.py` and review the relevant files.

```sh
# Show matching tag lines and filenames for a keyword or partial tag.
rg -n -g 'day-*.md' '^tags:.*traversal' workers/web/solution-notes/

# List notes with an exact tag, without matching longer tags containing it.
rg -l -g 'day-*.md' '^tags:.*[\[, ]ssrf[,\]]' workers/web/solution-notes/

# Search for any of several related techniques.
rg -n -g 'day-*.md' '^tags:.*(header-spoofing|reverse-proxy-trust|loopback-bypass)' workers/web/solution-notes/
```

The `^tags:` anchor targets tag-list lines, rather than incidental mentions in note bodies. Replace
the example terms with relevant kebab-case tags or partial terms. Commands use repository-root
paths; adjust them if running from another directory.

### Work-in-progress

Write your solution's note directly to `workers/web/solution-notes/day-[DD]-[slug].md` and its
script(s) to `workers/web/past-solutions/day-[DD]-[slug]-<name>.py` (e.g. `-solve.py`, and
`-mastery.py` if mastery needed a separate script) - this is the single consolidated,
git-tracked reference library for every day, not a new per-day directory. Use the frontmatter
template at the top of any existing `day-*.md` file (`day`, `slug`, `lane: web-exploitation`,
`modes`, `has_solve_py`, `tags`) and actually fill in `tags` yourself for your own new note -
`workers/web/solution-notes/TAGGING-PROMPT.md` documents the tagging convention other notes were
tagged with. Keep scripts verbatim once they answer correctly (no later "cleanup" edits - the
point is an accurate historical record), and write the note with observations and anything that
will matter to future agents or to the `recovery_lib.py` recoverers (see below).

### Python-first iteration

Write your solution in Python: it iterates faster while you're still exploring an unfamiliar
encoding or an ambiguous classification, and lets you confirm answers against the live Game
Master. Keep the working script once it answers correctly - it's a useful reference for future
challenges that reuse the same patterns and should be considered integral. There is no compiled
or Rust solver in this repo; Python is the final solution form here, not an intermediate step.

### Reusable modules

- `solver_py_scripts/recovery_lib.py` / `recovery_lib_ws.py`: shared target classifiers and
  recoverers (HTTP and WebSocket respectively) - the automated first pass in `tools/web_rush.py`
  and worker solutions both use these. When a solution finds a new bug class, fold a `rec_*`
  recoverer into `recovery_lib.py`'s `classify_and_recover` dispatch (a coordinator decision made
  after reviewing graded evidence, per `docs/policies.md`) so the next day's first pass can solve
  it unaided.

### Research

Prefer the docs already in this repo over web searching or re-deriving things from scratch:
`docs/web-worker-lessons.md` (bug-class curriculum), `docs/lane-patterns.md` and `docs/history.md`
(per-day outcomes), `docs/live-web-catalog.md` (catalog notes). If you need token-heavy research
that would bloat your context, use a sub-agent and wait for it to return before continuing -
don't run multiple at once or try to multitask while one is working.

## Available CLI tools

Check `flake.nix` for a full list of tooling specific to this environment. Useful tools include:

- `uv`: for Python-related tooling,
- `bun`: for Javascript-related tooling,
- `sqlmap`: for SQL injection exploitation/recon.

Other tools that are globally installed:
- `curl`
- `tmux` (the release-night pipeline's only UI - see below)
- `opencode` (the release-night pipeline's worker agent - see below)
- `file`
- `jq`
- ripgrep (`rg`)
- Archive utilities: 7zip (`7z`), `unar`, `gnutar`, `xz`
- `pip`

## Coordinating a release night

If you are the coordinator (watching worker agents, merging notes, running the release-night
pipeline), start with `README.md` for the architecture, then `docs/runbook.md` for the
step-by-step runbook and `docs/policies.md` for the standing rules. `docs/history.md` and
`docs/lane-patterns.md` hold the dated per-day record; `docs/web-worker-lessons.md` holds the
accumulated grading/bug-class lessons.
