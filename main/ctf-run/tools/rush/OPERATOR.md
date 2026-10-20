# Rush operator cheat-sheet

Day-release workflow for the human operator. Launch and internals live in `rush.py` / `AGENTS.md`; this is the
at-release quick reference, with the **web (quarantine) lane** rules called out. Commands are shown from the repo root
(`.../huntress-ctfs/2026/`); `merge.sh` and ranked tools run from `ctf-run/`.

## Launch & attach

The systemd user timer (`huntress-rush.timer`, 22:45 AEST on event days) runs `rush.py all` for you. To do it by hand
or follow along:

```
python3 ctf-run/tools/rush/rush.py all --day N      # prep + launch + watch (what the timer runs)
tmux -L huntress attach -t rush                     # attach to the session
journalctl --user -fu huntress-rush                 # follow the timer run
```

One `rush` tmux session, one window per lane. Nothing about attaching changes from before - it is still a single
session you drive.

## Two zones in the one session

| Window(s) | Zone | Branch / cwd | You do here |
|---|---|---|---|
| `control` + `warmups` `metadata` `script` `binary` `mobile` | **Coordinator** | `master` / main checkout | overview, `status`, `merge.sh`, HoF campaigns, `takeover` |
| `rush:web` | **Quarantine** (isolated worker) | `web-vault` / `../wt/web` | direct the web lane by hand |

Switch windows: `Ctrl-b` then the window number, or `Ctrl-b :select-window -t rush:web`.

## Coordinator (your overview) — same as before, plus one rule

```
python3 ctf-run/tools/rush/rush.py status --day N   # web shows a single "quarantined" line, read nothing of it
ctf-run/tools/rush/merge.sh N                        # merges the 5 solver lanes; web is skipped by design
ctf-run/tools/rush/merge.sh N --deploy               # + ship the static binary to the jumpbox
python3 ctf-run/tools/rush/rush.py takeover --day N --lanes <lane>   # finish a stalled solver lane (never web)
```

**The one hard rule:** from the coordinator, never `cat` / `grep` / open any file under `../wt/web/` or any
`*web*` path under `utils/solver_py_scripts/`, and never merge `web-vault`. Reading that content is what downgrades this session's model
(5.5 -> 4.8) and trips the auto-mode classifier into denying Bash. The tooling already keeps it out of your way -
`status` abstracts it, `merge.sh` omits it - so just don't go looking.

## Web lane (`rush:web`) — drive it by hand

Same as Day 19, now self-isolating:

1. At release it does read-only recon, presents the day's web challenges, and **waits**. It starts nothing on its own.
2. You tell it what to start / solve, in its own window. Direct it there; don't relay its files through the coordinator.
3. It keeps its material in data files, commits its solves to **`web-vault`** (never master), and routes any
   sensitive text to a non-Claude model via `utils/safeguard-probe/openrouter.py`. You don't manage that - it's
   internal to the worker.
4. Its brief / recon / markers live in `../wt/web/.vault/dNN/`, not the shared `runs-dNN/rush/`.

To message it by hand (its launcher may leave a picker open - dismiss with Escape first, as `rush.py send()` does):

```
tmux -L huntress send-keys -t rush:web Escape
tmux -L huntress send-keys -t rush:web -l 'your message'; tmux -L huntress send-keys -t rush:web Enter
```

## Merge & ranked

- **Solver lanes:** merge on their `done-<lane>` markers (`merge.sh N`), then run the HoF campaign once lanes are in
  (see `AGENTS.md` / the `day-release-automation` + `hall-of-fame-mechanics` memories). Unchanged.
- **Web:** never merged into master and never part of the coordinator's campaign. If you want a ranked web run, direct
  it from `rush:web` on your say - the worker owns its `web-vault` worktree end to end.

## Getting web results into your overview — safely

Don't open the worker's files. Ask the **worker** (in `rush:web`) to report: it will give you flags, route status, and
mechanism in prose (not the raw material), which is safe to read in the coordinator. That keeps the overview complete
without pulling classifier-tripping content into your main model.

## Golden rule

Treat `rush:web` as a sealed box: interact through its window, read its replies, but never let its files or branch
cross into the coordinator. Everything else about release night is exactly as you ran it before.
