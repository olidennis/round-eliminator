# Implementation review

2026-10-13. Static review of the current model, parser, algorithms, transports,
and UI, comparing the implemented features with their old lib/server/wasm
counterparts. Broader old proof-search modules were inspected for reusable ideas,
not audited as implementations of features we have yet to port. This is not a
new benchmark campaign or a proof that every case is correct. No algorithm or
UI behavior was changed by this review.

## Existing performance comparison

Native release, same ten-CPU Linux/aarch64 machine, configured mimalloc v2 with
option 26 = 0 in both benchmarked libraries. Old timings are reused; current
timings are the final integrated live-pruning measurements.

| Maximization kernel | Original elapsed | Current elapsed | Elapsed speedup | Original CPU | Current CPU | CPU ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Basic, both custom saturations | 27.496 s | 7.211 s | **3.81x** | 272.056 s | 51.534 s | **5.28x** |
| Large fixture, fixed second maximum | 19.636 s | 1.226 s | **16.01x** | 191.012 s | 4.277 s | **44.66x** |
| Matching, fixed step-ten maximum | 103.219 ms | 16.690 ms | **6.18x** | 439.834 ms | 54.721 ms | **8.04x** |

These measure maximization kernels, not complete RE sequences or browser clicks.
Old native ten-worker limits launch ten generation plus ten pruning threads;
the current implementation shares one ten-worker Rayon pool. Full normalized
results were compared. The earlier
[old/adaptive comparison](crates/core/benches/re_comparison/investigation/adaptive-batches.md)
and latest [live-pruning report](crates/core/benches/re_comparison/investigation/live-pruning.md)
retain methodology and raw records. The initial contaminated live-pruning trials
remain excluded. Aggregate CPU is a work proxy, not an instruction/energy count.

## Coloring investigation completed (2026-10-14)

The [coloring report](crates/core/benches/coloring/README.md) investigates the
degree-ten BMM/RE(BMM) incidence slowdown and extends the experiments to all four
coloring kinds. Native CaDiCaL's weak refinement path is the principal cause;
equal output domains with distinct sources also cause duplicate enumeration.
Ordering color behaviors, eager passive degree-two encoding, and grouping domains
with separate provenance reduce RE(BMM) from a 30-second cutoff to 16 ms native
and 397→40 ms wasm. Original inputs, exact General checking and cancellation are
preserved. These three changes are now integrated, with a conservative eager-work
budget and exact lazy fallback for larger encodings. The quoted timings above
are the historical prototype measurements; production confirmation is recorded
in the linked report.

The full review below retains the original findings. The maximization follow-ups
have been completed and integrated selectively; the coloring investigation now
has integrated the selected improvements. Other items remain future work.

### Maximization follow-up

The requested prototypes and their production confirmation are recorded in
[review-improvements.md](crates/core/benches/re_comparison/investigation/review-improvements.md).
Implemented useful-join rejection, prepared alignment traversal, component-balance
rejection, stack compatibility caching, selective exact generated history, and
65–128-label packing. Broad history sharing regressed sparse workloads and was
not selected. The subsequent experiments adopted wider masks and dynamic-mask
history compression, and tested flow-network reuse without selecting it. The non-maximization
items below remain deferred except for the completed coloring investigation above.
The numbered descriptions retain the original review's observations and proposals;
consult the follow-up for the current implementation and measurements.

The later [four-category experiment report](crates/core/benches/re_comparison/investigation/final-four.md)
records the user's next request: switch the server to mimalloc v2, test flow reuse,
wider sets, overlapping phases, and compression/scheduling, then stop. All four
were tested. Wider density-aware masks, dynamic-mask compression and conditional
unit-count grains are adopted; flow reuse and phase overlap stay out of the
production path. The server now uses mimalloc v2. Earlier allocator/timing
statements below describe the static review's historical state.

## Stateless cache reuse implemented (2026-10-14)

The user selected a stateless backend and authorized shared computational state.
`ProblemState` now owns an immutable problem and completed analysis cache on the
client. Rust operations automatically consume applicable artifacts; the common
client API attaches state and merges cumulative new entries. Existing saved
sessions migrate to the new state layout. No persistent workers/backend handles.

Cached output maxima accelerate exact output diagrams; joint maxima accelerate
exact pair/input-aware diagrams without claiming output-only maximality. Zero-round
and coloring checks reuse maxima while retaining concrete counterexample clauses.
Exact results and completed coloring bounds are reused; maximum search skips proved
palettes, zero-round strategies certify every coloring kind, and negative coloring
results exclude zero-round solvability. Partial bounds from cancelled searches are
retained by the UI but do not yet initialize future searches. Basic FP retains its
representation-dependent original partial diagram so its construction is unchanged.

## Current deferred work

Consolidated on 2026-10-14 at the user's request. The historical numbered review
below includes suggestions that have since been implemented or tested and
rejected; use this list for remaining work. These are investigation topics,
without a selected implementation unless explicitly stated otherwise.

### Saved review topics

| Topic | Work to revisit |
| --- | --- |
| Resume partial proofs | Completed maxima and palette/zero-round proofs are now reused. Investigate resuming cancelled searches directly from saved partial-bound snapshots. |
| Simplify unbounded verification | Verify repeated unbounded behaviors directly instead of enumerating redundant dummy colors. Related-query proof reuse is now implemented. |
| Alternative zero-round encodings | Compare membership SAT with the current per-rule/per-port encoding; benchmark specialized passive-degree-two behavior/compatibility searches. |
| Allocation-free set overlap | Test Boolean overlap without constructing an intersection. Vector subset cardinality rejection is already implemented. |
| Multiset enumeration memory | Avoid retaining every visited multiset when uniqueness follows from the family's structure; keep exact deduplication for overlapping domains. |
| Lazy active input obligations | Work with condensed active promises and add only violated obligations, preserving all promised inputs. |
| Diagram computation and storage | Group equal occurrence profiles, build expensive contexts lazily, benchmark exact joint products/coverage, and consider a compact SCC/DAG representation with session migration. |
| Basic FP lattice scaling | Replace quadratic operation tables with direct operations or selective caching when the lattice is large. |
| Runtime scheduling and interruption | Consider budgets for mixed SAT/Rayon workloads, clearer generation/pruning phase accounting, and cancellation granularity in parsing/library calls including flow. Evaluate wasm worker reuse only if startup measurements justify it. |
| UI data and computation controllers | Keep immutable snapshots raw/shallow, consider paging/virtualization for large tables, and share repeated cancellation/progress/controller logic with a small composable. |

### Later coloring follow-ups

- Investigate safe distance-two symmetry breaking: color renaming must permute
  both own-color rows and neighbor-color columns.
- Calibrate the eager encoding budget on larger color/input/output domains.
  The existing threshold chooses eager versus exact lazy encoding; it does not
  bound permitted problems or palette searches.

These follow-ups were identified during the later coloring work, rather than
being part of the original saved review. The three tested coloring improvements
are already integrated.

### Other recorded longer-term topics

- Condensed-configuration normalization: merge equal parts and sort them;
  explicitly deferred by the user, independently of temporary algorithm views.
- Parallel native SAT solvers and possible GPU kernels; neither is selected.
- Greedy distance-two coloring and the design question of optional global
  realizability checks for input promises.
- Revisit measured storage/scheduling policies on other workloads, small-kernel
  parallel CPU overhead, and allocator portability when justified by evidence.
- Starred configurations remain unsupported and explicitly deferred.

Flow-network retention, overlapping maximization phases, broad compression,
unconditional coarser generation grains, unrestricted coloring clause batching,
and extra witness probes were tested without selecting them for production.
They are retained as experiments, not pending integrations.

## First improvements to consider

### 1. Avoid allocating sets when only overlap is needed

In [configurations.rs](crates/core/src/algorithms/configurations.rs),
`allows_positions_with` tests compatibility using
`!offered.intersection(allowed).is_empty()`. This builds an entire intersection
for a Boolean answer, repeatedly during capacity checks and graph construction.

Add an allocation-free `LabelSet::is_disjoint` or `intersects` method using the
sorted storage and stop at the first common label. Also consider the cheap
`self.len() > other.len()` rejection in vector `is_subset_of`. Keep real
intersection operations wherever their contents are needed. This is a small,
direct improvement candidate; its application-level gain is unmeasured.

### 2. Reject parent pairs before intersection/exchange preparation

[combination.rs](crates/core/src/algorithms/maximize/combination.rs) currently
computes the whole intersection matrix and dominated exchanges before discovering
whether there is any incomparable operand pair to join. If all operands are
comparable, it performs that preparation and returns no candidates.

The old [maximizer](../round-eliminator/round-eliminator-lib/src/algorithms/maximize.rs)
finds `good_unions` before constructing intersections. Borrow that order: find
admissible union choices first and return immediately when none exist. This
preserves the existing pruning rule, custom algebras, and trace order; it does
not skip self-combinations indiscriminately. Savings depend on how often parent
pairs have no useful join.

### 3. Share duplicate information earlier, with an explicit memory tradeoff

The old maximizer inserts every normalized generated row into a shared `DashSet`
before local containment pruning. The current engine uses immutable global
history during each generation batch, and publishes only pair-local survivors.
Locally discarded rows can therefore be regenerated and matched elsewhere.

Consider first deduplicating across jobs within one batch, then publishing more
already-covered rows to longer-lived history if memory permits. An atomic
insert-if-absent is preferable to the old separate contains/insert sequence.
Restrict timing-dependent history to untraced execution. Entries must be covered
by retained parents or eventually published survivors; cancellation returns no
maximum. Keep exact keys/equality rather than rejecting solely on a fingerprint.

The tradeoff is real: the old history compressed normalized rows with bincode
and LZ4; an expanded history of owned working configurations could retain much
more memory. Do not add compression to every small packed row by default.
Preserve the public generic engine's current `Ord` contract when choosing a
concurrent collection rather than casually requiring `Hash` from every algebra.

### 4. Reuse completed maxima in diagrams and solvability

Currently [the protocol](crates/core/src/api/protocol.rs) accepts a cached
maximum for Apply RE only. Diagram, zero-round, and coloring controls still
operate on the original constraints. Thus clicking Maximize does not yet make
those operations use its fast path. The old problem object reused its
`is_maximized` passive constraint across these analyses.

Use the separate immutable maximum artifact as an optional argument when it
already exists. Do not compute it implicitly for an unrelated requested action.
For input-independent validity, an output-only maximum can supply fast ordinary
containment. For restricted mappings, a joint maximum needs joint/pair-aware
checking; it does **not** certify that the original output constraint is maximized.
Keep these guarantees distinct, and leave the displayed/source problem unchanged.

### 5. Compare membership-based zero-round SAT with the current port encoding

Zero-round search currently allocates output-choice variables per local rule and
port, then collects their union into `(input, output)` emissions. Coloring already
uses output-set membership variables and shared active matching constraints.

An alternative is one available-output set per original input label, existential
active matching for each local input, and universal passive validity. Under the
current local semantics, unions of a valid strategy's emissions supply such sets;
conversely, selecting an active matching from sound sets supplies local rules.
Recover the configuration/port witness once at the end. This can reduce variables
and share more encoding code, but changes SAT search behavior and needs independent
witness/equivalence tests before replacing the current encoding.

For the plain degree-two case, the old
[triviality](../round-eliminator/round-eliminator-lib/src/algorithms/problem_triviality.rs)
and [coloring](../round-eliminator/round-eliminator-lib/src/algorithms/coloring_solvability.rs)
algorithms also expose a useful behavior/compatibility-graph approach. Minimal
active supports and precomputed forbidden label pairs are worth comparing with
the membership solver. Enumerating supports can itself explode; retain the
general solver and preserve original inputs/nonregular degree obligations.

## Further algorithm and memory opportunities

### 6. Keep genuinely lazy multiset traversal small in memory

`visit_multisets_with` retains a `BTreeSet` of every previously visited concrete
configuration. This prevents duplicates caused by overlapping parts, but even a
single part pays the cost although its count distributions are unique. A long
successful coverage check can retain its entire enumerated language.

Skip the visited set for zero/one-part families and, after a cheap proof, families
with pairwise disjoint label domains. Keep provenance-correct deduplication when
overlapping parts can produce the same concrete configuration. This improves
memory behavior without changing the mathematical search or imposing a limit.

### 7. Reuse matching and alignment preparation

[matching.rs](crates/core/src/algorithms/configurations/matching.rs) recomputes
compatibility after successful capacity rejection, allocates a fresh flow graph,
and obtains a full flow result for a Boolean query. Alignment search rebuilds
conflict adjacency, margins, count storage and heap frames for each union choice
although much of that preparation is constant for the parent pair.

Try reusable scratch storage, prepared conflict indices, and borrowed counted
views before changing matching libraries. Keep the current allocation-free
rejection path when most matches fail. For sparse alignment graphs, connected
component capacity balance is another safe rejection test: every component must
have equal residual row/column demand. This is necessary, not sufficient, so
passing it never substitutes for exact search.

### 8. Extend packed working storage beyond 64 labels carefully

The current adapter falls back to vectors at 65 distinct labels, including joint
input-output alphabets. That introduces a likely performance discontinuity whose
size has not been measured. Two-word masks, multiword bitsets, and interning
immutable sets into IDs are candidates. The old symbolic proof algebra also
demonstrates interning and memoized operations.

Sparse wide alphabets may still favor vectors. Benchmark different densities
and both targets; preserve the public label-set representation and the generic
engine. Finite observation profiles from old symbolic proof search must not be
mistaken for equality of arbitrary problem objects.

### 9. Reduce repeated diagram work

[relation.rs](crates/core/src/algorithms/diagram/relation.rs) precomputes exact
contexts for every label, even when occurrence inclusion proves an arrow. Group
identical occurrence profiles within permitted input groups and compute expensive
contexts lazily. Degree-two neighborhood/profile tables can use dense bitsets.

Diagram results also store all transitive successors plus SCCs and reduced edges.
SCCs plus a condensation DAG can encode a transitive relation more compactly,
with reachability reconstructed for consumers. This needs explicit saved-session
migration and preservation of partial/exact guarantees; the immediate simpler
candidate is reducing repeated comparisons, not changing the wire schema.

### 10. Avoid quadratic Basic FP table construction for large lattices

[lattice.rs](crates/core/src/algorithms/fixed_point/lattice.rs) enumerates every
successor-closed set and then stores all pairwise joins, meets and comparisons.
If the lattice has K values, these tables cost O(K^2); K can already be exponential
in the original label count. Small lattices benefit from the current direct tables.

For larger ones, compare direct packed set operations plus an interning lookup
with memoization of only encountered operand pairs. Preserve the full set of
lattice values initially; constructing only reachable lattice values would be a
separate algorithmic change requiring proof. Do not import the old star threshold
or proof-search cutoffs into our exact finite procedures.

### 11. Reduce repeated verification and reuse existing proofs

The non-distance-two unbounded-witness verifier repeats the same behavior over
a dummy palette and enumerates color assignments. Such a certificate can instead
be checked directly against original active/passive inputs using its one behavior
table, without those redundant color assignments.

Maximum coloring checks also rerun zero-round search even when the card already
has a completed zero-round result. Exact-query caches do not yet derive fixed
palette decisions from a saved maximum, lower bound, or unbounded certificate.
Reuse valid proofs by restriction, keeping original inputs and distinguishing
local-color, neighbor-color, and order witnesses. Never turn cancellation into
an upper bound. Keep the user-selected linear palette search.

### 12. Consider lazy active input obligations

Active inputs are fully enumerated in [inputs.rs](crates/core/src/algorithms/inputs.rs)
for solvability; the passive side stays condensed. A membership-based solver
could request only a violating active input configuration and add that matching
obligation. One route projects the input/output product restricted by the current
memberships onto inputs, then checks coverage of the original input promise.

This is a larger, unbenchmarked idea. Exact coverage can still be exponential,
so neither a speedup nor removal of worst-case enumeration is established.
Retain the input promise and existing local realizability convention, and never
encode feasibility as a narrower input promise.

## Runtime and code organization

- **Production allocator remains undecided.** The current server uses the system
  allocator; the old server uses jemalloc on Linux and mimalloc elsewhere. The
  speedup table uses matched mimalloc-v2 benchmark executables. Select an allocator
  in the native executable, using platform-specific evidence; keep it out of the
  reusable core and wasm. Existing allocator measurements should inform that
  decision rather than copying platform rules without checking.
- **CPU permits currently count requests, not occupied cores.** Multiple SAT
  calls can run on blocking threads while maximization uses the whole shared Rayon
  pool. The pool prevents independent maximization pools from multiplying, but
  mixed workloads can still compete. Consider weighted admission/budgets only
  if concurrent-request responsiveness becomes an issue.
- **Immutable UI snapshots need little reactivity.** App stores cards in a deep
  `ref`, so large problems and cached results can become reactive proxies. Keep
  mutable card settings reactive and consider raw/shallow immutable problem and
  result payloads. Large constraint/strategy tables may eventually need paging
  or virtualization. Preserve collapse without cancelling computations.
- **Computation controllers are repeated across Vue components.** A small Vue
  composable could share cancellation, stale-response guards, progress and error
  state, while each analysis retains its own result cache. This is a justified
  potential cleanup; a general operation/plugin framework is unnecessary.
- **Progress/cancellation gaps deserve targeted checks.** Known pair progress is
  reported during generation, before global pruning completes; long pruning can
  make a displayed percentage pause. Parsing mapping entries and some library
  calls also have coarser cancellation granularity. Add checks/clearer phase
  accounting where needed, keeping current delivery coalescing and avoiding a
  claim of predicted overall completion time.

## Choices to keep

### Preserve the distinction between domination and semantic coverage

This matters when proposing stronger pruning. Counted flow tests a **uniform
matching of whole label sets to containing target parts**. It is sufficient for
semantic language inclusion, but not necessary, even for one target row:

```text
target: AB AC BC
family: A BC^2
```

The family produces only `ABB`, `ABC`, and `ACC`, all admitted by the target.
However, only the target's `BC` part contains the entire set `BC`, and its
multiplicity is one, so it cannot uniformly cover both occurrences of that part.
The flow domination check correctly returns false for its intended relation.
The exact coverage fallback correctly handles the larger semantic relation.

Do not discard maximization rows merely because their concrete language is
covered by other rows. A complete maximum must still cover every admissible
condensed family by the domination relation used in fast containment checks.
Changing that invariant could invalidate `PassiveMode::Maximized` and the
portwise enlargement argument for Apply RE. The current code keeps the necessary
distinction; degree-three examples like this should accompany future pruning
changes, beyond existing small-degree exhaustive tests.

Keep the canonical mapped problem with trivial/explicit input; separate name
tables; independent graph class; counted configurations; private parser pairs;
generic ordered algebra for maximization; distinct output-only/joint guarantees;
immutable original problems and external result caches; RustSAT abstraction;
independent witness verification; generated TypeScript protocol; thin adapters;
bounded shared Rayon pool; coalesced progress; cooperative native cancellation
and worker-termination wasm cancellation. The module structure broadly follows
these responsibilities. No evidence here calls for changing Vue or Axum.

Do not borrow old automatic computation of unrelated analyses, mutate the source
problem to store maxima, restore original paired metadata, add starred
configurations, or introduce heuristic limits on exact searches. Parser/model
normalization remains the user's deferred choice; normalize temporary algorithm
views if needed without changing public equality incidentally.

Suggested sequence: small allocation/preparation improvements; measured duplicate
sharing; use computed maxima; compare zero-round encodings; then address wide
alphabets and lattice growth. Every performance claim needs representative
benchmarks when that specific change is authorized, with total CPU as well as
elapsed time. No new measurements were made in this review.
