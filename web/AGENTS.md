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

The Hall of Fame speed-scoreboard API (undocumented, same host and token) is described in
`orchestrator/hall-of-fame.md`.

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

When creating a solution to a challenge, you may wish to reference previous solutions: while some
previous solutions are not available for reference, you can view implementations in the `ctf-run` 
project (Rust), and in `utils/solver_py_scripts`.

### Work-in-progress

Your work-in-progress should go in a subdirectory in `utils/solver_py_scripts/day-[DD]-[lane-name]/` 
that you create (following the general format/layout per other solutions). Keep this directory clean 
and well-organised, but retain all artifacts for future reference. After each solution, note down any 
observations and other important information - particularly anything that will be of importance for 
future agents that will build on your work.

### Python-first iteration

Unless otherwise instructed to do a Rust implementation, you should write your solution using
Python: it iterates faster while you're still exploring an unfamiliar encoding or an ambiguous
classification, and lets you confirm answers against the live Game Master before committing to the
compiled solver. Keep the working script once it answers correctly - it's a useful reference for
future challenges that reuse the same patterns and should be considered integral. See `ctf-run/AGENTS.md` 
for how the compiled solver should relate to it. Since Day 15 the standing instruction is to solve new
days directly in `ctf-run` (see `orchestrator/policies.md`).

### Reusable modules

- `utils/solver_py_scripts/nested_zip.py`: flattens nested zip archives into `{zipname: {member: bytes}}`
  and resolves a question's `artifact` field (`"<zip> -> <member>"`, optionally prefixed `delivery N:`).
  **Mastery-mode downloads usually are nested zips** (outer zip -> one inner zip per question set), and
  question sets/kinds can differ between variants - use this module instead of re-implementing it. Under
  `python3 -I`, add its directory to `sys.path` explicitly (see
  `utils/solver_py_scripts/day-04-metadata-forensics/picture-this/solve.py` for a worked example).

### Code research via sub-agents

You have tools available at your direct disposal in order to keep context in check and remain on-task.
Such tooling includes the `attack_query.py` utility (`utils/attack_query.py`), which can be used to 
make focused queries for MITRE ATT&CK techniques, which is preferable over web searching, for example.

Sometimes, however, you will need to do token-heavy research; rather than bloating your context by reading
in content, you may wish to use ephemeral sub-agents and instruct it to perform research. Instruct them 
to use the `previous-solution-research` skill here.

Never spin up more than one sub-agent at most; if you feel like this is necessary, there is a good chance
you are trying to multitask too heavily. Additionally, rather than trying to perform additional work while
waiting, you should ALWAYS wait for the sub-agent to return with its results to avoid focusing on the 
wrong tasks.

## Available CLI tools

Check `flake.nix` for a full list of tooling specific to this environment. Useful tools include:

- `uv`: for Python-related tooling,
- `bun`: for Javascript-related tooling.

Other tools that are globally installed:
- `curl`
- `file`
- `jq`
- ripgrep (`rg`)
- Archive utilities: 7zip (`7z`), `unar`, `gnutar`, `xz`
- `pip`

## Coordinating a release night

If you are the coordinator (watching rush lane agents, merging, deploying, running ranked campaigns), start
with `orchestrator/README.md`: standing user policies, the release-night runbook, Hall of Fame mechanics,
grading lessons and a dated history all live there.

