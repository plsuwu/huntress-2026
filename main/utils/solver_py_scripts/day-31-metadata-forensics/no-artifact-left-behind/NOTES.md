# Day 31 metadata - No Artifact Left Behind (`metadata-forensics-day-31-no-artifact-left-behind`)

Prompt: "Everything is evidence now. Reconstruct the complete execution-and-delivery chain, defend every edge with
primary metadata, and identify the tempting conclusion the corpus does not support." As usual only one new question
kind came out of it: single-solve asks it once, TT is the cumulative bundle (36 q), and mastery has **15 sets** (540 q).

## New kind: `d31-case-001-graph-execution-chain-artifact-ids`
- Evidence: `case-001.zip` = `artifacts/<id>` only (no manifest, no extensions; ids like `birch-bd18`). Each
  artifact names the next one in its primary metadata:
  - PowerShell text: `Start-Process '.\<id>'` / `Invoke-Item` / `& ".\<id>"` (`#` comment line is a decoy word);
  - PNG: `tEXt Comment` = `next=<id>`; `done <hex>` marks the end of the chain;
  - SQLite: `launch_queue(id, target, note)` - `target` is the id, `note` (`nectar a78687`) a decoy;
  - email: base64 attachment `route.txt` = `forwarded to: <id>` / `open: <id>` (Subject words are decoys);
  - zip: `run.txt` = `execute <id>`, `readme.txt` decoy words.
- Single: 22 artifacts = one 4-node chain from the only root (no incoming edge) to the `done` PNG, plus an
  18-node **cycle** with no entry point (the "tempting conclusion"). Two script nodes on the cycle had mismatched
  quotes (`".\x'`) - flagged `broken` in facts, not used for the decision.
- Graded: the ids along the acyclic root -> terminal walk, in chain order, compact JSON (`json` format, `trim`).
- New `ctf-run/src/formats/chain.rs` (`Graph::parse` reuses `cluster::Cluster::parse` for the zip/ids; pointers
  are whole-token occurrences of known ids in the primary fields; `execution_chain` = the one root whose walk ends
  at a terminal; errors on forks or >1 candidate). `lanes/metadata.rs::graph_answer` dispatches on `-graph-<field>`
  after relation. Only `execution-chain-artifact-ids` is graded; chain length, root/final id and `cycle-artifact-ids`
  are guesses. An unknown field fails with `Graph::facts` (id, kind, next, END/BROKEN, evidence per node).
- Predicted follow-ups (the prompt): "delivery" vs "execution" edges split by kind (email `forwarded to` vs
  `open`), "defend every edge" (edge evidence / broken-syntax edges), and the "tempting conclusion" (the cycle ids).

## Runs (all untimed, all CORRECT on the first submit after adding the kind)
- `ctf-run/runs-d31/.../single_solve-b72d92f6-1793451750`, `time_trial-fa066eb9-1793451762`,
  `mastery-4dad78f4-1793451777` -> `fixtures/metadata/d31-{single_solve,time_trial,mastery}.json` (replay 85/85 clean
  before the fixtures were added).
