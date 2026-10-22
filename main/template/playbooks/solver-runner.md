# Solver/runner playbook: build and maintain a compiled CTF solver

How to stand up and run a single compiled binary that solves challenges from a Game-Master-style API (list → start →
download → submit), split into **lanes** (families of challenges that change a little every day). Nothing here
assumes particular lanes. A lane is anything with a challenge-id prefix, a stream of artifacts and questions, and a
grader.

## 1. Goals, in priority order

1. **Never submit a guess.** A wrong submit can burn a one-shot attempt or a timed slot. Uncertainty is an error.
2. **Correct first, fast second.** Solves and overall place outrank leaderboard times.
3. **Fast where the clock is.** Learn exactly what the server times, and spend engineering only inside that window.
4. **Cheap to extend.** A new day should mean adding a question kind or a format reader, never writing a new solver.

## 2. Architecture

```
cli            run (default) | replay | oracle | status | routes | lanes | bench
run            guard -> preflight (free) -> start -> parallel fetch+analyse -> answer -> submit -> persist
gm             protocol client, response normalisation, route cache (pool sizes, timing flags, deadlines)
net            keep-alive HTTP pool, transparent decompression, per-request timings
lanes/<lane>   one module per lane implementing the lane contract
formats/       container and file-format readers (archives, executables, documents, media...)
crypto/        hashes, ciphers, KDFs (crates where they win; see §6)
codec          text codecs (base-N, escapes, ciphers) and "is this real text" scoring
rx             regex with the semantics of the reference solvers' language, so their patterns port verbatim
replay         offline: run a lane over saved runs/fixture manifests and diff against graded answers
oracle         untimed partial submits to localise wrong answers
```

### The lane contract

- `prefixes()`: the challenge-id prefixes the lane owns. This is the lane's whole identity; routing is by prefix.
- `analyse(delivery)`: runs once per downloaded artifact, on the fetch worker that downloaded it, so analysis overlaps
  the other downloads. It parses everything any question might need.
- `answer(question, analyses)`: runs per question. It dispatches on the question-id **kind** (the id minus its
  per-set prefix, e.g. `setN.`) through a table of kind → handler.
- `delivery_of(question)` (optional): lets a question be answered, and submitted early, as soon as its artifact lands.
- **Errors carry evidence.** An unknown kind or an unparsable artifact returns `Err` with what was seen (magic bytes,
  member names, candidate values). The runner then refuses to submit; an override flag submits a partial set.
- **Self-validate** wherever the lane allows: known templates, checksums, answer patterns, cross-checks between
  independent fields.

### Answer formats

- Read the question's declared format (`answer_format`, normalisation, casing, padding), but **the artifact's own
  instruction wins** when the two disagree. If decoded text says "submit this exact answer: X", submit X whole even if
  it breaks the declared pattern. Truncating to fit the declared format has failed real graded runs.
- Keep artifact bytes and whitespace verbatim: strip only line terminators. Leading spaces have been significant.
- Learn the graded format (casing, `0x` padding, braces, separators) on the free single-solve, then encode it.

## 3. The live pipeline

1. **Guard** (`check_route`): refuse dry runs that would start a clock, timed flags on routes that don't support them,
   one-shot routes without an explicit flag, and routes missing from the route cache. An environment variable
   (e.g. `NO_RANKED`) refuses ranked starts entirely; automated agents always run with it set.
2. **Preflight** (free): route cache plus `status`, so an already-solved or live attempt is reconnected, not
   restarted.
3. **Start.** The response lists questions, answer ids and deliveries. Use the exact answer ids.
4. **Fetch and analyse in parallel** on pre-warmed connections, analysing on the fetch workers. Prefer bundle or
   capability download links over per-file endpoints. For small-artifact lanes, optionally **hedge** (duplicate
   downloads, take the first) to beat server-side straggler tails. Never hedge large downloads.
5. **Answer and submit.** Nothing may run between the last answer and the submit. Batch every answer into one submit
   unless a size limit forces chunks.
6. **Persist after submit:** `meta.json`, `start.json`, `deliveries/`, `answers.json`, `result.json` (a list of submit
   records). A saved run directory must replay unchanged.

**Measure the clock.** Find out when the server's timer starts and stops (start received? submit received? grading
included?). Log server-relative timestamps for every leg (start sent/received, each delivery, submit sent/received),
and keep a tool that splits saved runs into legs. Only legs inside the window are worth optimising.

## 4. Ground truth: fixtures, replay, oracle

- **Fixtures are ground truth.** A fixture is a manifest pointing at saved start/delivery files plus the graded answers.
  `replay fixtures/<lane>` must be 100% clean before any live run, merge or deploy.
- Turn every **graded** run into a fixture with a script; don't hand-write expected answers.
- **Grader eras:** graders change without notice. Gate new behaviour on the attempt's start time (or another field in
  the saved start response), never on the calendar day, so old fixtures keep replaying with old rules.
- **Oracle:** on untimed attempts, a partial submit reports which fields were accepted. Probe in **groups**
  (one submit per question family, then per field) with visible progress; a blind serial bisection is too slow.
  On timed attempts a wrong full submit usually says only "incorrect", so reproduce on an untimed variant or a
  practice continuation of the same artifact.
- **Independent checkers:** when a graded result disagrees with replay, write a small independent decoder for that
  lane (any language) and diff it with the solver across the failing run *and* every saved graded run. A pattern that
  appears only in the failures is your cause.

## 5. Bootstrapping a new solver binary

1. **Read the organisers' docs and the API.** Record every op, its parameters, response shapes, pool and attempt
   semantics, error codes (`attempt_active`, `start_required`, `prerequisite_required`, `temporarily_unavailable`...),
   and what "timed", "ranked", "one-shot", "practice" and "retry" mean.
2. **Protocol client and route cache first.** Make `routes --refresh` and `status` work, with the token only in an
   environment variable, never printed, never sent over plain http.
3. **A persistence layer** that saves everything about every attempt, before any lane exists.
4. **One lane end to end** on single-solve (untimed, usually free to resubmit). Iterate in a scripting language
   while the encoding is unknown; port to the compiled lane once single-solve is graded CORRECT. Keep the script: it
   is the reference.
5. **Fixture generator** from graded runs, and `replay`.
6. **Guards and the no-ranked environment variable** before anything timed is possible.
7. Then add lanes one at a time, each gated by 100% replay.
8. **Static release build** for the run host (e.g. musl, with a real allocator such as mimalloc, since musl's malloc
   is slow), built off the run host and shipped as one file.

## 6. Performance work

- **Benchmarks:** `replay` over each lane's fixtures is the CPU benchmark. Use two metrics:
  - **suite** (many cases in one process): throughput, with compile costs amortised;
  - **fresh process per timed fixture**: closest to a live run, where every lazy compile and cache warm-up is paid.
  A change can win one and lose the other.
- **Profiling:** build with frame pointers and debug info (`-C force-frame-pointers=yes`, keep symbols), then
  `perf record -g`. On hybrid (performance/efficiency core) CPUs, compare per-core-type cycle counts
  (`perf stat -e cpu_core/cycles/u`) alongside wall time; scheduling noise swamps small differences.
- **Crates vs hand-rolled code:** profile first. For each hot primitive, micro-benchmark the candidates on *real*
  payloads in a scratch crate, swap in the winner, re-run the full replay and both benchmarks, and keep it only if it
  wins. Record the measured reason next to the dependency.
  - Usual winners: SIMD substring search, hardware CRC, C-backed decompressors, SHA-NI-capable hashes, a real JSON
    parser driving the existing value type, a better allocator.
  - Hand-rolled code can still win when it is specialised: a multi-lane batch KDF, or a pipelined block cipher in
    exactly the needed mode. Keep it.
  - **Check semantics, not just tests:** run old and new implementations over every saved input in the repo and
    require identical output (e.g. parse every saved JSON file with both and compare).
  - **Regex engines:** automata engines are fast to match but slow to compile Unicode classes. A single-process run
    pays every compile, so use ASCII-mode builds for ASCII subjects (results are identical there), build the Unicode
    variant lazily, and cache runtime-built patterns.
  - **The run host differs from the dev box** (missing CPU features, fewer cores, burstable credits). Test fallback code
    paths and benchmark there when possible, sparingly if CPU credits matter.
- **Network:** keep-alive pools, pre-warmed connections, compressed JSON bodies (accept the fastest encoding the server
  offers), identity encoding for already-compressed downloads.

## 7. Deploying

1. Clean the replay, run the tests, and build the release with the network features you need.
2. Build the static binary off-host. Record its hash.
3. Copy it to the run host under a temporary name. **Keep the previous binary** (`<bin>.prev-<hash>`), then atomically
   `mv` the new one into place. Never deploy while a campaign is using the binary.
4. Refresh the run host's route cache.
5. **Live check without spending ranked attempts:** an untimed regular attempt (or a reconnect of a solved one) on
   every route of the current day. Only then spend ranked attempts.
6. If ssh to the run host is slow, open one multiplexed master connection and route every command and copy through it
   (keep the socket path short; Unix sockets cap paths at 108 bytes).

## 8. Maintenance loop for a new day or a new question kind

1. `routes --refresh <day>`: note timing support, pool sizes, deadlines and one-shot flags.
2. Dry-run single-solve. Unknown kinds fail loudly with the facts seen.
3. Extend the lane: a codec, a format reader, a kind-table row or a new container. Prefer generic fixes that follow
   the declared format.
4. Submit single-solve. On `incorrect`, use the grouped oracle; on thousands of answers, oracle by family first.
5. Untimed TT/mastery regular attempts; fixtures from the graded runs; 100% replay.
6. Notes per challenge (what was new, what was graded, gotchas), plus a running "how this lane evolves" document
   with next-day predictions.
7. Only then ranked runs (see the orchestrator playbook).

## 9. Hard-won rules

- Single-solve undersamples the vocabulary: pre-support every variant seen on earlier days, and treat the first timed
  run as a likely fixture-collection run.
- Regular and ranked pools can draw on different variant generators; a quirk may appear only in ranked variants.
- Treat transient server errors (5xx, "temporarily unavailable") as transient: back off and retry. But record them,
  since a 5xx on submit may have consumed the attempt.
- Saved file formats drift (e.g. a single result object becomes a list of submit records): tooling that reads them
  must accept both.
- Old notes from previous sessions are hints, not truth; the question contract can change between sessions.
