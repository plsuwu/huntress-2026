# Repository Layout

This repository combines the former `2026` and `2026-web` workspaces.
Read `README.md` for the migration scope and content exclusions.

- Main lanes: work within `main/` and follow `main/AGENTS.md`.
- Web lane: work within `web/` and follow `web/AGENTS.md`.
- Interpret historical references to a repository root as the respective subtree,
  except Git operations, which now apply to this combined repository.
- Keep challenge payloads, extracted files, fixtures, test data, and large files
  out of version control. Only source, notes, configuration, and small challenge
  metadata belong here. The migration used a 1 MiB per-file limit.
- Existing instructions for running tests or replay require separately restored
  local data. Do not add challenge content just to satisfy those instructions.
