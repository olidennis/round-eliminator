# Native SAT fixed-point search

Native default-feature builds use three cooperating searches for the existing fixed-point `loop`
operation when the active degree is finite and positive and the passive degree
is two. The browser/WASM and `onlyrust` builds keep the original implementation.
Other degrees and partial fixed-point searches also keep the original path.
The partial-extension code requires an invertible original-label mapping,
whereas SAT allows label mergers.

The diagram/game search looks for a fixed point. The general proof search independently
looks for a universal nonexistence certificate. A witness-guided worker searches
small recombinations of actual game derivations. The first two use dedicated
workers; guided jobs run in a configurable pool with one shared scheduler.
A conclusive answer cancels and joins the remaining workers. No GUI button,
manual restart, or symbolic diagram-completion search is involved. The original
algorithm remains callable explicitly as `Problem::fixpoint_loop_symbolic`, but
is not a worker in this native Loop. `is_pred` is unchanged.

Native GUI requests also print SAT progress to the server terminal (stderr),
including the diagram size and proof-step bound being searched, candidate checks, exhausted sizes,
and construction of the successful fixed point. GUI progress events are still
sent as before; no extra option is needed to enable terminal output.

Successful results are automatically named before being returned, with no GUI
button or extra step: the image of `A` is named `A`, a shared image of `A` and
`B` is named `(A=B)`, and auxiliary nodes are named `(FP0)`, `(FP1)`, etc.
Original names are reserved; generated names receive a suffix if needed to
avoid collisions. This changes presentation only, not search or verification.

The GUI automatically shows a **Label mapping** table from original labels to
result labels. It uses the existing `mapping_oldlabel_labels` and
`mapping_oldlabel_text` metadata, so subsequent manual renaming updates the
displayed targets. If normal GUI simplification removes a target, the table
marks its entry as removed. The library's `Found.mapping` and exported diagram
retain the complete synthesis mapping, with the same readable diagram names.
These nodes are not treated as speedup label sets or renamed by generators.

## Running a bounded search

From the repository root:

```sh
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example fixpoint_sat -- round-eliminator-lib/examples/fixpoint_sat/two_color.txt 10 10000
```

The arguments after the file are an optional maximum lattice size and an
optional total number of candidate checks. Without those arguments the search
has no bound. A successful run prints the fixed point and its diagram, including
the original-label mapping, in the custom fixed-point input format.

For the library API:

```rust
use round_eliminator_lib::algorithms::{
    event::EventHandler,
    fixpoint_sat::{SatSearchOptions, SatSearchOutcome},
};

let options = SatSearchOptions {
    min_nodes: 1,
    max_nodes: Some(10),
    max_candidates: Some(10000),
    ..Default::default()
};
let outcome = problem.fixpoint_sat(&options, &mut EventHandler::null())?;
```

`Found` contains the checked fixed point, lattice order, label mapping, exported `diagram_text`, and
statistics. `Exhausted` rules out only the specified size interval.
`Inconclusive` means a candidate budget or SAT conflict limit was reached.
`NoFixedPoint` contains an all-size certificate from the existing symbolic
nonexistence test. These outcomes are deliberately distinct.

`conflict_limit` is an optional limit per solver call. `generalize: false`
uses exact candidate exclusions, useful as a reference implementation.
`use_game: false` restores full construction of every candidate for comparison.
The command-line example accepts a final `--full-checker` argument after both
numeric bounds for the same purpose, and reports elapsed search time.
`check_nonexistence: false` disables the symbolic proof oracle, leaving the
candidate search entirely free of mirrored expressions.

## Running the cooperating searches (the native GUI Loop path)

`Problem::fixpoint_sat` remains the diagram-only API for controlled comparisons.
`Problem::fixpoint_search(&diagram_options, &certificate_options, eh)` runs all three.
`CertificateSearchOptions` has independent optional `max_steps` and
`conflict_limit` bounds; both are `None` by default. Disabling
`diagram_options.check_nonexistence` bypasses both certificate workers as well.

For the same parallel search from the terminal:

```sh
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example fixpoint_sat -- round-eliminator-lib/examples/fixpoint_sat/hard_nonexistence.txt --parallel
```

Finite diagram exhaustion is not a conclusive result for the combined search:
an unbounded proof worker keeps running. To bound the entire API call, bound
the diagram and certificate options. The guided worker also respects the
certificate options, up to its eight-new-step local limit. If all workers
finish without a conclusive result, the combined API
returns the diagram worker's `Exhausted` or `Inconclusive` outcome.

Proof synthesis alone is also available:

```sh
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example fixpoint_certificate -- round-eliminator-lib/examples/fixpoint_sat/maximal_matching.txt 4
```

Its arguments are the file, optional maximum combination steps, and optional
SAT conflict budget. The API is `Problem::fixpoint_certificate`. Its `Exhausted`
means only that this bounded proof search found no certificate, **not** that a
good diagram exists. Budget interruption is likewise inconclusive.

## What is being synthesized

An N-element lattice has a Boolean order matrix and one-hot join and meet
tables. Clauses require a partial order and the actual least upper/greatest
lower bounds, not merely some common upper/lower bounds. Both operations are
commutative and total.

Nodes are numbered in a linear extension, with bottom 0 and top N-1. This
removes some renaming symmetry without losing any finite lattice. Interpretations
of original labels are solver variables and may coincide; they are not required
to follow the numerical order of the original label IDs. The supplied input
diagram's positive order relations must hold. If it is absent, it is computed
first. Bounds and minimality refer to this class of diagrams.

There are no mirror nodes, mirror operations, or mirror-compatibility axioms in
the SAT encoding. Arbitrary nondistributive lattices are included.

The basic lattice encoding uses O(N^3) variables and O(N^4) clauses. This is a
small-model search, not a polynomial-time algorithm for finding fixed points.

## Checking and learning

By default each candidate is checked by a witness-producing version of the
two-player tree game. Only the passive constraint is saturated; the active
constraint is queried through the game rather than constructed in full.
Rejected candidates never require full active construction. For a successful
diagram, `fixpoint_onestep` and `compute_triviality` still construct and check
the actual returned problem; disagreement with the game is an error.

A game position is an unordered active tuple to be dominated. The first player
chooses a coordinate z and a split x,y with z <= join(x,y). The second player
chooses one of the two replacement tuples to challenge. A position dominated
by an original input line is a winning leaf. Both children must win for a split
to win. The solver lazily explores the finite AND/OR graph and propagates wins
through reverse edges. Cycles alone are not wins; this matters for
nondistributive lattices.

A winning strategy yields actual lattice terms: join at the split coordinate,
meet elsewhere, with occurrence permutations preserved. These terms may
strictly dominate the requested tuple. The actual terms, not the requested
labels, feed the SAT blocker and the nonexistence oracle. Targets are minimal
pairwise-compatible tuples; incompatible prefixes and provably nonminimal
subtrees are skipped. When the nonexistence oracle is enabled, all winning
minimal targets are examined for certificates; otherwise the first suffices.

Statistics include `game_checks`, `game_positions`, `game_moves`, and
`full_constructions`. The default path performs zero full constructions for
rejected candidates and one when returning a fixed point. The game can save
substantial active-saturation work, but is not guaranteed faster on every
instance: its state space, passive saturation, or SAT encoding can still be
expensive.

A failed candidate does not advance the size. The same
incremental Minisat instance receives another blocker and is solved again.
Only UNSAT advances the size. Thus, without budgets, every size is exhausted
before moving on; a successful diagram is smallest in the requested interval.
An exhausted size is reported as a progress event.

A generalized failure certificate consists of:

1. A derivation of an active line with lattice terms a[0], ..., a[d-1].
2. For every ordered pair (i,j), a derivation of a passive line (p,q).
3. Inequalities p <= a[i] and q <= a[j].

The passive procedure operates on the reversed diagram, so its recorded union
is interpreted as meet in the forward lattice, and its recorded intersection as
join.

Why this is sufficient: replaying an active derivation produces that line or
a dominating active line in the saturated active constraint. Replaying a
passive derivation produces that pair or a dominating passive pair under the
reversed order. Monotonicity preserves the listed inequalities under those
dominations. Every pair drawn from the active line is therefore allowed by the
upward-expanded passive constraint, which witnesses triviality. Changing the
lattice may change which derivations the implementation chooses to retain;
the argument requires their semantic consequences, not identical execution
traces or pruning decisions.

The solver gets the negation of the conjunction of these inequalities.
Terms are over original input labels, so certificates can be reused at larger
sizes. Their one-hot evaluations use the candidate's actual operation tables.
Each extracted derivation and obstruction is evaluated in its source candidate
before use; an inconsistent trace is an error, never a nonexistence result.

When original labels have merged, the implementation retains a whole original
input line as the source of each initial mapped line, with occurrence-level
provenance. Inverting a many-to-one label mapping would incorrectly turn
different original labels into the same symbolic generator.

## Preserving all-size nonexistence detection

Each recovered game witness is also submitted to the old symbolic test: for its
leftmost expressions e[i], ask whether every mirror(e[i]) <= e[j] is forced
by the original problem. This reuses `Context::init_from_problem`, the mirror
transformation, and `is_pred`; no finite diagram completion is needed for this
query. Its conclusions retain the original algorithm's assumptions about
good diagrams.

Mirrors therefore remain internal to the proof oracle. They do not increase
the size of the lattice being synthesized. `use_game: false` instead submits the full checker's trivial active
derivations. The two checkers and the symbolic search can discover different
derivations and certificates;
it is not claimed to reproduce the symbolic search's discovery order or
termination behavior.

### Independent SAT certificate synthesis

The new worker synthesizes a finite DAG of active derivations. Each leaf is a
whole original active configuration, not independently chosen labels. A step
chooses two earlier configurations, permutes their occurrences bijectively,
takes join at one coordinate and meet at all others. Earlier steps can be
reused, so repeated subderivations do not require repeated SAT structure.
Fixing the join coordinate to zero is harmless because both input permutations
and the permutations at all later uses remain free.

For terms `x,y`, encode `C(x,y) = is_pred(mirror(x), y)` directly as a Boolean
circuit. Ground compatibility comes from a fresh original-problem oracle.
Decomposing either term produces OR for a join and AND for a meet; the two
available decompositions are ORed, exactly as in `is_pred`. Child terms precede
their parents, so the circuit has no self-supporting proof cycles. The final
tuple must satisfy every `C(x[i],x[j])`, including diagonal pairs. The encoding
has no finite diagram or extra mirror nodes.

A satisfying model is independently replayed from its chosen whole leaves,
parents, and occurrence permutations. The recovered expressions are then
submitted to the existing `NonexistenceOracle` before returning the certificate.
The SAT compatibility circuit alone is never trusted as the final check.

The worker tries increasing proof bounds in fair rounds. Unfinished bounds
remain scheduled with increasing conflict budgets, while longer proofs also
get a turn. All bounds use assumptions in one incremental solver. Only an
actual UNSAT result marks a bound exhausted; a conflict slice never does. This
does not promise a shortest proof or a generally terminating YES/NO procedure.

Failed diagram checks also send their valid active derivations to the proof
worker as extra leaves. A bounded nonblocking queue prevents either worker
waiting for the other. Imported hints are limited to 32 distinct derived tuples
and 256 distinct term nodes; excess hints are optional accelerators and may be
discarded. Original configurations and all legal combinations remain available,
so proof discovery does not depend on receiving a useful hint. Arrival timing
can affect discovery order, but no randomized restarts are required.

Cancellation interrupts active native SAT solves and is checked during encoding,
game exploration, and saturation. The coordinator keeps issuing progress events
while SAT is blocked, so the server's existing STOP callback can cancel all
workers. Scoped threads are joined even when that callback unwinds.

### Witness-guided small SAT instances

The additional worker imports compact derivation DAGs from the game, including
intermediate winning configurations, winning positions explored off the final
strategy, and alternative already-explored splits with winning children. It
replays each DAG against the original whole active configurations, checking
parent order, pivot coordinates, and bijective occurrence permutations. It
never treats independently selected coordinates as a valid starting line.

Concrete fragments become fixed SAT leaves. Each local instance contains at
most 12 configurations and initially searches one **additional** step.
Later slices extend this local bound one step at a time, up to eight;
a difficult earlier bound does not prevent trying a longer bridge.
The fixed subexpressions already contain their earlier construction: three new
steps does not mean expression depth three. Compatibility between concrete
terms is evaluated before SAT; the unknown circuitry concerns the few new
steps. Non-pivot coordinate permutation symmetry is removed. The native solver
is still the existing MiniSat dependency; no extra solver or mirror nodes were
added. SAT results are replayed and verified with the unchanged `is_pred` oracle.

The bridge scheduler lazily pairs six-fragment blocks using round-robin
matchings. Each round covers the whole archive; a sweep covers every block
pair. New/changed blocks have a separate priority lane, alternating with the
regular sweep without resetting it. Only the latest version of each block
pair is remembered, rather than eagerly queuing thousands of overlapping jobs.
Fresh batches alternate with unfinished batches. Conflict slices increase
from 2,000 to 8,000; every retry retains its native solver, learned clauses,
and already-proved UNSAT bounds. There are no cold retries. A neighborhood
gets at most 16 slices, each making one bounded SAT call, before retiring
locally as inconclusive. Bounds grow first, then unresolved bounds are revisited
in rotation. A user-supplied
conflict limit disables retries after the final bound expansion.

This optional accelerator keeps a rotating 256-slot archive for ordinary
game/feedback derived configurations, in addition to pinned original inputs
and default-diagram seeds. New valid fragments can replace older unpinned ones;
in-flight/cached solvers own their original terms independently of archive slots.
It caps expanded
configuration trees at 4,096 nodes, and distinct fixed subterms in each batch at
512. These are heuristic resource limits, not completeness claims. Deduplication
uses syntactic equality plus commutativity/idempotence, never equality of values
in one finite diagram. The unrestricted proof worker remains available, with
its original inputs and grammar. Neither local UNSAT nor a local budget limit
is reported as a global conclusion.

The same worker additionally runs a bounded **feedback/repair engine** for
active degrees up to six. It keeps a rotating pool of 96 original-input
derivation DAGs (up to 96 DAG nodes and 4,096 expanded term nodes per fragment).
One-to-three-step jobs can satisfy partial compatibility goals rather than
requiring a complete certificate immediately. Valid models and their
intermediate derivations are replayed, retained, and reused; complete models
must still pass the original nonexistence oracle.

Internal repair keeps a proof's surrounding wiring fixed and re-synthesizes
the parents and occurrence permutations at one to three selected internal
combination steps. The pivot at each selected step stays fixed. All affected
ancestors are re-evaluated; they are not treated as unchanged concrete
expressions. Growth jobs remain available to change topology and proof size.

The pool ranks proofs by compatibility between coordinates and with original
labels, but retains up to four syntactically distinct proofs per profile/depth
bucket. Profile equality or equality in a candidate lattice never establishes
symbolic equality. Scheduling mixes high-scoring proofs with breadth/age-based
exploration and does not require monotone score gains. Seed batches contain at
most eight fixed fragments. SAT encoding is capped during allocation at
150,000 variables, with 2,000 additional variables reserved for partial-goal
circuitry. Unfinished jobs retain their solvers and are retried with budgets
increasing from 2,000 to 32,000 conflicts. Bridge and feedback caches share the
pool's variable-credit budget, with no additional per-family cache cap.
Under memory pressure the scheduler resumes cached jobs before admitting fresh
ones, and may leave some worker slots idle. The working set also has a
16-jobs-per-configured-worker count limit. A finite `max_steps = n` caps fresh
feedback scheduling at `8*n` turns; saved jobs still receive their bounded hot
retries. Exhaustion of these
heuristic resources is always inconclusive.

After the fast game check, witness export also samples up to 64 omitted
decompositions, with a 2,048-pair work cap, using already winning positions or
immediate original-input leaves. It does not extend the reachability search,
change the selected game strategy, or change the learned diagram blocker.

Terminal events `Proof: growing reusable fragments`, `Proof: repairing internal
branches`, `Proof: retaining partial SAT derivation`, and `Proof: retrying cached
repair/feedback` describe this work. The displayed best compatibility count is
a heuristic score, not a guarantee of progress toward a certificate.
See [the feedback/repair report](fixpoint-sat-feedback.md) for controlled
reconstruction tests and a separate blind benchmark.

No new GUI control is needed: native **Loop** automatically starts this worker.
Terminal events prefixed `Proof: guided` report fragment counts, new-step bounds,
SAT variable counts, local exhaustion/budget limits, and verified certificates.
The diagram-only API still does not run this worker. With `use_game: false`,
the default seed described below is still available; full-checker witnesses
continue to feed the general proof worker as before.

### One-time default-diagram seed

Native Loop's guided scheduler first constructs the ordinary default lattice
(right-closed subsets ordered by reverse inclusion) and runs the full **active
constraint saturation** on it, with provenance tracking. It keeps all recorded
intermediate configurations, including ones discarded by dominance during
saturation, not just the final triviality witness. Passive saturation is not
needed for this bootstrap: it supplies active derivations, and compatibility
is checked by the unchanged universal oracle.

The bootstrap runs once per Loop invocation, in the guided scheduler thread;
the diagram and general proof workers run concurrently. It is independent of
the `use_game` choice and of any cached/custom GUI fixed-point diagram. There
is no additional GUI control, SAT backend, mirror relation, or WASM change.

All retained bootstrap configurations get additional bridge-archive capacity,
separate from the ordinary 256-fragment allowance. Their DAGs are also kept as
a read-only source for the feedback/repair pool: four roots are visited per
scheduling turn, cycling through the entire source, rather than permanently
dropping everything beyond a one-time sample. Existing per-fragment and local
SAT limits still apply. The general proof worker receives optional nonblocking
hints under its existing 32-configuration/256-subterm admission limits.

To bound the optional startup work, default completion stops at 128 nodes
(also at 128 original labels), and this bootstrap only handles active degree
at most six. Saturation has a cooperative five-second / 4,096-tracking-entry
budget; an in-flight combination may overshoot before returning. Valid partial
tracking is still exported, with an export checkpoint at 4,096 DAG steps.
Skipped or interrupted seeding is never a mathematical conclusion. Global
STOP/winning-peer cancellation is propagated, and every imported DAG is replayed
from original whole configurations before use. There is no inverse-label guess
when the default diagram merges equivalent original labels.

Terminal messages prefixed `Proof: default diagram` report startup, extraction,
retention, and any skipped work; `Proof: saturating default diagram` reports its
node count. See [the diagnostic and integration report](fixpoint-sat-default-seed.md).

### Parallel guided jobs

The shared guided scheduler now dispatches independent bridge, growth, and
repair jobs to a worker pool. `RE_GUIDED_THREADS` defaults to available logical
CPUs minus two, clamped to 1–6; it can be set explicitly from 1 to 32. The
diagram and unrestricted proof workers remain separate. The shared
`RE_GUIDED_MAX_VARIABLES` budget defaults to 1,500,000 and covers active job
reservations plus cached solvers, not an exact RSS bound. Each pool job is
capped at 152,000 variables. See [configuration, cancellation, and memory
accounting](fixpoint-sat-parallel.md). Default seeding and fragment imports
still happen once, not independently in each worker.

To benchmark the real Loop path without providing the known certificate:

```sh
cargo build --release --manifest-path round-eliminator-lib/Cargo.toml --example fixpoint_sat
mkdir guided-results
node round-eliminator-lib/examples/fixpoint_sat/guided-benchmark.cjs \
  round-eliminator-lib/target/release/examples/fixpoint_sat guided-results 180
```

The script writes separate stdout, progress, resource usage, and JSON results.
It never reads the known-certificate fixture; a timeout remains inconclusive.
The [development benchmark](fixpoint-sat-guided.md) records the current hard-case
result: smaller restricted instances, but no certificate within 180 seconds.

The hard-example fixture and its supplied certificate are regression tests:
the certificate's full active derivation is checked against the grammar, and
both compatibility implementations accept it. A further test synthesizes a
certificate from its two proper subderivations, neither of which is itself a
certificate. These tests do **not** claim fast rediscovery from the original
problem alone. That remains a performance challenge.

Neither finite exhaustion nor a budget limit proves unrestricted nonexistence.
No guaranteed terminating all-size YES/NO procedure is claimed.

### Exporting a known-satisfiable benchmark

The native `fixpoint_certificate_cnf` example exports the same certificate
encoding at one fixed step bound, with only original active configurations as
leaves. It does not run the diagram worker or the smaller proof bounds. SAT
here means finding a universal **nonexistence certificate**, not a good diagram.

The supplied hard-case certificate fits a DAG of 21 combination steps after
sharing repeated configuration derivations (not 21 diagram nodes). This is a
verified sufficient bound, not a claim that 21 is minimal. From the library
directory, with output paths that do not already exist:

```sh
cargo build --locked --example fixpoint_certificate_cnf
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf export \
  examples/fixpoint_sat/hard_nonexistence.txt 21 proof-21.cnf \
  src/algorithms/fixpoint_sat/proof/known_certificate.txt known.model
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf verify \
  examples/fixpoint_sat/hard_nonexistence.txt 21 known.model
```

The optional known-certificate arguments produce a separately validated model.
Its parent and permutation choices are supplied as temporary SAT assumptions
only to that validation solve. Neither those choices, intermediate expression
leaves, nor learned clauses are included in the exported CNF. Omitting the two
arguments exports the identical unrestricted formula without validating its
satisfiability. The hard-case export has 751,178 variables and 2,940,191 clauses.

External DIMACS models can be checked using `verify PROBLEM STEPS MODEL`.
Verification checks every regenerated clause, replays the active derivation,
and calls the existing nonexistence oracle; it does not run another SAT search.
The `solve CNF MODEL` command runs the existing native MiniSat core on the same
file for comparison. No solver dependency or GUI default was changed.

`examples/fixpoint_sat/benchmark.cjs` runs CaDiCaL, Kissat, and native MiniSat
sequentially with the same CPU affinity and wall-clock budget, recording
commands, logs, peak memory, and timing. It also independently checks models
against the actual exported DIMACS file, in addition to the Rust derivation
check. See [the benchmark report](fixpoint-sat-benchmark.md) for the measured
results, artifact location, and full reproduction instructions.

The exporter and verifier also accept `--depth=N` and `--symmetry`:

```sh
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf export \
  examples/fixpoint_sat/hard_nonexistence.txt 21 proof-21-depth8-symmetry.cnf \
  src/algorithms/fixpoint_sat/proof/known_certificate.txt known-depth8-symmetry.model \
  --depth=8 --symmetry
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf verify \
  examples/fixpoint_sat/hard_nonexistence.txt 21 known-depth8-symmetry.model \
  --depth=8 --symmetry
```

Depth counts parent edges from original configurations (depth zero), not
diagram nodes or expanded expression size. It bounds every encoded derivation
step. Symmetry breaking orders the left parent's nonpivot occurrence indices
and canonicalizes identical original-label occurrences in both parents. These
are optional **benchmark** constraints, not changes to the GUI's Loop defaults.
The hard-case known derivation is reindexed to satisfy the canonical ordering
before its separate validation; no resulting choices are added to the CNF.
Use the same options when verifying an external model. See
[the Gimsatul comparison](fixpoint-sat-gimsatul.md).

## Validation

The tests independently enumerate small finite lattices and label assignments,
compare them against SAT, check that learned obstructions never exclude the
tested good diagrams (including at other sizes), and compare encoded blockers
with direct term evaluation. They cover merged-label provenance, replay of a
successful diagram, bounded exhaustion, budgets, and preservation of the
symbolic nonexistence certificate. The game is compared with full construction
on the enumerated candidates, and its blockers are checked against good
diagrams across sizes. Dedicated regressions cover cyclic games, winning
strategies that strictly dominate their targets, and target-enumeration pruning.
Automatic names and mapping metadata are tested through GUI requests, including
merged labels, collisions, and replay of diagrams containing `=` inside labels.

```sh
RE_NUM_THREADS=1 cargo test --manifest-path round-eliminator-lib/Cargo.toml \
  algorithms::fixpoint_sat -- --test-threads=1
node --test www/gui-label-mapping.test.cjs
```
