# Native SAT fixed-point search

Native default-feature builds use three cooperating searches for the existing fixed-point `loop`
operation when the active degree is finite and positive and the passive degree
is two. The browser/WASM and `onlyrust` builds keep the original implementation.
Other degrees and partial fixed-point searches also keep the original path.
The partial-extension code requires an invertible original-label mapping,
whereas SAT allows label mergers.

The diagram/game search looks for a fixed point. The general proof search independently
looks for a universal nonexistence certificate. A witness-guided worker searches
small recombinations of actual game derivations. They run on three scoped worker
threads; a conclusive answer cancels and joins the remaining workers. No GUI button,
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
most 12 configurations and initially searches one, two, then three **additional**
steps. Later rounds extend this local bound one step at a time, up to eight;
a difficult earlier bound does not prevent trying a longer bridge.
The fixed subexpressions already contain their earlier construction: three new
steps does not mean expression depth three. Compatibility between concrete
terms is evaluated before SAT; the unknown circuitry concerns the few new
steps. Non-pivot coordinate permutation symmetry is removed. The native solver
is still the existing MiniSat dependency; no extra solver or mirror nodes were
added. SAT results are replayed and verified with the unchanged `is_pred` oracle.

The scheduler combines blocks of six fragments in pairs, so old and new blocks
can meet instead of searching only consecutive arrivals. Pending subsets can
be replaced by their supersets. Fresh batches alternate with unfinished batches;
conflict budgets start at 2,000 and increase on retries. A cache retains native
solvers and learned clauses up to 300,000 total SAT variables; cold retries
remain scheduled if a solver does not fit the cache. A user-supplied conflict
limit instead gives each bound a bounded attempt in each expansion round,
and disables retries after the final expansion.

This optional accelerator caps retained derived configurations at 256, expanded
configuration trees at 4,096 nodes, and distinct fixed subterms in each batch at
512. These are heuristic resource limits, not completeness claims. Deduplication
uses syntactic equality plus commutativity/idempotence, never equality of values
in one finite diagram. The unrestricted proof worker remains available, with
its original inputs and grammar. Neither local UNSAT nor a local budget limit
is reported as a global conclusion.

No new GUI control is needed: native **Loop** automatically starts this worker.
Terminal events prefixed `Proof: guided` report fragment counts, new-step bounds,
SAT variable counts, local exhaustion/budget limits, and verified certificates.
The diagram-only API still does not run this worker. With `use_game: false`,
the guided worker has only original configurations; full-checker witnesses
continue to feed the general proof worker as before.

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
