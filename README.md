# Huntress CTF 2026

## Foreword

Prospective employers: I really would like a job and I have experience with this kind of tooling waow

Realistically: **I would consider nothing here to be worth looking at in any meaningful capacity**. Its mostly LLM-generated, 
and its mostly here for reference purposes; I would consider it closer to an artifact at a museum, perhaps. 

A little opinion piece should be available at [plsuwu.com](https://plsuwu.com/post/2026/10/5) in the near future 
(if u even care).

---

One repository containing the two original workspaces:

| Directory | Original repository | Contents |
| --- | --- | --- |
| `main/` | `2026` | Rust runner, Python solvers, lane notes, orchestration, and run metadata |
| `web/` | `2026-web` | Web tooling, Python solvers, solution notes, and run metadata |

Work from the appropriate subtree so existing relative paths continue to work.
For example, run main-lane tooling from `main/`, Rust commands from
`main/ctf-run/`, and web tooling from `web/`. Each subtree retains its own
`flake.nix`, `.envrc`, `AGENTS.md`, and operational documentation.

## Repository Contents

This is a source-and-metadata archive, not a challenge-content distribution.
Authored solvers, documentation, configuration, and small run records are kept.
Run records retain their original paths and JSON contents: `meta.json`,
`start.json`, `result.json`, `answers.json`, `answers_full.json`, `errors.json`,
and `oracle.json`, where available and no larger than 1 MiB.

Challenge deliveries, extracted artifacts, fixtures, test files and test data,
compiled output, caches, runtime logs, local agent settings, VPN configuration,
and private-key files were excluded from both the imported histories and the
working tree. Files larger than 1 MiB were also excluded throughout history,
including oversized JSON responses. Small `meta.json` records remain available
for those attempts. The external `main/utils/attackv19.2.csv` dataset is omitted
as well; `attack_query.py` needs a separately supplied local copy.

The root `.gitignore` allowlists source/configuration formats and selected JSON
metadata names, then excludes payload and test directories. File size is not
enforced by Git's ignore mechanism: check sizes before adding new files.
Keep any locally restored payloads under ignored directories. Replay commands
and tests described in historical documentation need separately restored data;
they are not self-contained in this archive.

Local `.env` files were not copied. Configure credentials separately before
using live tooling. Existing notes and historical API responses have not been
redacted and may contain tokens or capability URLs; review them before making
this repository public.

## History

Both source histories were filtered and relocated before merging. Author,
timestamp, commit message, and surviving merge ancestry are retained, but commit
IDs changed. Commits emptied by filtering were pruned.

| Source | Original HEAD | Original commits | Retained commits |
| --- | --- | ---: | ---: |
| `2026` | `5e81b6d17ea722eed1a4011e79b7b917735dd6cf` | 251 | 245 |
| `2026-web` | `ec063c969559d7a973798490c3d0249bbf6e47f3` | 44 | 42 |

`history/main` and `history/web` mark the filtered source tips. The `main` branch
merges them and includes additional small metadata records found in the original
working directories. Only the original checked-out branches and their ancestors
were imported; unrelated branches, tags, and linked worktrees were not migrated.
Local old-to-new commit maps and migration details are under `.git/migration/`;
these local records are not included in future clones.

The original sibling repositories were left untouched, including uncommitted
work, ignored payloads, credentials, and linked worktrees. This repository has no
remote configured and does not depend on those siblings to access its history.
