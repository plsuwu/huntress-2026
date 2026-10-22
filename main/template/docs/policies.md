# Standing policies (user decisions)

Each entry gives the rule, then why, with the date it was set and the user's own words where useful. These override
defaults. Update entries in place when a decision changes; don't append contradicting ones.

## Attempts

- **Timed and ranked attempts need explicit approval, every time.** Approval for one campaign does not carry over.
  Untimed attempts are free. *(YYYY-MM-DD)*
- **Solves vs leaderboard:** <which comes first; may automation fire one-shot modes, and under what conditions>.
  *(YYYY-MM-DD)*
- **Feedback loop for timed failures:** a failed timed/one-shot mode gets an untimed practice run on the same
  artifact plus the grouped oracle, and the lane keeps iterating there. *(YYYY-MM-DD)*
- **When to campaign:** <e.g. hold ranked runs until every lane of the day is solved; one campaign per day>.
  *(YYYY-MM-DD)*
- **Where ranked runs run:** <run host>. If it is down, ask; never fall back to another machine. *(YYYY-MM-DD)*

## Scope

- **Registered lanes only** (`config.toml` [[lanes]]). Lanes the event adds later must not be engaged in any
  capacity unless the user says so. *(YYYY-MM-DD)*
- **Sub-agents:** <how the user wants parallel work done, e.g. warm lane agents in tmux, no ad-hoc sub-agents>.
  *(YYYY-MM-DD)*

## Load on shared endpoints

- Never saturate the Game Master or a live target: one request at a time or small bounded concurrency, timeouts,
  session reuse, backoff on 429. A 5xx may be challenge content: record it, don't blindly retry. *(YYYY-MM-DD)*
- Probe grading in groups with visible progress, not with blind serial bisection. *(YYYY-MM-DD)*

## Engineering

- <language/solver policy, e.g. solve new days directly in the compiled solver; crates only where benchmarked
  faster>. *(YYYY-MM-DD)*
- Commit to the main branch before a release: lane worktrees branch from the committed HEAD. *(YYYY-MM-DD)*
