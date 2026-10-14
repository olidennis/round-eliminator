# Round Eliminator 3 project brief

This branch is a ground-up rewrite. The source worktree is `/workspace/round-eliminator`
on `llm-enhanced`; this worktree is `/workspace/round-eliminator-3` on `re3`.
Keep both worktrees. Read only `round-eliminator-lib`, `round-eliminator-server`,
`round-eliminator-wasm`, and the GUI files needed to understand them in the old tree.
Keep this worktree's `.git` pointer relative to the sibling `round-eliminator`
folder. Absolute `/workspace` paths break Git when these folders are opened on
the user's Mac. Git worktree repairs may rewrite this pointer; restore it to
`gitdir: ../round-eliminator/.git/worktrees/round-eliminator-3` afterwards.

## Goal and scope

- Build readable, fast, idiomatic Rust, with coherent modules, tests, and comments.
- Keep abstractions few and justified, but leave room for all old GUI features and future features.
- After each substantive edit, assess whether the code still belongs in its
  current modules. Suggest useful restructuring as the code grows, without
  splitting files merely to give each type its own file.
- Apply each change consistently across related code, types, variants, variables,
  protocol names, generated bindings, tests, documentation, and the feature list.
  A naming change must update the implementation as well as displayed labels.
  Preserve saved files and links through explicit migration when needed; keep
  obsolete names confined to migration code and its tests.
- Eventually handle non-regular active/passive graphs and problems with input labels.
- Users may specify possible active and passive degrees separately. If omitted,
  infer the degree sets from the supplied problem, then retain the resolved
  graph class independently of later changes to output constraints.
- For all variants with inputs, infer omitted degree lists from the input
  constraints only (the input projection for paired problems).
- For configurations of input/output pairs, the graph comes with a valid
  solution of the input projection. Users may also specify degree restrictions
  for this representation; store the resolved graph class in every variant.
- Enforce model invariants through checked construction and controlled mutation:
  group configurations by their computed degree and validate label references.
- The first version supports exact degrees only. Reject starred configurations;
  the user may choose to add them in the future.
- Inclusive exponent ranges such as `^5..8` expand during parsing into exact
  multiplicities, independently per part. Zero occurrences omit the part.
  `Part::new` accepts zero multiplicity; `CondensedConfiguration::new` discards
  zero-count parts, so direct Rust construction follows the same rule.
  Do not impose a limit on the number of expanded configurations. Keep ranges
  out of the algorithm-facing model.
- Input-independent output validity is a pair of ordinary input/output problems.
- Input-dependent validity may be expressed either as allowed multisets of
  `(input label, output label)` pairs on each side, or as an input problem plus
  output constraints and a map `input label -> allowed output labels`.
  The latter is described in Section 6.1, Definition 6.1 of
  https://arxiv.org/pdf/2510.17639 (printed page 38).
- Keep all four text-entry forms, but convert them once to a single mapped
  `Problem { graph_class, input, output, mapping }`. `Input` is `Trivial` or
  `Explicit(ConstraintPair)`. Trivial input represents one configuration at each
  graph-class degree without allocating dummy labels or constraints.
  `OutputMapping` is `Unrestricted` or `Restricted(Vec<LabelSet>)`; restricted
  entries index input IDs, with one implicit entry for trivial input.
- Plain entry uses trivial input and unrestricted mapping. Independent entry
  uses explicit input and unrestricted mapping. Mapped entry supplies both.
  Paired entry projects the original input promise and interns each distinct pair
  as an ordinary output ID, restricted to its originating input. Preserve condensed
  parts and multiplicities. Readable tag names are `input,output`; these are just
  names after conversion, not structured pairs for algorithms.
- The user explicitly rejected preserving metadata for the original paired entry:
  show the converted problem and witness directly. Future relaxations and other
  transformations act on the same model users see. Do not add decoding tables or
  original-entry presentation wrappers.
- `Part<L>`, `CondensedConfiguration<L>`, `Constraint<L>`, and `LabelSet<L>` share
  container code for `LabelId` and `LabelPair`, but algorithms on `Problem` use
  ordinary output IDs only. `ConstraintPair` owns one name table and ordinary
  active/passive constraints. `LabelPair` and `InputOutputConstraintPair` are
  private parser details in `parser/paired.rs`, used for paired-entry validation,
  projection, and conversion. Their name tables are temporary. Do not export
  either from the model or derive Serde/TS for them. Keep pair-specific validation
  tests in `parser/paired/tests.rs`. Algorithm equivalence tests use tuple-based
  original constraints as an independent oracle, without importing parser types.
- Model fields are private and constructors check multiplicities, degree
  overflow, label references, and mapping completeness. Start parses text once.
  Analysis requests carry the structured problem returned by Start. The user
  explicitly trusts the client: derive Serde deserialization directly on the
  model and accept its snapshots without repeating semantic validation. Keep
  validation in Rust constructors for newly built problems. Do not introduce
  mirrored data types or a validation layer for these trusted requests.
  Graph classes may contain degrees with no valid outputs.
- `LabelSet` has private sorted-vector storage and set operations. Use it as the
  default abstraction and benchmark realistic workloads before changing storage.
  Its JSON shape is an array. CondensedConfiguration normalization remains deferred.
  `Constraint::new` discards structurally identical configurations, preserving
  the first occurrence within each degree. Paired input projection can create
  duplicates even when the original output configurations differ; deduplicate
  in the common constraint constructor rather than hiding repeated rows in the UI.
- `Configuration<L>` represents one concrete multiset as sorted, unique
  `(label, multiplicity)` entries. Merge duplicate labels and discard zero counts
  in construction. Enumeration and membership use this counted type; occurrence
  vectors remain only where port order or SAT variables per port require them.
  The user prefers a library when measured runtimes are comparable. Native and
  wasm matching benchmarks compared the original matcher with petgraph Dinic
  and Edmonds–Karp. Select Dinic with elementary single-label/single-part checks
  and capacity rejection: complete wasm checks were comparable, repeated counts
  much faster, and dense graphs avoided Edmonds–Karp's slowdown. Small individual
  matches are slower; do not claim a universal speedup. Keep the reproducible
  benchmarks in `crates/core/benches` when revisiting this choice.
- Consider GPU execution only when a concrete algorithm and benchmark justify it.
- The GUI should be able to use both wasm and a server through one coherent API.
  The initial backend is Server on localhost or 127.0.0.1 and WebAssembly
  elsewhere; an explicit ?backend=server or ?backend=wasm overrides this default.
  Avoid adding an independent manual request mapping in both JavaScript and Rust.
- Framework choices, including Vue, are open.

## Workflow

`FEATURES.md` inventories existing GUI behavior and tracks unimplemented work.
After implementing a feature, update its status and any related controls or
displayed results there. Keep old button names alongside renamed equivalents;
audit against the actual UI and API rather than relying on the summary prose.
Start now accepts all four problem variants, sends their text to Rust,
and displays the returned definition. The user selected a typed Rust protocol
with generated TypeScript types, thin server/wasm adapters, a degree-indexed
symbolic constraint model, and Vue with TypeScript. Start does not compute
analyses. `README.md` documents entry notation and the module layout.
For each later feature, discuss meaningful alternatives and their
tradeoffs with the user, wait for their choice, then implement the selected
feature with tests and comments. Do not import old algorithms wholesale or
implement further features ahead of that choice. Treat the source tree as
reference, not as code to preserve for its own sake.

GUI presets in `web/src/presets.ts` fill the editors and leave Start as the
explicit parsing action. Leave both preset degree fields blank so the graph
class is inferred. Include classical graph problems, input promises, and
equivalent mapped/paired list-coloring examples; check both transports using
`web/scripts/test-presets.mjs`. Loading an example copies its text definition,
so editor changes never mutate the preset.
Include the small mapped input-aware label replacement preset: i permits A/B,
j permits C; passive outputs are `A C / BC^2`. Output-only strength is A → B → C,
while both input-related diagrams retain A → B and exclude B → C because i
forbids C. Cheap and exact both display one arrow. Keep degree lists inferred.

Coloring solvability must let strategies use both the given coloring
and all original local inputs, retaining the input promise and output mapping.
Orientation is an ordinary input promise, combined with other inputs when
present; dedicated orientation search algorithms are unnecessary. The user
authorized the SAT coloring implementation and streamed partial palette results.
Include orientation-input coloring GUI presets:
3-coloring/outdegree 1/degree 3 and 5-coloring/outdegree 2/degree 5, with inferred
graph classes.

## Coloring solvability decisions

The user approved membership SAT for vertex/strong hypergraph coloring, graph
edge coloring (degree-two passive nodes, matching colors across an edge), and
incidence edge coloring (distinct at both sides). Strategy choices may depend
on the entire active input/color configuration and port order. A witness stores
available output sets per original `(color, input)`; existential active matching
and universal passive validity certify these sets. Preserve all four entry
forms through the same canonical model. Share active matching, original input
preparation, and passive checking with zero-round. Support general and maximized
passive modes; the GUI uses general until computed maxima are integrated into
these checks.

Maximum search first checks for a repeatable color-independent zero-round
strategy, which certifies unbounded solvability. The user replaced doubling and
binary search with linear search for every coloring kind, including distance-two:
try consecutive palettes from the local minimum and stop at the first UNSAT.
Large jumps caused impractically expensive queries (bipartite maximal matching
with incidence colors: SAT at 10 led to a query at 20 instead of 11). Reuse one
incremental solver with color
activation assumptions, gating active obligations when colors are disabled.
The degree-ten bipartite matching regression (`M U^9 / P^10`,
`M UP^9 / U^10`) must query 10 then 11 and return maximum 10. The degree-three
preset has maximum three. Candidate counts measure proposed SAT models examined
by refinement, not time or internal solver work; native CaDiCaL and wasm BatSat
can produce very different counts. Compare elapsed times with native release
builds before drawing backend performance conclusions.
Finite behavior vectors and bounded degrees ensure an unbounded palette would
give a repeatable behavior, so no heuristic palette cutoff is necessary.
Also support fixed-palette decisions and independent witness verification.

The user requested partial results while searching: publish proved lower bounds
with witnesses, inclusive upper bounds, and the current palette through generic
`ComputationEvent::PartialResult` events on both transports. `Progress` remains
generic phase information, separate from these mathematical results. The GUI
displays partial proofs and retains them on cancellation; interruption is never
an UNSAT decision. Analysis reuses the parsed structured problem and returns no
problem echo. Keep browser strategies in `ColoringView.vue` and query/progress
state in `ColoringAnalysis.vue`. The coloring strategy explanation belongs in a
"Reading the strategy" help tip collapsed by default, independently of Show strategy.
Both analyses use `ComputationProgress.vue`.
Tests compare small compact encodings with independently constructed explicit
colored input problems, verify witnesses, and check both transports using
`web/scripts/test-coloring.mjs`. No performance superiority is claimed without
benchmarks; further coloring optimizations are listed in FEATURES.md.

## Distance-two coloring decisions

Standard distance-two coloring decision: the user corrected the initial
information model: an active node knows its own color and all neighbors' colors.
Implement this for degree-two passive nodes only, without a greedy coloring
promise. The initial fixed-palette version is now extended to maximum search.
Do not reuse the color-independent unbounded certificate, which applies to the
three color kinds without neighbor information. Keep original inputs and output
mapping available to strategies.
Distance-two membership keys are `(own color, neighbor color, original input)`;
active matching handles the whole configuration with pairwise distinct neighbor
colors excluding the center. Passive endpoints reverse the two color indices.
On trees, remaining endpoint contexts combine independently, so the compressed
representation is exact under our existing local-input semantics. Original
passive input alternatives stay condensed. `coloring/distance_two.rs` owns this
incremental numerical search and its independent non-SAT witness checks; it reuses shared
constraint encoding, color-placement helpers and passive verification. Both
passive modes work. `ColoringWitness` distinguishes local-color and neighbor-color
certificates; its matrices index actual colors, with empty diagonal entries.
The UI uses the existing request, shows own and neighbor colors, and explains
that output choices are made as whole active configurations. Test against
independently materialized tiny colored input problems, including all four
entry forms, nonregular degrees, mapping, cancellation and both transports.

The user approved the order-based unbounded check: a node uses only each
neighbor's smaller/larger comparison with its own color, plus all original
inputs, to choose a whole valid active output. `coloring/order.rs` reduces this
to the existing zero-round solver using two temporary input labels per original
input, preserving condensed constraints and reversing comparisons at each edge.
Returned `ColoringWitness::ColorOrder` stores the smaller/larger output tables;
the UI shows `Any / Smaller` and `Any / Larger`. Check this certificate without
SAT by testing all comparison patterns and opposite-endpoint emissions.
Completeness under local/tree semantics uses finite Ramsey: classify palette
pairs by both directional output tables over all inputs; a homogeneous subset
of size max(active_degree + 1, 2) covers every comparison pattern. Thus failure
proves a finite numerical maximum. Do not compute a huge Ramsey bound or add a
heuristic palette cutoff. Both numerical searches share `search_maximum` in
`coloring/mod.rs`. Distance-two queries retain memberships, active encodings and
learned clauses in one solver; gate active obligations by all their colors and
disable out-of-palette memberships when shrinking. Retain streamed proofs on
cancellation. `coloring/tests.rs` covers the original three kinds, while the
larger `coloring/distance_two_tests.rs` covers neighbor colors and order
certificates; keep these adjacent files rather than making a test-only folder.

## Zero-round solvability decisions

Zero-round results read "NOT solvable in 0 rounds" or "The problem IS solvable
in 0 rounds". Keep the strategy table visible, with its explanation under a
"Reading the strategy" help tip collapsed by default.

The user explicitly deferred changing the native solver for parallel SAT solving.
Keep CaDiCaL for now. A future evaluation may benchmark CryptoMiniSat or another
parallel backend, with incremental assumptions and working cancellation required.
Reuse RustSAT encodings and retain the separate wasm backend; do not integrate
a new solver before the user requests that work. Track this in FEATURES.md.

The user approved a shared SAT approach for all four variants, then RustSAT
so the backend can be replaced without rewriting the encoding. Use RustSAT's
common literal types and generic interfaces, with packaged CaDiCaL 2.2.1 for native
builds and pure Rust BatSat for wasm, selected through target-specific dependencies.
Keep backend construction/cancellation setup in `algorithms/sat.rs`; avoid
solver-specific operations in problem encodings. Select outputs per active input occurrence (full
input context and local port order), not just per input label. Encode active
validity as matching to condensed parts with capacities; do not expand output
permutations. Encode passive degrees up to two eagerly. At higher degrees,
solve, verify exactly against the union of passive configurations, and add a
counterexample clause until a witness or UNSAT is obtained. No maximization
precondition or arbitrary search limits. Keep an independent witness checker
and tests comparing against exhaustive search on small instances.

Passive verification decision (2026-10-11): the user approved adopting two
modes after native/wasm benchmarks. Both keep passive inputs condensed and share
emission preparation and the SAT loop. `PassiveMode::General` is the default;
it tries counted containment first, then checks output multisets lazily against
the whole constraint if containment fails. `PassiveMode::Maximized` uses the
same containment check and can reject immediately on failure, blocking the
selected emissions. Its caller must guarantee that every maximal passive
configuration is listed, up to permutation. The user explicitly requested
hand-written maximized fixtures before the maximizer was implemented. These
independent fixtures remain useful verification tests.
Do not keep the expanded-input or direct SAT containment implementations in
production. Historical results are in `crates/core/benches/passive_inputs/results.csv`;
the runner now compares the two adopted modes without source patching.
Containment can safely accept a family without maximization, but failed
containment cannot reject it unless maximization is guaranteed. Temporary merging
of output parts with equal choices and origins is not model normalization.
The GUI currently sends general mode. Integrating cached maxima and adding a user
choice are later work; total cost includes maximizing the constraint. Do not add a stale maximization
flag to `Problem` ahead of transformation semantics.

The user approved removing the zero-round `View` struct. General borrowed
accessors (`graph_class`, `input`, `output`, `mapping`) and incidence restrictions
(`output_allowed`) live on `Problem`. Algorithm-specific rule/domain preparation
remains in `algorithms/solvability/zero_round/prepare.rs`; shared original input preparation
and absent-input conventions live in `algorithms/inputs.rs`.
The former `Validity` selector is removed: search and verification use the shared
ordinary output constraints directly. The model must not depend on zero-round
witness types or SAT preparation.

Module placement after the organization review:

- Group zero-round, coloring, and their strategy-specific shared helpers in
  `algorithms/solvability/`: `zero_round`, `coloring`, and `passive`.
  The user corrected the broader grouping: generic operations must live outside
  solvability. Keep `configurations` and `sat` directly
  in `algorithms`. These modules must not depend on solvability types or policy.
  Use the operation's purpose to choose placement, rather than its current
  callers or whether its Rust signature has type parameters. Future unrelated
  algorithms can be siblings of `solvability`; keep tests beside their modules.
- Prefer inline `#[cfg(test)] mod tests` for manageable test suites in standalone
  source files. Avoid creating a directory solely for one small test module.
  Larger algorithm folders can keep neighboring `tests.rs` modules.
- The user selected `model` as the domain module name. `model.rs` only declares
  private implementation modules and reexports their public types. The problem,
  input/mapping enums, and construction error live in `model/problem.rs`; sibling
  files own labels, constraints, configurations, and graph classes. No `mod.rs`
  is needed for this module. Use `crate::model::{Problem, LabelId, ...}`.
- `model/labels.rs` owns `LabelId` and `LabelSet`, reexported from `model`,
  plus the shared internal label-name validator. Keep model types together.
- `model/configuration.rs` owns `Configuration`, `CondensedConfiguration`, and
  `Part`, including their multiplicity validation.
- `model/constraint.rs` owns `Constraint` and `DegreeGroup`: grouping and lookup,
  including `Constraint::configurations_at_degree`. Matching and concrete
  enumeration remain algorithms in `algorithms/configurations.rs`. Generic
  count-distribution stepping (`advance_counts`) is shared there by multiset
  enumeration and passive counterexample checking, rather than duplicated.
- Generic SAT cardinality operations (`Formula::exactly_one`, `Formula::at_most`)
  and their tests belong in `algorithms/sat.rs`, alongside backend setup. Use
  RustSAT Ladder for at-most-one and Totalizer for larger bounds. Complete native
  and wasm checks on 19 fixtures favored the library over the former custom
  sequential counter; see `crates/core/benches/README.md`. Keep a shared variable
  manager and check cancellation while streaming generated clauses to the solver.
  RustSAT CollectClauses exposes only an out-of-memory error: the private adapter
  stores the actual cancellation/solver error, aborts generation through that
  channel, then restores the original error before returning to callers.
  Generic matching against a constraint is encoded by `encode_constraint` in
  `algorithms/solvability/constraint_encoding.rs`. The user requested that this
  shared solvability encoding live with its searches. It supports activation gates, used by
  palette queries, and applies to either side of a problem. Both solvability
  checks call it for active validity.
- Original input preparation lives in `algorithms/inputs.rs`, shared with diagrams, with concrete
  active inputs and condensed passive inputs, without algorithm witness types.
  `algorithms/solvability/passive.rs` owns shared verification modes, emission families,
  containment, exact checks and blocking clauses; input keys can include colors.
- Zero-round `mod.rs` holds local-rule/result types, entry points, and search;
  `prepare.rs` prepares initially empty rules and output domains;
  `verify.rs` verifies complete witnesses using general mode. Generic counted
  containment belongs in `algorithms/configurations.rs`, alongside membership.
  `zero_round/tests.rs` holds search tests and independent exhaustive helpers;
  `zero_round/tests/passive.rs` groups passive-family and blocking-clause tests.
- `algorithms/solvability/coloring/` owns palette types/search (`mod.rs`), color placement
  (`prepare.rs`), incremental membership SAT (`encoding.rs`), independent witness
  checks (`verify.rs`), and tests. Color placement is implicit; do not materialize
  a colored input ConstraintPair. Original inputs and mapping remain intact.
- `parser/paired.rs` owns the private `LabelPair`/`InputOutputConstraintPair`
  entry types, input projection, and conversion to the shared model.
- `api/mod.rs` owns shared dispatch and JSON entry points; `api/protocol.rs`
  owns request/response/event schema and API errors; `api/tests.rs` owns API tests.
  Use the public `api` module in both adapters. Keep native stream lifetime
  handling in the server crate, and JS/wasm boundary handling in the wasm adapter.
- Core has four responsibility areas: `model`, `parser`, `algorithms`, `api`.
  `computation.rs` stays a shared root module because parsing and algorithms both
  use its progress/cancellation context. Do not place it inside either area or
  create a directory just to contain this one small file. Rust file modules and
  directory modules are peers; folders group several related implementation files.
  Keep `lib.rs` focused on module declarations and convenient public exports.

The current decision criterion requires every listed active input configuration
at an allowed degree to have a rule, and all compatible passive combinations to
be valid. It does not solve global realizability of the input promise. Returned
rules sort input occurrences by label ID and break ties by local port order;
outputs align with this order. Plain rules use local port order directly.
The protocol request `CheckZeroRound { problem, passive_mode }` takes the already parsed definition
and returns only `ZeroRoundResult`; witness IDs refer to the submitted problem's
label tables already displayed by the UI. Do not echo the problem in this response.
It does not parse notation or expand exponent ranges again.
The UI calls the same typed request through either transport. Native CPU work
runs outside the async executor with concurrency bounded by available CPUs.
Start only parses; the zero-round button belongs below the constraints inside
the parsed problem. It transforms into phase progress and Cancel while running,
then displays the result below. Analyze the displayed structured problem,
independent of subsequent editor changes. Closing a problem or importing a
session cancels the removed problems' running analyses. Start appends rather
than replacing any existing problem. Cancellation leaves a retry button.

Progress transport policy: native CPU callbacks write to latest-value watch slots
and never wait for the UI or serialize progress. The async stream samples every
100 ms; its final response uses a separate oneshot and immediately flushes the
latest progress and proof snapshot. Wasm throttles using performance.now before
JSON serialization, callbacks, and worker messages, then flushes on completion.
PartialResult is a cumulative, self-contained snapshot (ColoringBounds or a
complete source MaximizationResult),
so superseded updates can be dropped without losing the best proof. Do not use
this channel for additive deltas without revisiting its delivery policy.
Cancellation checks must stay independent of transport throttling. Keep tests
that leave a 100,000-update stream unread and verify work finishes, CPU permits
are released, and the latest bounds and final response survive.

## Maximization

Maximization is implemented in algorithms/maximize.rs (snapshot preparation and
result schema), maximize/engine.rs (generic closure and history),
maximize/combination.rs (counted pair generation and exchange pruning), and
maximize/antichain.rs (dominance caches and batch pruning). The engine takes counted
Configuration<T> values and an Operations<T> algebra, with an optional derivation
observer. Configuration construction accepts ordered non-Copy values as well as
label IDs. Empty-set admissibility stays in the algebra, preserving Part's model
invariant. Normalize working configurations only; do not change model-wide
condensed equality. Shared count-matrix visitation and flow matching live in
algorithms/configurations/{alignments,matching}. InputOutputLabel in model/labels
is shared by joint maxima and diagrams, replacing the diagram-specific node type;
its JSON shape is unchanged. The Maximize request reuses parsed snapshots and
returns only a complete MaximizationResult. Server and wasm need no manual
request-specific adapter logic. Browser MaximizationAnalysis.vue owns progress,
cancellation, retry, and the per-problem result; session files/links retain it.
Do not automatically compute it on Start, or replace displayed constraints.
Apply RE now consumes the source maximum. Current diagram/solvability consumers
still use their existing methods; wiring the stored maximum into those computations
is later work. Generic closure correctness is tested against exhaustive degree-two
languages, degree-three condensed families, 441 joint relations, empty values,
derivations, cancellation, and serial/parallel agreement. High-degree mapped
inputs stay condensed. web/scripts/test-maximization.mjs checks both transports.
The original maximization bench records a baseline. Production optimizations and
measured old/new comparisons are documented in the investigation report below.
Small-pair scheduling was much slower than serial work; the engine estimates
matrix freedom to use workers only for larger batches, without search limits.

The user requested an old/new native RE comparison and a fair maximization-only
comparison. Reproducible code, exact old test_all's second fixture, dependency
lock, individual timings, and summaries live in crates/core/benches/re_comparison.
Its runner uses a temporary Cargo package; do not add the old library to production
dependencies or patch either algorithm for measurement. Compare ten raw RE steps
on degree-10 bipartite matching and two actual RE calls on the long-label fixture,
plus the old button's automatic cleanup workflow. The old test helper with steps=2
has a different scope (one Speedup plus passive maximization).
For kernel comparisons, feed both maximizers identical old-sequence constraints,
check normalized maxima and aligned full RE outputs outside timing, reverse
implementation order across samples, and distinguish optional single-sample
cleanup runs from three-sample main results. RE3 is faster early but slower in
later/harder maximizations in the original baseline; do not claim a universal
speedup. The initial request added measurement tooling and an optimization TODO;
the user subsequently authorized integrating all validated improvements.
With worker limits of 10 and native release builds, three-sample sequence medians were
296 ms old versus 683 ms new for ten raw matching RE steps; the full old button
workflow took 379 ms. Identical-input maximization totals were 297 versus 550 ms.
Two raw RE calls on the long-label fixture took 24.422 versus 36.469 s; the full
old button took 24.840 s. Shared-input maximization totals were 24.314 versus
40.107 s. The hard second maximization took 24.182 versus 40.078 s, while the
first took 109 versus 29 ms. All paired normalized maxima and aligned complete
RE outputs agreed. Do not subtract independent-sequence and shared-input totals
to infer conversion overhead: source numeric IDs and row order can differ.

Follow-up profiling and controlled prototypes live in
crates/core/benches/re_comparison/investigation; investigate.py instruments only
temporary library copies. These paragraphs record the original baseline and
prototype stages; the production integration is described below.
The old native worker limit of 10 creates ten generation plus ten pruning
workers; RE3 has one ten-worker Rayon pool and serial global pruning, on the
same ten logical CPUs. test_all itself does not choose the count. Do not call
an old worker limit of one single-threaded: it still has both pipeline stages.
For the fixed matching step-ten input, missing early two-by-two intersection
exchange pruning creates 345,625 candidates versus 10,117 old. For the fixed
hard step-two input, both construct 945,393 candidates; the new run spends
28.32/39.01 seconds merging and performs 127.39M global containment checks
versus 57.40M old. Capacity rejection before graph allocation cuts the large
run to 19.34 seconds; a recent-dominator cache cuts checks to 51.31M. Batches
of 256 instead of twenty reduce generation from 10.42 to 4.43 seconds.
The combined temporary prototype takes 63.1 ms versus current 256.0 ms on
matching (three-run medians), and 10.08 versus 39.01 seconds on the hard input
(single diagnostic runs). Every variant's complete maximum matches baseline;
10 maximization and 11 generic configuration tests pass on the combined variant.
Naive shared Mutex-based deduplication and reversing scans did not help. Preserve
the generic algebra, derivations, empty values, condensed inputs, cancellation,
wasm and deterministic merging when integrating optimizations. Batch size 256
is experimental, not a universal policy. No wasm speedup has been measured.

The user then requested trying parallel global pruning. parallel_pruning.rs in
the benchmark folder supplies two temporary variants: parallel-global rejects
against an immutable antichain then retains serial ordered admissions;
parallel-antichain also computes old-row removals and new-row maximality in
parallel, applying masks deterministically on the calling thread. Observed
derivations use the ordered variant. Each task slot keeps its own dominator cache;
there is no lock around a shared mutable antichain. compare_pruning.py measures
sequentially, alternates order, and checks complete result equality.
Three-run medians on the same frozen inputs and a ten-worker pool: matching
62.54 ms with serial pruning/batch256, 62.07 ms serial/batch4096, 53.18 ms
parallel/batch4096; hard fixture 10.161, 9.789, 6.425 s respectively. At the same
batch size, global pruning time drops from 5.74 to 2.38 s and generation stays
about 3.8 s. Parallel caches perform more containment checks (77M versus 51M)
but reduce elapsed time. Direct Rayon collection did not prove an extra benefit;
the repeated chosen comparison uses the existing map_with_progress helper.
The selected parallel variant passes 12 maximization tests (including two
prototype-specific tests) and 11 generic configuration tests. All full maxima
agree. Detailed trials and stage-tagged results are in investigation README.md
and parallel-experiments.jsonl. These figures describe the prototype stage;
wasm performance is unmeasured.

When asked why global pruning does not scale with cores, additional timers on
one selected-variant run measured 2.303 s global: serial duplicate-history
processing 1.043 s, combined worker dispatch/check/collection phases 1.138 s,
other bookkeeping 0.122 s (mask application itself 0.008 s). The previously
reported 2.377 s is a three-run median. About half the remaining stage is serial;
it processes/clones 945,393 candidates into a BTreeSet. Parallel caches also do
77M versus 51M containment checks. Equal-count task chunks can have unequal
cost and require barriers; do not present their individual contributions as
measured. Follow-up counters and the stage=breakdown result are preserved in
the investigation report. The next concrete targets are history processing,
cache effectiveness and task granularity, rather than increasing worker count.

Production integration (2026-10-13): the user authorized all validated improvements.
Generic matching now rejects insufficient compatible capacity before allocating
flow networks. Pair generation precomputes dominated two-by-two exchanges; above
256 compatible cells this optional shortcut is skipped, never the exact search.
Antichain insertion caches recent dominators, including sound removed cache rows.
Native batches scale as workers*512, clamped to 512..4096; wasm batches remain one.
Large global merges parallelize rejection and maximality with persistent per-slot
caches and ordered masks. Observed derivations retain serial ordered admissions.
Large histories use parallel immutable membership queries before ordered BTreeSet
insertion (history>=4096 and batch>=2048). Duplicate candidates keep their first
derivation. Optional derivations are boxed internally so untracked candidates
stay compact; public observer/algebra bounds and schemas are unchanged.

Detailed follow-up counters separated global checks: serial cached/scan/removal
5.20M/36.37M/9.75M; parallel cached/scan/removal/new-peer
5.38M/46.32M/20.07M/5.25M. Thus extra checks come from frozen scans, removing old
rows against candidates later dominated within the batch, and new-peer comparisons.
Reducing new candidates serially first lowers checks to about 59.20M but does not
improve elapsed time. Borrowed history lookups before cloning, sorting histories,
cache refresh, and promoting cache hits did not establish useful gains. Concurrent
history queries gave about 3% on the large diagnostic fixture. Retain these trials
in the investigation report; do not integrate every experiment indiscriminately.

Three-sample uninstrumented production reruns with ten workers are in
investigation/production-results.jsonl. Ten matching raw RE calls: median 133.118 ms;
two long-fixture calls: 5.613 s. Shared-input maximization totals: matching old 302.187 ms
vs new 131.896 ms; long fixture old 26.387 s vs new 5.953 s. Full normalized maxima and
aligned RE outputs agree at every paired step. Different source numbering makes
raw and shared-input timings separate experiments; do not subtract them to infer
conversion overhead. The workspace passes 121 core, 5 server, 2 wasm-host Rust tests.
New tests cover batch maxima, ordered admissions, large duplicate history and first
derivations, zero counts, and cancellation. Original benchmark kernels are archived
in investigation/baseline and installed only in temporary profiling copies.
The rebuilt wasm/release-server integration checks pass all 18 maximization and
18 RE fixtures, plus all four entry-variant parity checks. The frontend was rebuilt
so web/dist includes the current wasm binary. Native performance figures above
do not establish wasm speedups.

The user clarified that energy means total computational work, not physical joules.
compare_work.py and main.rs resource-new benchmark the uninstrumented production
maximizer on the same frozen inputs with 1, 4 and 10 workers. One worker is fully
serial; production batch policy remains active (512/2048/4096 respectively).
Measurements run sequentially, alternate worker order, check complete maxima,
and use three samples, each averaging 30 matching kernels or one large kernel.
work-results.jsonl retains the raw records. CPU is process user+system time summed
across all threads, with setup/output overhead included; kernel wall excludes it.
One/four/ten-worker medians: matching wall 141.14/64.58/51.00 ms, CPU 141.24/170.63/192.53 ms;
large wall 8.308/6.755/5.808 s, CPU 8.301/16.544/21.458 s. Ten workers cost 36% more CPU for
2.77x matching speedup and 158% more for 1.43x large speedup. Redundant global checks
are 77.01M versus 51.31M in the earlier diagnostic, not all-operation counts.
Large-fixture system CPU rises 0.067 to 5.512 s; allocation/synchronization/scheduling
contributions are not individually attributed. CPU is a work proxy, not an exact
instruction count. No hardware energy/instruction counters are exposed in this VM.
The user has not requested changing the production parallelism policy. Future
choices can target wasted pruning/OS work or expose latency-versus-work preferences;
do not change defaults solely on these two fixtures. New-batch reduction's CPU
effect remains unmeasured even though its comparison count is smaller.

Current benchmark request: compare Basic fixed-point custom saturation on
`A^5 X^2 / B^5 Y^2 / C^5 Z^2` and `AX BYCZ / BY CZ / XYZ^2`, plus allocator trials
on the frozen matching-step-10 and hard-step-2 inputs. Benchmark-only
fixed_point.rs calls old public diagram/default-lattice initialization, expands
concrete original rows, and maps singleton values into the 27-element lattice
of successor-closed sets (including empty). There are 3 active and 15 passive
seed rows. Active algebra reverses inclusion and swaps ordinary union/intersection;
passive uses ordinary inclusion/union/intersection. Both old and new use the same
HashMap join/meet tables and HashSet reachability callbacks, permit every lattice
value, and disable derivation tracking. Time both saturations; preparation and
result serialization are excluded from kernel wall timing. This adds no production
fixed-point feature. Canonical outputs decode IDs to the underlying closed sets
and check full equality, not just counts. The system-allocator pilot produced
102 active and 14 passive rows for all three variants: new serial 371.947 s,
new ten-worker 49.370 s, old worker-limit-ten 33.134 s. Corresponding aggregate CPU
371.919/471.236/327.316s; new parallel uses 27% more CPU for 7.53x elapsed speedup.
The expensive Basic case uses one complete run per setting; matching/hard use
three samples, with matching averaging 30 kernels per process. Raw completed
system Basic records are in investigation/basic-system-results.jsonl.

Allocator correction: the old server selects tikv-jemallocator on Linux and
mimalloc elsewhere; old library code does not select an allocator. All earlier
benchmark tables used the system allocator for BOTH libraries, rather than the
old server's custom allocator. Allocator features are now confined to the temporary
comparison executable so both libraries get the same allocator per process.
run.py --allocator builds distinct re-comparison, re-comparison-jemalloc and
re-comparison-mimalloc binaries. Versions match the old server lock: jemallocator
0.6.1, mimalloc 0.1.52, libmimalloc-sys 0.1.49. Mimalloc trials include the user's
old mi_option_set(26,0) setting even on the Linux benchmark host: the crate defaults to v3, where option 26
is disallow_arena_alloc (zero by default). In v2 it is abandoned_reclaim_on_free.
A separate mimalloc-v2 feature/benchmark tests the historical meaning, also with
26=0. Correct earlier v2-header explanations rather than conflating the two builds. This is not a default-only mimalloc
measurement and not a Mac measurement. compare_allocators.py runs one measured
child at a time, reverses order between samples, checks complete results across
allocators/implementations, and records user/system CPU separately. Result paths:
investigation/{matching-allocator-results,test-all-second-allocator-results,
basic-custom-allocator-results}.jsonl. Production server allocator is not changed
by these benchmark features. Finish collecting results and update the reports
before proposing a production allocator change. The old worker limit 10 still
creates ten generation and ten pruning workers, versus one ten-worker new pool.

Allocator/Basic benchmark completed: reports and 84 measured-process records
cover 36 settings. Basic uses one complete run per setting; ordinary matching
and hard inputs use three samples per setting. Matching samples average 30
kernels. Allocators are system, jemalloc, mimalloc v3 with literal 26=0, and a
separate mimalloc-v2 trial with 26=0 (the historical abandoned-reclaim meaning).
Backend default is v3 in wrapper 0.1.52/sys 0.1.49; v2 is enabled by mimalloc/v2.
Do not repeat the earlier incorrect claim that the default pinned build is v2.

Basic elapsed seconds (new serial / new ten workers / old ten-worker limit):
system 371.947/49.370/33.134; jemalloc 346.233/46.600/28.236;
v3 353.428/46.868/29.446; v2 331.741/43.823/28.070. Every completed sweep checks
full canonical results (102 active and 14 passive rows). With v2, new parallel
is 7.57x faster than new serial for 26% more aggregate CPU, yet takes 1.56x old
elapsed time and 1.51x old CPU. Custom allocation does not close that algebra gap.
Shared lattice/seeding preparation is about 2 ms and excluded from kernel timing.
No full UI/button or post-computation cleanup is included.

Fixed matching-step-ten kernel medians, old parallel / new serial / new parallel:
system 113.96/140.33/50.76ms; jemalloc 106.27/131.08/50.36ms;
v3 114.07/132.76/49.38ms; v2 104.64/130.52/48.33ms.
Hard-step-two kernel seconds in the same order: system 24.683/8.393/5.678;
jemalloc 19.879/5.894/5.064; v3 20.283/5.831/5.268; v2 19.159/5.694/5.010.
New hard-kernel aggregate CPU serial/parallel: jemalloc 5.897/17.510s and
v2 5.696/18.527s. Thus parallel only saves 0.68–0.83s there for roughly 3x CPU
after allocator improvements. System CPU remains 5.5–5.9s in parallel, so do not
attribute all overhead to system malloc. Component causes remain unprofiled.

The complete methodology, backend distinction, raw-record links and reproduction
commands are in investigation/allocator-comparison.md. run.py has optional
benchmark-only allocator features; production crates and workspace lock are
unchanged. investigate.py copies fixed_point.rs into its temporary package;
instrumentation anchors were checked after the driver extension. V2 runs are
subsequent sweeps rather than interleaved with the first three allocators;
single Basic runs cannot establish small cross-allocator differences.

Follow-up complete: the user requested old serial with mimalloc v2 ONLY, reusing
all previous measurements and omitting v3 from the updated report. Do not rerun
new serial/new parallel/old parallel or other allocators. A native old worker
limit of one still has concurrent generation/pruning, so true serial is measured
by compiling the old existing wasm loop natively in a temporary library copy.
run.build(..., old_serial=True) replaces only its two maximize.rs cfg guards;
original source trees are untouched. The binary is re-comparison-mimalloc-v2-old-serial.
compare_allocators.py --old-serial selects only old/1 and the suffixed binary.
Both env worker limits are 1; initialization can use the idle one-worker pool,
but the timed saturation loop is fully serial. This is the old serial algorithm,
including its per-parent pruning; it is not the native pipeline with a smaller pool.
Record three matching samples (each averaging 30 kernels), three hard samples,
one expensive Basic run. Raw files are investigation/{matching,test-all-second,
basic}-old-serial-mimalloc-v2-results.jsonl. Matching median: 333.713 ms wall,
333.874 ms aggregate CPU. Hard fixture median: 145.433 s wall, 145.416 s CPU.
Basic single run: 225.283 s wall, 225.250 s CPU (102 active/14 passive rows).
All requested measurements completed. The allocator report now excludes v3
results and includes old serial, reusing all prior system/jemalloc/v2 rows.
Explain the native-compiled serial branch when presenting comparisons. Serial
repetitions agree fully and row counts match previous runs; previous full outputs
were not retained for a direct comparison against this follow-up.
No other timings were remeasured.

## Current performance investigation (2026-10-13)

The user requested investigating (1) slower Basic fixed-point custom saturation
and (2) excessive parallel CPU for the ordinary large fixture. Completed using
temporary current-code copies, configured mimalloc v2/option 26 = 0 only, Rust
1.89.0 release, native Linux/aarch64, ten logical CPUs. Production code remains
unchanged during the initial investigation; the implementation section below
records the subsequent user-authorized integration. No agents were spawned. New benchmark tools:
benches/re_comparison/investigate_current.py builds phase-only and optional
counter-enabled copies; diagnose_current.py runs sequential controlled variants,
measures summed process CPU, compares complete results, and retains canonical
outputs next to raw records. Linux process-phase CPU uses /proc/self/stat at 100Hz;
getrusage supplies the accurate whole-process resource totals.

Basic's current diagnostic baseline spends ~41s generating and ~2.8s globally
pruning/history out of ~44s. Old checks shared history before local containment;
new checks global history only after generation and otherwise deduplicates locally
per parent pair. Active-side counters: same 255,150 parent pairs; old/new local
containment 548,340/107,535,025, total flow calls 3,988,406/54,555,110. Old rejects
16,304,718 of 16,423,911 constructed candidates early using history. New earlier
immutable history queries reject 20,479,716 of 22,191,147 candidates and reduce
local checks to 7,138,536/flows to 6,519,936. No Hash bound or contended history
is needed for this improvement; same-batch duplicates still remain.

Old good_unions also prunes operand pairs yielding identical joins: choices
1,975,944 -> 1,450,668, complete matrix visits 402,746,175 -> 294,521,022.
Current exchange pruning rejects 380,555,028 complete matrices; prototype search
rejects partial branches once both conflicting cells are occupied. With union
pruning this leaves only 16,423,911 complete matrices (same constructed candidates
and subsequent checks as late exchange filtering). Exhaustive 19,456-case 2x2
margin/compatibility/conflict-mask oracle passes in the temporary core crate.

Basic three-sample phase-only medians (elapsed/CPU): baseline 44.126/420.939s;
earlier history + union pruning + 32-pair tasks 13.635/116.105s; additionally
earlier matrix-conflict rejection 8.382/66.392s. The latter fully serial prototype
takes 43.806/43.796s (one run), versus previous serial new331.741/old225.283s.
These are historical pre-integration values; see the implementation section below. Without union
pruning the early-history/search/chunk prototype takes 9.722/78.192s (one run).
Newly checked old parallel reference 28.435/280.982s; full Basic canonical
active/passive results agree across all variants, beyond 102/14 row counts.

Large fixture: 741,382 tiny generation jobs/messages. Three-sample medians
elapsed/CPU: baseline 5.186/18.926s; 32-pair jobs 1.935/9.300s; 128-pair jobs
1.882/8.866s. System CPU drops ~5.61 ->0.28s. Single phase comparison generation
elapsed3.629 ->0.474s/CPU13.52 ->3.82s; global stage elapsed1.445 ->1.368s/CPU
5.17 ->5.18s. Grouping leaves all operation counts unchanged. This directly
isolates task/channel/scheduling overhead rather than mathematical extra work.

Remaining large global checks: serial51,312,392 vs parallel77,014,442, identical
945,393 candidates/371,816 flows. Cache5.20/5.38M, existing scans36.37/46.32M,
old-row removals9.75/20.07M, parallel peer checks5.25M. Immutable snapshots and
testing old rows against later-dominated peers cause redundant work. Grouped
generation + serial pruning takes3.64/7.65s vs parallel1.93/9.15s in initial
trials. Peer-first pruning reduces checks to62,808,727 (removal5,865,184), but
three-sample medians1.925/8.786s vs grouped baseline1.930/8.971s show no useful
elapsed gain and small CPU improvement. Do not recommend it simply based on
comparison counts; many eliminated comparisons are cheap.

Avoid blanket policies: five matching samples give baseline50.28ms/CPU189.63ms,
fixed32 jobs63.36/155.83ms (slower), combined early-search29.15/104.01ms. Task
granularity must suit heterogeneous workloads. Union grouping helps Basic but
slows grouped large1.85s/9.31CPU ->2.13s/11.56CPU in initial trials. Recommend
early immutable history, early exchange conflicts, adaptive/cheap-job grouping,
and union pruning only where profitable, then normal derivation/cancellation/
custom-algebra/wasm verification before production integration. Reusable early
matrix search belongs in configurations/alignments.rs; dominance proofs and
operand grouping remain in maximize/combination.rs; task dispatch belongs in
parallel.rs or the maximizer's scheduling according to actual reuse.

Full report: benches/re_comparison/investigation/current-performance.md. Raw
current-{basic,large}-{phases,counts,repeat,final-phases,final-counts}.jsonl and
current-matching-{repeat,final-phases}.jsonl accompany it. All runs completed,
counter-enabled times are separate from phase-only timing tables, and no allocator
or SAT backend changes were made. FEATURES.md initially recorded prototype
improvements as pending; the implementation section below supersedes that status.
Do not rerun allocator sweeps to continue this work.

## Integration of the current performance improvements

The user authorized implementation after the investigation. Completed in
configurations/alignments.rs (generic early conflict pruning),
maximize/combination.rs (early immutable global-history lookup and repeated-join
operand pruning), maximize/engine.rs (history borrowing and cumulative progress
counts), and parallel.rs (adaptive native Rayon ranges, bounded result batches
and batched caller-thread progress). No production allocator, solver, public
algebra/model/API schema changes. Wasm remains serial and cancellable.

Union operand pruning is used only when every intersection cell is admissible
and derivations are untracked. This selects dense custom lattices, avoiding the
measured grouping overhead on sparse set fixtures and preserving traced operand
order. The proof uses monotonicity: equal joins consuming smaller operands leave
larger remaining values for intersections. The Operations contract is unchanged.

Native map_with_progress folds adaptive Rayon ranges into result messages. Its
minimum range size scales with item/worker counts, capped at16; maximum128.
Small maps retain item-level ranges, including per-worker global pruning jobs.
Results remain indexed/ordered. Progress callbacks receive increasing cumulative
completed counts once per received batch, never on workers; callers must not
assume one callback per item. The maximizer uses a fixed batch-start count plus
the received count, avoiding reporting batch count instead of pair count.
Cancellation checks remain in computation loops and result collection; errors
cancel siblings and receiver drop unblocks sends. Nested Rayon callers stay serial.

Final release/configured mimalloc-v2 three-sample medians, elapsed/aggregateCPU:
matching fixed step10 26.143ms/119.859ms; large fixed step2 1.976s/9.983s;
Basic both saturations 8.160s/66.281s. Previous production values reused:
48.333ms/180.665ms, 5.010s/18.527s, 43.823s/418.500s respectively.
Initial integrated full old/new comparison at1/10workers also passes canonical
equality, with new serial medians81.649ms,5.800s,41.734s. The final refinements
only affect parallel execution, so these serial values remain applicable.
Old parallel Basic freshly measured27.496s; no allocator sweeps beyondv2.

Tests pass126core+5server+2wasm-adapter. Added permanent exhaustive19456-case
matrix oracle, reversed/self conflict and cancellation checks, both lattice
orders against independent degree-two port choices, reconstructed derivations,
early-history filtering, caller-thread/batched progress and large-map cancellation.
Rebuilt wasm/release server pass all18maximization and18RE fixtures through both
transports, including cache/source promises and zero/nonregular/high degrees.
Frontend typecheck/build passes and web/dist includes the latest wasm.

Historical before-integration kernels are benchmark fixtures under
benches/re_comparison/investigation/baseline/2026-10-13/ (six private modules).
investigate_current.py restores them before instrumentation; investigate.py also
restores historical parallel.rs for its earlier prototypes. Restored diagnostics
prepare/build successfully. compare_allocators.py now has optional implementation
and worker filters for targeted verification. Initial/final/grain integration raw
records are investigation/integrated-*-results.jsonl. README, FEATURES and the
current-performance report record the production status and measurements.
The peer-first pruning experiment remains excluded: its elapsed gain was negligible.

## Basic relative parallel CPU follow-up

The user asked why Basic's new parallel CPU overhead is 59% versus old21%.
Completed targeted current-production diagnostics in temporary copies, mimalloc
v2 only. Production was not edited. New investigate_cpu.py reuses driver/allocator
setup but instruments actual current modules. Flags only override generation
batch size or force serial global pruning. diagnose_current.py adds batch512,
batch4096 variants and sample-start for nonoverlapping repeat labels.

Percentage denominator matters: old CPU serial225.250/parallel272.056s gives
46.806s extra (+20.8%); integrated new serial41.729/parallel66.281s gives24.553s
extra (+58.8%). Absolute additional CPU actually fell48%; serial improved5.40x
in CPU versus parallel4.10x. New before optimizations was331.709/418.500s,
extra86.791s(+26.2%). Do not interpret the percentage rise as rising absolute work.

Measured current generation history lag: default512pair batch for serial vs4096
for ten workers. In phase-only controls, serial512 elapsed/CPU41.360/41.358s,
serial4096 48.987/48.983s. Phase CPU generation33.50 ->41.17s even without
threads. Parallel4096 first sample8.114/65.796s, phases51.41s generation and
14.37s merge/history; serial512 merge7.82s. Holding4096 leaves~10.2s additional
parallel generation CPU beyond the batching effect; runtime/memory mechanisms
are not individually profiled. Global parallel pruning adds~6.6s CPU.

Three-sample medians from alternating parallel policies: default4096
8.124s/65.805CPU vs batch512 7.613s/55.198CPU. Smaller batches save16%CPU,
finish6%sooner, and overhead relative to serial control drops~59% ->33%.
Serial global pruning with parallel generation4096 takes13.497/59.477s;
generationCPU51.31s unchanged, globalCPU8.15s, so slower elapsed despite lessCPU.

Counters serial512 / parallel4096 / parallel512: same255150pairs and16423911
complete matrices/candidates. Early history rejects15636121/15056401/15636121;
local checks2571247/6024264/2571247; local survivors189214/320941/189214;
late global duplicates82925/215792/82925. Global checks5862732/9489532/7981487;
total flows3242974/6004587/3821205. Normal parallel therefore has134% extra local
checks,62% extra global checks,85% extra flow calls. All generation-side counts
match serial after reducing parallel batch to512, proving earlier history visibility
eliminates that algorithmic excess. Frozen global snapshots, separate caches and
peer/removal comparisons account for remaining global excess. Serial-pruning4096
global checks5834805 and totalflows4992624 with unchanged generation counts.

Old21% is not isolated threading overhead: old shared DashSet updates during
generation, native pruning reads a live append-only relation and recent caches;
the natively compiled wasm serial loop does per-first-parent reduction then global
insertion without native caches. Do not invent a measured contribution for those
old pruning differences; this follow-up reused old timings and did not rerun them.

Full canonical outputs agree across all controls/repeats. Report is
benches/re_comparison/investigation/basic-parallel-cpu.md; raw basic-cpu-{phases,
counts}.jsonl plus corresponding canonical .results artifacts. current-performance
and FEATURES link the follow-up. Consider adapting generation batches/history
visibility to duplicates rather than forcing512 globally (larger batches help the
large sparse fixture). Shared mutable history requires contention and derivation
evaluation. The user asked for understanding, so no production policy change was
made. All diagnostics completed; no allocator sweeps or additional feature work.

The follow-up fixed generation-batch sweep tests256/512/1024/2048/4096 on the
actual current production kernels, ten workers, configured mimallocv2, three
samples with reversed middle order and full canonical equality. Elapsed/CPU(s):
large fixture second-step maximum:5.514/19.502,3.555/14.084,2.580/11.298,
2.113/10.053,1.974/9.941. Thus256 is2.79x as long and96%moreCPU than4096;
systemCPU rises0.769->7.023s, generation0.651->2.964s and merge1.239->2.455s.
Basic(both saturations):8.454/55.608,7.790/55.713,7.511/57.113,7.647/60.287,
8.141/65.877. Basic256 has essentially sameCPU as512 but8.5%slower;
1024fastest among tested fixed settings. The old2.80s large512 trial was a
historical fixed-grain prototype, not today's adaptive-grain production.
Raw generation-batches-{large,basic}.jsonl and complete .results artifacts;
tables/reproduction in basic-parallel-cpu.md. No production batch change.

User then requested a strategy suitable for both Basic and the large fixture.
Completed adaptive prototypes using investigate_adaptive.py (temporary production
copies), same10workers/mimallocv2, full canonical equality. Recommended measured
policy:10ms target generation time, start4096 perdegree, adjust nextpair batch
by measured rate, boundchanges2x andsize512..4096; learn fromfullbatches AND
expensive partialroundtails(>=10ms), ignoringcheap tails. Initial fullbatch-only
feedback learnedtoolate and BasicCPU~59s; includingpartial evidence gets~56s.
Do not use fixture-specific names/algebra checks. Production was unchanged
during this strategy investigation; application serial/wasm scheduling stays
as before. For futureintegration deriveuppercapfromworkers andkeepserial/wasm
policy, preserveorderedhistory, progress,cancellation,andoptionalderivations.

Three-sample medians on sameprototypebinary, middleorderreversed:
Basic baseline8.129s/65.931CPU vs recommended7.565s/55.892CPU:6.9%faster,
15.2%lessCPU, overhead35%vs priorserial41.358CPU (previousdefault~59%).
Large2.007s/10.056CPU vs2.000s/10.060CPU. Matching30kernels/process:
26.400ms/120.023msCPU vs26.503ms/120.167msCPU. Bothlatterwithin0.5%.
5ms partialfeedback Basic7.725/56.096; notbetter than10ms. Shrink-onlywithin
degree rule andseparatinghistoryupdatesfrombufferedglobalpruning didnotbeat
thebidirectional10ms policy. Serialglobalpruningwasalreadyknownslowerelapsed.
PoliciesstillleavefrozenparallelpruningextraCPU; no universaloptimalityclaimed
andnoadaptive operationcounterruns. Rawadaptive-partial-{basic,large,matching}
.jsonl+canonical.results; adaptive-batches/sticky-{basic,large} retain earlier
variants. investigate_adaptive.py supportstime,partial,sticky,buffered andoptional
batchtrace flags onlyintemporarycopies. diagnose_current.py --iterations30
meansordinarykernelsaveragedperprocess, CPUamortizedperkernel; Basicrequires1.
Detailedreport adaptive-batches.md, linkedfrombasic-parallel-cpu/current-performance.
Alljobsfinished; noadditionalallocator sweep, featurework, commit, orpush.

## Adaptive batching integration

The user subsequently authorized integrating the selected adaptive rule and
requested an old/new comparison including total CPU overhead. They explicitly
said to reuse existing measurements; do not rerun timings for this integration.
The earlier prototype-only status is superseded by this section.

Implemented in maximize/engine.rs: caller-local Cell batch size resets for each
degree and persists across that degree's closure rounds. Native untraced calls
outside the Rayon pool time generation, target 10 ms, learn from full batches
and partial tails lasting at least 10 ms, and bound each adjustment by 2x.
Sizes stay between 512 and worker_count*512 clamped to 512..4096. Serial, nested
Rayon, wasm and observed derivations keep their old fixed batches. Timing cannot
change trace admission order. Shared Rayon dispatch, ordered history/pruning,
generic algebra, empties, progress and cancellation remain as before. The small
private rate helper belongs in the closure engine; no new production module or
public configuration was needed.

All 128 core + 5 server + 2 wasm-adapter Rust tests pass. Added rate-bound,
zero-duration and expensive-partial-tail tests. Rebuilt release server and wasm
pass 18 maximization and 18 RE fixtures through both transports. npm run build
passes typechecking and includes the rebuilt wasm in web/dist. Temporary server
on 18083 was stopped. No new timing runs, allocator/solver changes, commits or
pushes. Use explicit +1.89.0/RUSTUP_TOOLCHAIN=1.89.0 in this container because its
default stable toolchain currently lacks rustc/rustfmt.

Reused old parallel / new adaptive elapsed and CPU respectively:
Basic 27.496s/272.056s vs 7.565s/55.892s;
large second-step maximum 19.636s/191.012s vs 2.000s/10.060s;
matching step-ten maximum 103.219ms/439.834ms vs 26.503ms/120.167ms.
For overhead, use the existing uninstrumented integrated new serial medians
(CPU 41.728659s, 5.802504s, 81.750833ms), not the diagnostic Basic serial control.
Old serial CPU is 225.250191s, 145.416461s, 333.873633ms respectively. Extra CPU:
Basic old 46.805941s(+20.8%) vs new 14.163262s(+33.9%);
large old 45.595111s(+31.4%) vs new 4.257902s(+73.4%);
matching old 105.960433ms(+31.7%) vs new 38.416067ms(+47.0%). Absolute additional
CPU is lower in all three new cases; percentages rise because new serial is much
faster. Old serial is the old wasm loop compiled natively; native old generation
and pruning differ, so percentages do not isolate threading overhead. Old limit
10 creates 10 generation + 10 pruning threads on the ten-CPU VM; new has one
ten-worker Rayon pool. Basic old serial has one sample; other values are medians
of three samples. These are fixed maximization kernels, not full RE sequences.

Documentation/status updated in README, FEATURES, benches/README and the
basic-parallel-cpu/current-performance/adaptive-batches reports. To keep historical
experiments reproducible, froze the pre-adaptive engine as benchmark fixture
baseline/2026-10-13/fixed-batch-engine.rs. investigate_cpu.py --fixed-batches
selects it; without that flag the script instruments current production and an
explicit batch override disables adaptation. investigate_adaptive.py always uses
the fixed baseline before applying its experimental timing/sticky/buffered flags.
Both preparation paths were verified to succeed; do not claim new timing samples.

The user selected keeping maximization as a separate reusable computed object
associated with the original problem snapshot. The UI need not display a new
maximized problem or change its output names. Speedup, diagrams, and solvability
checks can consume that object. Keep it separate from Problem's mathematical
definition, and recompute it for transformed snapshots unless preservation is
proved. Only complete results provide a maximization guarantee.

The proposed input-aware algorithm avoids enumerating complete input configurations:
construct the condensed joint relation of valid input-output pairs using the
input promise, output constraint, and mapping, then apply ordinary pairwise
union/intersection maximization over those pairs. Existing product_with performs
the condensed join. Joint maxima compactly cover all fixed-input maximal output
families, but individual fixed-input projections can contain redundant families.
Use ordinary output maximization for unrestricted mappings. Distinguish the
joint maximization guarantee from output-only maximization when consuming it.
Retain the original input promise; projecting valid joint configurations could
remove admissible inputs with no legal outputs and conceal unsolvability.
Fast input-output arrows on a complete joint maximum use occurrence containment
between pairs with the same input; input-aware output arrows aggregate those
checks and mapping permissions. Implemented Apply RE conversion: for each joint part
R, extract its nonempty fibers R(i) = {o | (i,o) in R}. Intern distinct output
sets as new labels; replace R by alternatives among those fiber labels, keeping
its multiplicity, to obtain the new active constraint. A new set-label S is
allowed at input i iff S is a subset of the old mapping entry g(i). Obtain the
new passive constraint from old active parts O by replacing O with all new
set-labels S intersecting O. Swap active/passive graph degrees and input
constraints to follow the old Speedup role convention; retain the original
input promise. The joint projection onto output sets is universally valid, and
every fixed-input valid family is covered by a compatible fiber selection of
some joint maximum. The equivalence argument targets the unpruned input-aware
RE definition (GRB22, Definition 3.1), restricted to the retained input promise;
the compact result can retain redundant per-input configurations. It is not
literal agreement with Definition 6.2 of arXiv:2510.17639 as printed: that
definition maximizes outputs globally before applying the subset mapping rule.
Example: passive outputs AC / BC have global maximum AB C. If input i permits
only A, the printed definition's alphabet {AB,C} permits no new label at i,
whereas the joint-fiber construction retains {A}. Do not claim these two
definitions are generally equivalent or treat the distinction as mere renaming.
The user clarified that the required direction is a zero-round mapping from
the printed paper RE problem to our compact input-aware result. This direction
does hold under the retained input promise: at each universal-side node, attach
its actual inputs i_k to its source set-labels S_k. This is a valid joint family,
dominated by a joint maximum M after alignment. Emit fibers T_k = M_k(i_k),
which contain S_k and are subsets of g(i_k). Their IDs occur in the proposed
fiber-converted universal constraint; enlarging sets preserves the existential
witnesses at the other side. The choice uses only the node's local inputs and
source outputs. For globally maximal source configurations from Definition 6.2,
T_k = S_k: a strict enlargement would contradict global output maximality.
Thus on valid source instances the mapping simply relabels the same set-labels.
The earlier counterexample blocks the reverse direction, not this required one.
The user agreed to use the unpruned GRB22 definition as the reference and justify
the joint-maximization conversion by an explicit zero-round equivalence proof
before implementation. The globally maximal Definition 6.2 does not establish
that equivalence. No claim is made that the literature lacks an input-aware
maximality definition; the discussion's search did not find one.
A temporary exhaustive oracle checked soundness and coverage in 441
two-input/two-output degree-two cases; permanent maximization tests now check
all condensed candidate families in these cases. Permanent Apply RE tests now
verify both local zero-round mappings against unpruned input-aware RE in 1,024
small cases, including empty mappings and impossible input/output languages.
Custom operations, empty-set policies,
optional derivation tracking, cancellation, and native parallelism must remain
possible. Discussed optimizations still need correctness checks and benchmarks.

## Apply RE

The user requested Basic FP (temporary) and restoration of the old determinate
pair progress, then explicitly prioritized finishing those BEFORE investigating
CPU overhead. Both are now implemented and verified. BasicFixedPoint request
uses fixed_point.rs plus fixed_point/lattice.rs: default partial output diagram,
transitive principal closures, all successor-closed subsets including empty,
dual active / ordinary passive counted closure, passive downward expansion, then
the old cleanup. Generic reachability is in graphs.rs; output-independent cleanup
(strict weak active choices, dominated rows, iterative removal of one-side-only
labels and compact ID remapping) is in simplification.rs. It retains graph class
and independent input promise. Input-dependent mappings have no defined Basic
extension: API rejects them and the button is hidden. No other FP variants added.
Returned problem contains actual transformed labels/constraints, no FP metadata.

Renamed RoundEliminationActions.vue to ProblemTransformations.vue, sharing request
lifetime, Cancel, busy gating and appended-result handling for Apply RE and Basic
FP (temporary). Closing source cancels; folding retains computation. Results use
ordinary session persistence. The source maximum cache is preserved and is never
associated with the newly generated problem. README/FEATURES updated.

Progress has optional WorkProgress { completed:u64,total:u64,pass:u32 }, generated
as JS numbers for presentation, absent when work is unknown. Generic report_work
and report_progress retain cancellation before/after callbacks. Engine counts
all actually queued unordered parent pairs incl self, excluding previous/previous
pairs; totals are independent of adaptive batch boundaries. It emits 0..total per
closure pass, then starts the next at0. UI progress value is completed/total,
displays pass/percentage/pair counts, and remains indeterminate for SAT. FP remaps
closure phases to ClosingActive/ClosingPassive, preserving the work snapshot;
Cleaning covers the final cleanup. Existing 100ms progress coalescing unchanged.

All134core+5server+2wasm-adapter tests pass, with full-pass counter tests,
finite-lattice/dual tests, cleanup and cancellation API tests. Rebuilt wasm and
release server pass6 Basic FP fixtures (including the A^5X^2 benchmark) and18
Apply RE fixtures through both transports. npm run build passes and web/dist
includes the button and new wasm. Added npm run test:fixed-point. The test server
on18083 stopped. CPU investigation is now resumed; do not forget it.

CPU goal: keep the fixed large fixture maximum <=2s while reducing relative
parallel CPU overhead to20-30% against the SAME updated serial implementation.
Before the priority correction, prepared a temporary packed-set prototype
investigate_packed.py/packed_sets.rs with scalaru64 sets for <=64 distinct labels,
generic fallback for larger alphabets, and pruning worker-count controls. Uses
the SAME generic closure and petgraph matcher. /tmp/re3-packed-phases binary was
built before progress/FP edits; packed-large-{10,1}.jsonl stores initial trials
with complete canonical equality. No production packed representation changes
have been made. Do not benchmark while building, testing or running other CPU
jobs. Use configured mimallocv2 only; no allocator sweep. First inspect the
initial records and then repeat suitable policies. User asked to investigate,
so do not silently replace the model or commit an unverified performance policy.

CPU investigation now COMPLETE; performance code remains prototype-only. Found
a regime satisfying both requested limits on the fixed large fixture: pack its
58-label sets asu64, use256-item generation ranges for indexed maps>=512,
and serial ordered global pruning while generation stays parallel. Same generic
closure/petgraph matching; >64 labels falls back to existing representation.
Three-sample medians with reversed middle order: selected1.910269s/4.289977CPU;
same packed representation fullyserial3.318950s/3.321364CPU. Extra0.968613CPU,
+29.163%. Individual selected runs1.909..1.917s and4.287..4.303CPU; full canonical
maxima agree with the earlier reference. Grain128 serialpruning1.929710/4.411812
(+32.831%CPU), packed ordinaryparallelpruning1.199448/5.946442(+79.036%).
Do NOT compute packed overhead against the old vector serial5.802504s denominator.
Previous production2.000s/10.060CPU had+73.4%; selected saves57.4%totalCPU.

Initial pruning-chunk variants2/4/6 didnotreach30%; packed serialpruning without
coarsegeneration2.102s/4.892CPU missedelapsed andCPUtargets. Raw
packed-efficient-large-{10,1}.jsonl and .results retain the repeated comparison;
packed-large and packed-grain-large records retain single exploratory trials.
Reportpacked-cpu.md links records/reproduction. investigate_packed.py builds
temporary current copies and adds only packed_sets.rs +envscheduling overrides.
No production packing/scheduling change, allocator sweep, SAT replacement, commit
orpush. Stop optional benchmarking: requested fixture target is now established.
Before future integration, choose a workload policy (coarse/serialpruning canhurt
Basic and matching) and test those cases. Feature/progress production work is
complete and verified; no outstanding user task after documenting this result.

Final polish: Basic set labels use compact concatenated names for ASCII
single-character alphabets (e.g. AX), matching the old display; other alphabets
use quoted set names to prevent collisions. Rebuilt server/wasm and reran all
134core+5server+2wasm-adapter tests and6 Basic FP transport fixtures successfully.
Frontend typecheck/build passes and web/dist includes the final wasm. All timing
jobs completed; test server18083 stopped. Generated TS whitespace cleaned after
export tests. git diff --check and cargo +1.89.0 fmt --all --check pass.

Final RE progress extension: conversion also publishes a known part total, so
cached Apply RE is determinate even when no maximization runs. UI displays the
supplied work-unit label rather than hardcoding pairs. A discarded configuration
resolves its unvisited parts; final conversion explicitly reports total/total.
Added a regression for impossible first-part substitution skipping the rest.
Final tests135core+5server+2wasm-adapter pass. Rebuilt both adapters and reran18
RE transport fixtures successfully;6 FP fixtures had already passed with the
same final FP implementation. Frontend rebuilt with the final wasm. CPU study
is still prototype-only and no extra timing runs were made during progress edits.

The user authorized the Apply RE button (old Speedup). Its Rust implementation is
algorithms/round_elimination.rs, with adjacent round_elimination/tests.rs. Keep
the general counted part-substitution helper in configurations/substitution.rs;
it is shared by universal conversion and existential substitution. The module
documents both directions of zero-round equivalence to unpruned input-aware RE.
Build only nonempty output sets actually used by the maximum, intern equal sets,
retain counted exponents, use subset permissions, and swap both graph-class and
input-constraint roles. Preserve input names and the original promise, even when
the output alphabet is empty. Fresh output names are compact rather than nested
source sets. Do not carry a source maximum into a transformed problem's caches.

Request::ApplyRe accepts the source Problem and an optional trusted complete
MaximizationResult of that exact snapshot. A missing cache is computed once and
streamed as PartialResult(Response::Maximization); the final Response::Problem
is the new definition. This partial event is self-contained and is safe under
existing progress coalescing. A complete maximum can survive cancellation of the
transformation; an incomplete maximum cannot. Both adapters use normal dispatch.
Phase::Transforming represents the conversion, separate from Maximizing.

ProblemTransformations.vue shows Apply RE in each problem header, progress and
Cancel while running, and retryable errors/cancellations. Folding keeps it alive;
closing/importing cancels it. ProblemCard coordinates it with the separate
Maximize action to avoid duplicate simultaneous source maximizations. Source
maxima streamed by RE update the Maximize section as well as the saved snapshot.
App appends a fresh current problem card without modifying or closing its source.
Every new card has empty analysis caches and can be transformed repeatedly.
Session files and links use the existing independent card representation.
web/scripts/test-round-elimination.mjs checks every preset through both transports,
cached/uncached equality, original promises, nonregular/zero degrees, high counts,
and repeated RE; npm run test:re invokes it.

## Diagram decisions

The user selected **Output-only**, **Input-output pairs**, and **Input-aware outputs**.
Use matching names throughout the implementation: `DiagramKind::{OutputOnly,
InputOutput, InputAware}` and protocol values `output_only`, `input_output`,
`input_aware`. Older spellings belong only in saved-session migration and its tests.
Hide inapplicable or redundant choices. For unrestricted mappings, hide pair
diagrams if every input label can occur at every relevant passive degree, and
hide input-aware outputs if every relevant passive degree has an input configuration.
Relevant degrees are positive allowed graph-class degrees with output configurations.
Trivial unrestricted input has only the output-only controls. Reuse earlier
equivalent cached diagrams: project one input copy and remap its existing node,
SCC and edge indices, or reuse the input-aware graph directly; no new computation.
Do not hide meaningful degree-specific distinctions on nonregular graphs.
Keep input-related diagrams when even a constant input restricts the outputs.
Hide graph edge and distance-two coloring unless every passive degree is 2.
Restore unsupported saved selections to an applicable choice before displaying
their controls or results.

The user approved all diagram methods and three types. Output-only arrows
preserve one-occurrence replacement in the passive output constraint. The fast
occurrence test is sound but partial for arbitrary constraints and exact under
a complete maximization guarantee; both options execute the same code. Exact
output checking compares residual context languages, with cached unions of
degree-one contexts for passive degree 2. Computed maxima are separate artifacts;
these diagram methods do not yet automatically consume them.
Cheap input-output arrows lift known output arrows through mapping permissions;
exact arrows also consider the passive input promise. Vertices are admitted
`InputOutputLabel { input, output }` pairs; arrows always keep the input fixed.
Input-aware output arrows hold for every input admitting the source; forbidden
targets block them. A source admitted by no input has vacuous input-aware arrows.
An admitted pair with no passive context can replace to any admitted target
at the same input. Use existing local-promise semantics, without global realizability.
No original paired-entry metadata or parser pair types are needed.

`algorithms/diagram.rs` owns schema/orchestration, `diagram/relation.rs` owns
occurrence/context comparisons, and adjacent tests use exhaustive joint labels
as an independent oracle. Keep generic context coverage and compatible multiset
products in `algorithms/configurations/`, generic SCC/reduction in `graphs.rs`,
and original input preparation in `inputs.rs`. Lazy counted multiset visitation
is shared with solvability's provenance-aware passive counterexample search.
Native comparisons use Rayon's shared bounded pool, avoiding per-request pools;
cheap operations stay serial to avoid measured scheduling overhead; general exact
coverage and large occurrence workloads use workers. Progress stays on the calling
thread. Wasm remains serial in its request worker.
The user requested using more threads for diagrams, not changing SAT backends.

The user explicitly requested SCC grouping, a visualization library, and diagram
export buttons. Retain full relations plus SCC members and reduced component
edges. `DiagramAnalysis.vue` controls methods, cancellation and saved state;
`DiagramView.vue` lazily loads Cytoscape/Dagre for the selected diagram, three
layouts, selection and text/PNG exports. Physics is the default for new diagrams;
preserve explicit saved layout choices. D3-force in diagramPhysics.ts provides
local repulsion, collision avoidance, and edge springs. Do not add gravity or
component packing: distant unconnected nodes must stay where they are dragged.
Cytoscape handles rendering; Dagre handles hierarchy. Stop physics while dragging
and settle after release without recentering or changing zoom. Free stops
automatic layout and
preserves current positions; fresh free diagrams and its Reset layout use a
compact initial grid. Stop animations before dragging, changing modes, or
destroying the view. Persist Free like the other layout choices.
The user requires choosing the diagram
type before computing, with an explicit button for new choices. Compute and
return only that relation: output requests skip inputs; exact input-output
requests skip output-only comparisons; cheap input-output requests need output-only
arrows, and input-aware requests need pair arrows, but reduce only the selected graph.
Cache completed results per problem and relevant type/method choices, reuse
them when reselected, and persist the cache in sessions. Ignore unused options
in cache keys. Do not cache cancellations or errors. Collapse must preserve running work,
and hidden canvases must resize correctly when expanded. Persist diagram
settings/results in version-one sessions; older sessions get default diagram
state. Partial results must describe missing arrows as unknown. Diagrams concern
passive validity, not full active/passive equivalence for transformations.

## UI history and sessions

Cache completed coloring results per problem, coloring kind, and fixed palette
size or maximum search. Maximum keys ignore the hidden fixed size. Reuse cached
choices immediately and persist all entries in files/links; migrate older singular
results. Cancelled and failed checks remain retryable. Put bulk collapse/close
controls in the top toolbar immediately before Import. Degree table headings
read "Deg." with full-name tooltips and accessible names.

The user prefers a compact, fluid layout like the old GUI, with a 16px base font.
Keep the page white,
New problem and numbered problem cards gray and full width, and their sections
white. Use pale gray headers and blue clickable titles, with disclosure arrows;
no separate per-card/per-section Collapse or Expand buttons. The native title
button supports Enter/Space; clicking the rest of a header also folds it, except
for independent controls (Start, selectors, Close, etc.). Retain bulk collapse
buttons. Reuse CollapsibleSection for problem cards, sections and editor groups.
Number problems for identification when folded; the corner × closes a card.
Show active/passive degree sets as compact problem-header metadata, visible even
when folded. Graph class must not occupy its own panel in the wrapping flow.
Its former collapse flag belongs only in session migration, where it is discarded.
White sections hug their contents and share one wrapping flow, without growing
to fill grid columns or matching another section's height. Constraints and analyses
must share rows when space allows; do not put each component group on its own row.
Within each problem, expanded sections share the available width beside a compact
vertical stack of collapsed section headers on the right. On narrow screens the
stack follows below. Expanding restores the section's original position. Use
CollapsibleSection's optional Teleport target to move existing DOM without
unmounting bodies, analyses, or diagrams; preserve keyboard focus when moving.
Nested Active/Passive folds remain inside their constraint section. Hide the
expanded flow when all sections are collapsed. Keep the wrapping problem-content
container at the full width available to it, with individual panels content-sized;
shrink-wrapping it makes collapsing sections incorrectly push other panels down.
The diagram starts compact: clamp(16rem, 19vw, 20rem), about 256–320 px at
normal font size, to fit typical laptop layouts. Coloring uses a 21.5rem
preferred width so its title and longest selector remain readable; do not use
min-content width for that panel. Diagrams do not shrink to fit beside a panel; they wrap
when needed and clamp on narrow screens. Degree override fields need their own
18rem preferred label width; do not inherit the min-content textarea label width.
Textareas and diagrams resize in both
axes, and their white panels follow their width; gray outer cards stay full width.
Clamp resized content to the screen width, and automatically refit diagram graphs.
Controls inside the diagram wrap when narrowed. Keep editor actions visible when
its body is folded; Start must still append. Examples, graph-class controls,
notation/sharing help, and analysis explanations remain expandable. Label counts
belong in section headers. Editor/help folding is local UI state; existing
problem/section collapse and analysis persistence use the session model.
Folding must retain editor contents, cached results, and running computations.

The native server serves `web/dist`. After frontend edits, run `npm run build`
in `web` so the served UI includes them; typechecking alone does not rebuild it.

The user requested multiple independently closable problems, whole-card collapse,
per-section collapse, and bulk Collapse all / Expand all / Collapse all except
current controls. Current is the most recently created or interacted-with card,
visibly marked; after import it is the last card. Keep stable runtime keys so
adding/removing cards preserves other cards' settings and analyses. Collapsing
uses visibility rather than unmounting and must not cancel computations; closing
unmounts the card and aborts its analyses. Close all removes every open card and
cancels its analyses, while retaining the editor. Start errors keep previous problems.

`web/src/session.ts` owns browser persistence using generated Rust types for
problems, results and bounds; do not create parallel problem schemas. Version 1
files have `{ format: 'round-eliminator-3', version: 1, session: { editor, problems } }`.
Save the current editor, canonical problem snapshots, whole-card and section
collapse flags, coloring settings, completed analysis results and proved bounds.
Do not persist workers, controllers, progress or running state. Clear bounds'
`testing` on export without mutating live data. Imported partial bounds offer a
retry, and imports replace the session and cancel its old work. Old GUI files
and links are not compatible. `SessionControls.vue` handles import/export and
clipboard status; links use browser gzip streams and base64url in `#re3=...` and
are decoded locally on load/hash changes. Provide a manual selectable link if
clipboard permission is unavailable. `ProblemCard.vue` connects analysis snapshots
and close/collapse actions; `CollapsibleSection.vue` supplies accessible shared
section headers with visibility-based bodies. `npm run test:session` checks codec
round trips, Unicode, versions and saved proof behavior.

Progress and cancellation use shared Rust-defined computation events. The native
UI uses an SSE POST stream; dropping it sets a persistent cancellation flag,
including when queued. CPU permits stay held until work exits. CaDiCaL polls the
flag through RustSAT's terminator; BatSat uses its stop callback. Expensive
parsing/enumeration/encoding/verification loops check it too.
Petgraph flow checks cancellation during network construction and before/after
each call, but the library has no cancellation hook inside a single native flow.
Wasm sends progress from a dedicated worker and cancels by worker termination.
Keep cancellation distinct from unsolvability. Show phases and SAT candidate
counts with indeterminate progress, never an invented completion percentage.
Child processes were considered and accepted as a fallback, but are unnecessary
with the cancellable backend. No solver performance comparison has been made.

## Live parallel pruning integrated (2026-10-13)

The user challenged the proposed universal use of serial global pruning: their
past workloads sometimes spent most time in pruning. They then authorized trying
the old live-sharing approach and combining useful optimizations. The earlier
packed/serial-pruning prototype-only recommendations are superseded here.

Production ordinary maximization now uses maximize/packed.rs for alphabets of
at most64 distinct labels, including temporary input-output joint labels. Dense
u64 masks replace working vector sets for union/intersection/subset; decoded
maxima keep the public model and canonical ordering. Larger alphabets fall back
without a limit. Custom algebras, empty lattice values, and observed derivations
keep the common generic engine. Packed large maps use a256-item minimum Rayon
range, custom algebras normal splitting. Bounded adaptive generation batches,
immutable duplicate history, and counted matching are retained.

maximize/antichain/live.rs is the private native concurrent implementation.
Workers dynamically take8 candidates, keep recent dominators, refresh immutable
Arc entry handles, scan newest first, and skip atomic removal flags. A short
RwLock protects appends/handle refresh only; matching never holds it. Admission
validates all entries appended since the scan before publishing, preventing
concurrent duplicate admissions. Newly accepted configurations and removals
immediately inform peers. Removed entries stay allocated until batch completion;
their cached configurations remain sound by transitivity. Compact survivors on
the coordinator. No unsafe code, new dependency, or extra worker pool was added.
Generation/pruning still alternate by batch; the old two-pool pipeline was not
copied. Tracing keeps ordered rejection/insertion; wasm and small batches stay
serial. Final outputs are sorted normally.

Pruning concurrency estimates candidate cost from summed worker elapsed time in
the preceding batch, targeting8ms/worker and clamping to2..shared-pool-size.
This is a scheduling heuristic, not CPU measurement or a universal overhead
guarantee. Earlier20/40ms targets underutilized Basic, and5ms gave30.8% CPU
overhead in final uninstrumented large-fixture runs.8ms met the requested budget.

Final UNINSTRUMENTED release/mimallocv2 option26=0, ten logical CPUs:
- Large fixed second maximum: serial3.297691s /3.300042CPU; parallel1.226442s
  /4.276880CPU, +29.6008% CPU. Three alternating samples each, full canonical
  equality. Meets <=2s and <=30% extraCPU. Previous vector parallel2.000/10.060s.
- Matching fixed step10: serial35.2963ms /35.404533CPUms; parallel16.690337ms
  /54.720967CPUms, +54.6%CPU. Three samples,30 kernels/process. AbsoluteCPU
  falls by >half versus previous120.1669ms, but relative overhead remains high;
  future tiny packed-generation scheduling work should acknowledge that.
- Basic BOTH custom saturations: parallel median7.211s /51.534CPU over3samples;
  newly measured serial41.356s /41.353CPU (one costly sample), +24.6%CPU.
  Previous production7.565/55.892s. Fixed two-worker pruning hurt Basic
  (8.76s), whereas all10 live-pruning workers could do6.37s with moreCPU.
- Old timings were reused, not rerun. No allocator sweep or v3 run. TotalCPU
  is work proxy, not hardware energy. Never benchmark beside builds/tests.

Report: benches/re_comparison/investigation/live-pruning.md. Final records:
live-integrated8-work.jsonl and live-integrated8-basic-{10,1}.jsonl. Intermediate
live-integrated-work.jsonl used the5ms policy. EXCLUDE live-large-10.jsonl timings:
the user reported concurrent heavy computation during those initial trials.
They confirmed it finished before every live-quiet, live-adaptive and final run.
Full-output comparisons from those initial trials are still valid.

Historical pre-live maximize.rs, engine.rs, antichain.rs and parallel.rs are frozen
in investigation/baseline/2026-10-13/pre-live. investigate_cpu.py restores them;
investigate_packed.py inserts benchmark-only adapters/flags afterward, preserving
earlier experiments despite integration. Production has no profile environment
flags. Python caches are ignored. README/FEATURES and the benchmark reports now
reflect integration rather than the old serial-pruning recommendation.

Verification:141 core +5server +2wasm-adapter tests =148 pass. Added random
counted multi-part pruning/serial oracle comparisons, removed-dominator cache
checks, cancellation during matching, and a barrier forcing concurrent admission
against the same initial list. Packed tests cover all64bits, noncontiguous IDs,
u32::MAX counts, zero degree and65-label fallback. Existing exhaustive universal
and custom-algebra tests remain. Rebuilt native server and wasm.18maximization,
18RE and6BasicFP fixtures agree through BOTH transports; frontend typecheck/build,
Rustfmt and git diff --check pass. No commits or pushes. Benchmarking is complete;
do not continue optional sweeps without a new request or concrete remaining risk.

## Implementation review completed (2026-10-13)

The user asked for current-vs-original speedups without remeasurement, then an
implementation review for reusable ideas and other improvements. Reused matched
native-parallel/mimallocv2 numbers: Basic27.496→7.211s =3.81x elapsed,5.28xCPU;
large fixed second max19.636→1.226s =16.01x elapsed,44.66xCPU; matching fixed
step10 max103.219→16.690ms =6.18x elapsed,8.04xCPU. These are kernel comparisons,
NOT final whole-RE sequence/UI speedups. No new timing runs or source changes.

REVIEW.md is the portable root audit record. Reviewed current model/parser,
maximization, configurations, RE/Basic/simplification, diagrams, solvability/SAT,
transport lifetime/coalescing, UI/cache/session organization and relevant old
counterparts. Broader unported old proof modules were inspected for ideas, not
claimed completely correctness-audited. No demonstrated correctness bug found.
FEATURES.md records the proposals, all still future work.

First candidates: allocation-free LabelSet overlap tests in allows_positions;
reject parent pairs with no useful unions before intersection/conflict preparation
(old good_unions did this first); exact generated-duplicate sharing within bounded
batches, possibly retaining covered local rejections with an explicit memory cost;
consume ALREADY cached maxima in diagrams/solvability without automatically doing
unrequested analysis; compare membership-based zero-round SAT/old degree-two
behavior searches, recovering configuration/port witnesses only once.
Other findings: visit_multisets retains every visited row even for one-part or
disjoint-domain families; Basic precomputes K² tables on an exponential lattice;
matching/alignment buffers/conflict tables repeat work;64-label packing boundary
needs wider/density-dependent candidates; diagrams can group profiles/defer
contexts/compress transitive graphs; unbounded coloring verification needlessly
repeats dummy colors; reuse zero-round and palette proofs across related queries;
consider lazy active obligations, raw UI snapshots/controller composable and
mixed-request CPU budgets. Production SERVER still uses system allocator, unlike
the mimalloc-v2 comparison executable; allocator choice belongs to executable,
not core/wasm, and should use platform evidence. Keep linear palette search.

Critical invariant noted in the review: counted-flow domination is uniform
whole-part matching, not complete semantic language containment. Example target
AB AC BC admits every concrete choice of family A BC² (ABB,ABC,ACC), but cannot
uniformly match its two BC occurrences into the single BC target part. A tiny
independent Python product/permutation check confirmed this example; it was not
a benchmark. Current exact coverage fallback handles it correctly. Do NOT prune
maxima simply by semantic language coverage: every admissible condensed rectangle
must still have a uniform dominating maximal representative for Maximized mode
and Apply RE's portwise enlargement argument. Add degree-three regression cases
when future pruning changes are implemented. Preserve current input promises,
separate output-only/joint guarantees, trace order, and generic algebra contract.

No library/framework/model rewrite is recommended from this static review.
No new features were implemented; discuss/prototype selected ideas only when
requested, then benchmark their actual impact and total CPU. Existing148 tests
were not rerun for documentation-only edits. No commits/pushes.

## Next performance task (2026-10-13)

User requested persistence of the full review and specifically the observation
that incidence edge coloring solvability on bipartite maximal matching is fast,
but checking after one RE step takes far too long. This is deferred; do not
investigate coloring in the current task. REVIEW.md and FEATURES.md retain it.
Current authorization: try only the review's maximization improvements, compare
complete canonical maxima, elapsed time and total CPU, and integrate useful
changes. Preserve generic algebra, uniform domination, trace determinism, wasm
and cancellation. Other review proposals remain future work. Benchmark without
concurrent builds/tests/heavy computation, mimalloc v2 only.

## Maximization review experiments integrated (2026-10-13)

Completed the user's maximization-only request. REVIEW.md retains the full static
review and the deferred incidence edge coloring slowdown after one RE step on
bipartite maximal matching. Do NOT silently turn that bookmark into current work.
Detailed experiments/results: crates/core/benches/re_comparison/investigation/
review-improvements.md. Other review findings remain future work in FEATURES.md.

Integrated useful-join discovery/empty-pair rejection before preparation; reusable
AlignmentSearch buffers/conflict indices; necessary AlignmentComponents balance
before exchange preparation (petgraph UnionFind, exact enumeration still follows);
stack u128 compatibility caching for <=128 flow cells (larger graphs unchanged);
exact batch-generated history plus covered local rejections for untraced degrees
certified dense by a common admissible meet; vector subset cardinality rejection;
u64 <=64 labels, u128 65..128, unlimited vector fallback above128. Generic public
Operations/Ord bounds, model/wire types, input promises and trace ordering stay
unchanged. No new production file/dependency/layer. Generic helpers remain in
configurations/alignments.rs and matching.rs, policies in maximize modules.

Broad generated-history sharing and a per-pair dense gate regressed the large
fixture; rejected. Common-meet selection preserved Basic gains while leaving
sparse history unchanged. More retained keys cost memory (~4 MiB extra on the
Basic comparison); monitor other algebras. Do not replace uniform domination
with semantic language inclusion. Added degree3 AB AC BC / A BC² regression.

Final pruning target is now12ms/worker, replacing8ms. With the faster serial
kernel,8ms left the large fixture at32.9% extraCPU. Tested8/10/12ms on large,
Basic and matching;12ms met the budget with little elapsed penalty. Generation
target remains10ms. This is a scheduling heuristic, not a universal guarantee.

UNINSTRUMENTED release/mimallocv2 option26=0,10CPUs, three samples unless noted:
- Basic BOTH custom saturations: before7.689900s /54.919213CPU; final6.168526s
  /41.462464CPU. New serial33.523s /33.520812CPU (ONE sample), +23.69%CPU.
- Large fixed second maximum: before1.266139s /4.359859CPU; final1.123922s
  /3.059804CPU. New serial2.384s /2.385624CPU (3samples), +28.26%CPU.
- Matching fixed step10: before17.159685ms /55.100067CPUms; final15.376311ms
  /51.010767CPUms. New serial31ms /31.138667CPUms (3samples,30kernels/process),
  +63.82%CPU, about20CPUms extra. Absolute CPU fell; tiny-kernel dispatch overhead
  is still a future opportunity. Do not claim every workload stays below30%.
Serial records precede the last PARALLEL-only pruning adjustment; serial code
is unaffected. Complete canonical serial/parallel/before maxima agree.
Old RE1 times are reused, NOT rerun: Basic27.496/272.056CPU, large19.636/191.012,
matching103.219ms/439.834CPUms. All comparisons are maxima kernels, not full RE/UI.

Synthetic twin-label wide fixtures (aliases always occur together):116-label
large and94-label matching demonstrate packing beyond64. Final native before/
after records: review-final-{large,matching}-wide.jsonl, complete equality.
Original old problem JSON sections are unchanged driver scaffolding, NOT
equivalent inputs for old-library timing; use resource-new only for these files.

Authoritative records: review-final-{basic,large,matching}.jsonl; paired previous
8ms production files review-production-*.jsonl; serial review-production-*-serial;
policy review-policy-*; final wider-label files. Adjacent .results store complete
maxima plus per-trial peak RSS and binary SHA256. Initial review-first/alignment/
combined screening used per-query string flags, so do not quote their differences
as final numbers. Later prototype flags parse once to bits; production has NONE.
investigate_review.py freezes the reviewed kernel files (including8ms Antichain)
under baseline/2026-10-13/review. investigate_review_policy.py copies current
integrated source and substitutes8/10/12ms in temporary code only. Old pre-live
investigation scripts keep their separate historical baseline. Updated their
restoration of old configuration exports, wrapper/candidate signatures and labels
so current production changes do not break or contaminate those earlier trials.

147core+5server+2wasm-adapter=154 Rust tests pass. Tests cover reused buffers after
early exit, all2x3 component graphs, cache bit127/larger fallback/Hall obstruction,
65/128-bit real closure vs vector sets, uniform degree3 coverage and dense history
vs ordered traced closure.20maximization (128/129 labels),18RE,6BasicFP fixtures
agree through BOTH transports. Native server and wasm rebuilt; last12ms change
is cfg-native only. Frontend typecheck/build, Rustfmt and git diff --check pass.
Server still uses SYSTEM allocator; matched benchmark executable uses mimallocv2.
Historical packed builder prepares and passes cargo check --release; original/
intermediate profiling anchors also prepare successfully after restoration.
No commits/pushes. Experiments complete; stop optional sweeps until requested.

## Authorized allocator and four final experiments (2026-10-13)

User now explicitly requested changing the server allocator, THEN testing ALL
four remaining proposed categories and stopping. Native server gets pinned
mimalloc v2 (wrapper0.1.52/sys0.1.49), option26=0 before Tokio workers; core/wasm
do not acquire an allocator. Four categories: (1) flow-network reuse, (2) interned
or multiword sets beyond128, (3) overlap generation/pruning, (4) history compression
AND small-workload scheduling. Build real controlled prototypes, compare complete
canonical maxima and serial/parallel wall/totalCPU/memory as appropriate; integrate
useful changes, preserve cancellation/tracing/wasm/generic algebra. Stop after all
four categories are tested; no unrelated feature or optional fifth optimization.
Incidence-coloring-after-RE and the other review work remain deferred.

## Allocator and all four experiments completed (2026-10-13)

User's stopping condition is satisfied: ALL four proposed categories have been
tested. Do not continue optimization sweeps or start a fifth idea. Only required
verification/reporting remained at completion. Incidence-coloring-after-RE and
the broader REVIEW.md issues remain deferred. Portable report: crates/core/benches/
re_comparison/investigation/final-four.md; frozen pre-change sources in baseline/
2026-10-13/final-four, helpers in benches/re_comparison/final_four/.

Native SERVER NOW uses pinned mimalloc0.1.52 with v2/sys0.1.49 extended; option26=0
before Tokio starts. Option26 is abandoned_reclaim_on_free in the pinned v2 C
header, absent from the wrapper's older Rust constants. Global allocator belongs
to server main.rs, not core/wasm. Cargo.lock and README/FEATURES updated. No v3
or allocator sweep. Build, feature-tree inspection and transport smoke pass.

1. Flow-network reuse: tried thread-local retained petgraph Graph and destination
indices, topology rebuilt, Dinic unchanged. Reentrant callbacks use fresh storage.
Native matching/large/Basic3samples found no meaningful runtime gain. REJECTED
because persistent state/reentrancy adds complexity for negligible/no improvement.
2. Wider sets: tested fixed4words and dynamic Arc<[u64]> on dense232/522 and
sparse174/522 alphabets. Numeric HIGH-word-first ordering helped dense traversal.
ADOPTED u64<=64,u128<=128,four-word129..256,dynamic>256, with average source set
cardinality>=word-count for the WIDE representations. Sparse wide stays vectors.
Density is a heuristic, not a guarantee or search cap. No model/wire changes.
3. Overlap: reserved5/5 or8/2 gen/prune workers in ONE bounded Rayon pool, bounded
channel, frozen parents/history, both immediate and1024-fresh buffered pruning.
Canonical maxima agreed. Large grew~1.11→3.6–4.1s and CPU3.07→5.55–5.85s.
Basic screening ONE sample:6.139/40.999CPU→9.489/36.746 or7.911/37.399. Some
CPU saving but large latency regressions. REJECTED for default. No claim that
every possible pipeline is slower; retain the prototypes for later if requested.
4. Compression/scheduling: exact value dictionary+varint configuration keys,
inline128-byte lookup key with unlimited overflow, Ord-only (no Hash/Serialize).
Broad compression hurts small packed/custom cases but saves memory. Dynamic522
benefits in BOTH time/CPU and memory:2.376/7.104CPU/221MiB→2.135/6.329/47MiB.
ADOPTED compact history ONLY for dynamic wide masks. New maximize/history.rs;
engine/combination use that private strategy, public Operations bounds unchanged.
Grains256/512/1024 tested. Unconditional512/1024 slows matching. ADOPTED512 only
for packed degrees whose normalized seeds all have unit counts; otherwise256.
Public custom algebras keep1, tracing remains ordered. Large CPU saves~4–5%;
matching unchanged. No dynamic, input-promise or semantic-coverage shortcuts.

Caught and fixed a production-only integration regression: wrapping FourWords
in generic BitAnd/Not caused wide2323.15s/15.37CPU despite prototype1.58/5.03.
Replaced it with the directly tested borrowed FourOperations and early-exit
subset/admissibility loops. The bad intermediate timings are preserved as
four-integration-generic-quad.jsonl/.results and are NOT current performance.
Use final four-final-wide232.jsonl. This validates why uninstrumented production
confirmation is necessary; canonical outputs stayed correct throughout.

Final UNINSTRUMENTED native release/mimallocv2 option26=0,10logicalCPUs.3samples,
alternating before/after. Matching30kernels/process; sparse1745, sparse5223.
- Large old RE3 before1.114255s /3.070091CPU; final1.122685s /2.927562CPU.
  Final serial2.358451s /2.359880CPU(3samples), +24.06%parallelCPU. <=2s/<=30%met.
- Matching14.921636ms /50.625233CPUms→14.826376ms /49.834567CPUms. No material
  latency regression. Relative parallel overhead is still a small-kernel issue.
- Basic BOTH custom saturations6.148510s /41.319611CPU→6.124164s /41.297710CPU.
  Final serial33.919688s /33.912157CPU(ONE costly sample), +21.78%parallelCPU.
- Dense synthetic232labels2.897801s /9.883955CPU /213.16MiB RSS→1.586539s
  /5.033741CPU /125.33MiB. Dense synthetic5224.018486s /14.826197CPU /386.18MiB
  →2.137495s /6.028839CPU /52.62MiB. Twin labels always occur together; these
  test related closure shapes, not all wide application problems.
- Sparse174~9.19ms before/final; sparse52281.28→80.77ms, vectors preserved.
Full canonical maxima agree before/after and serial/parallel. No old RE1 runs
or new whole-RE-sequence/UI speedup claim. Server uses the same allocator now,
but these wall numbers still measure native kernels excluding transport/render.

Authoritative records four-final-{large,matching,basic,wide232,wide522,sparse174,
sparse522}.jsonl plus -serial files, with adjacent complete .results/RSS/hashes.
Prototype four-{large,basic,matching,wide232,wide232-high,wide522,sparse*,buffered*,
compressed-words,unit-matching}.jsonl retain all four categories and tradeoffs.
Production has no profile flags. Old wrapper restoration added to
investigate_review.py so later private history/wide changes do not contaminate
that historical experiment. final_four/prepare.py restores the frozen pre-four
kernel files before injecting its benchmark-only variants.

149core+5server+2wasm=156Rust tests pass. Added exact full/compact history parity
with non-Hash/non-Serialize values, IDs/counts crossing varint boundaries, U32MAX,
empty configuration and overflow keys; wider dense closure vs vector oracle at
129/256/257/522, noncontiguousIDs, cross-word joins, zero degree/huge counts.
Native server/wasm rebuilt.23maximization fixtures(including through522labels)
pass BOTH transports after the direct-four-word correction;18RE+6BasicFP both
transports passed the integrated wider/history version before that equivalent
direct-ops correction. Frontend typecheck/build,Rustfmt/gitdiffcheck pass.
Frozen final-four and previous review experiment copies both pass cargo check
--release after restoring their historical wrappers. No leftover servers, builds
or benchmark jobs. No commits/pushes. All four experiments complete; STOP work.

## Coloring slowdown investigated, including other kinds (2026-10-14)

User authorized investigating incidence coloring on RE(BMM), clarified DEGREE10
(not the degree-three preset), then requested checking the other coloring kinds.
Completed investigation; no production algorithm integration was requested/done.
Report/helper prototypes/fixtures/raw records: crates/core/benches/coloring/.
Original BMM active M U^9/P^10, passive M UP^9/U^10. After one RE: active A D^9/
B C^9; passive AB BCD^9/D^10, four labels and two rows per side.

Native CaDiCaL's weak refinement path stalls at palette11:30s cutoff,63215models,
63211ten-literal passive clauses, only3active encodings,349vars/63894clauses.
27.32s solving/2.68s verifying; unbounded preflight tiny. Production wasm BatSat
finishes389ms/1807models; native BatSat screen295ms/1807models proves backend
choices matter much more than wasm/native execution here. Do not switch default.

Recommended THREE changes, currently only TEMP copies generated by prepare.py:
1 lexicographic ordering of whole color/input output-membership vectors, equality
allowed, gate successor enabled color. Safe for vertex/graph/incidence kinds,
including original inputs. Do NOT sort distance-two rows: must permute columns
too; no distance-two symmetry implementation attempted.
2 eager passive degree<=2 clauses reuse existing shared visit_forbidden, encode
every origin, extend only new colors, disabled memberships make clauses valid.
Works in all local-color kinds and distance-two (always passive degree2).
3 group equal output DOMAINS regardless of different source literals. Normalize
containment/enumeration with Configuration<LabelSet>; keep original provenance
lists and distribute each rejected group's counts back over original parts.
This avoids 2^10 repeated assignments for eleven B/D multisets without losing
input/color origins. General exact, no maximization required, no model changes.

Native medians before→allthree: incidence BMM75.075→1.079ms; RE(BMM)30scutoff→
15.875ms/112models; vertex RE(BMM)fixed10 UNSAT273.675→1.335ms; graph-edge MM
degree10max2.643→0.602ms; orientation-input vertex5max8.334→1.395ms; D2matching
degree3fixed5 UNSAT1.925→0.667ms; D2orientationinputfixed6 UNSAT54.120→28.259ms.
All completed decisions agree; witnesses independently verified outside timing.
Matched wasm release/no-wasm-opt before→allthree RE(BMM)397.194→40.273ms,
1807→157models, BMM29.998→13.181ms. Other wasm kinds agree/improve(single screens).

Rejected default: unrestricted all-placement clause batching508ms, ordering+
batch256ms despite only21models; more verification work. Max mode alone times
out15s, ordering+maxmode301ms/1079models (whole-family clauses weaker). General
with precomputed max+ordering62ms; add homogeneous probes32.5ms; max prep1.3ms.
Grouping wins16ms without extra max. Probes alone still timeout10s, grouped alone
still timeout10s. Keep caches/input-aware artifact reuse as separate future work.

31 native TEMP-core solvability tests pass, including existing plain/input/paired
exhaustive oracles and color/neighbor-color references. New tests: RE(BMM)degree
2/3/10boundary, both solvers disabling/re-enabling colors, and independent64mask
provenance oracle for counts1/2/4: every returned clause checked against ALL future
emission supersets.38production-script checks pass against temporary grouped wasm.
Source model/API/UI/native solver config unchanged; only bench tools/data and
memory/docs edited. No commits/pushes. Stop experiments; integration awaits a
new user instruction. Retain user preference for shared/generic code in proper
places: generic bit comparator SAT, eager/provenance shared solvability/passive,
color placement in corresponding coloring solver. Do not normalize UI models.
For eventual integration retain lazy fallback for very large potential-origin
pair counts: eager degree2 can become expensive with large alphabets/inputs/colors.
No crossover threshold was benchmarked; this is an encoding choice, never a
mathematical search cutoff. Grouping has some overhead when every domain differs.


## Coloring improvements integrated (2026-10-14)

User explicitly authorized integration ("do it") after the prototype status
answer. All THREE selected coloring improvements are now production code:
- Formula::lexicographic_order in algorithms/sat.rs, gated by the successor
  enabled color. Local-color membership vectors include all original inputs;
  equality permitted. Applied to vertex/graph-edge/incidence kinds ONLY.
  Distance-two retains its own/neighbor indices, without row-only sorting.
- Both coloring solvers reuse passive::visit_forbidden for affordable passive
  degree<=2 rows. Shared eager_encoding_work uses saturating arithmetic and
  EAGER_ENCODING_BUDGET=100_000 possible-origin tuples per query. This is an
  encoding scheduling heuristic, NOT a search/palette/problem limit and NOT a
  measured optimal crossover. Larger encodings keep exact lazy refinement.
  Each input row caches the largest wholly eager palette. Extensions encode
  only newly colored placements; smaller queries reuse previous clauses. A row
  may become lazy on a larger palette without losing prior eager clauses.
  No valid color placements means no passive obligation. In particular, avoid
  accessing off-diagonal memberships when distance-two has one color; this
  matters for isolated-node cases and was caught by existing production tests.
- Shared passive checker merges equal semantic output domains regardless of
  source origins, using Configuration<LabelSet> normalization. Original
  OutputParts retain provenance; rejected grouped counts are distributed back
  over original slots before constructing the blocking clause. No immutable
  model/UI normalization, new API, solver backend or input promise change.
  Eager origin scans also check cancellation, including before filtering.

Production confirmation: native uninstrumented release, pinned mimalloc v2,
three runs with no concurrent builds/tests/heavy jobs. Degree10 BMM maximum10
1.034ms; RE(BMM) maximum10 15.861ms; vertex RE(BMM) palette10 UNSAT1.243ms;
graph-edge matching maximum10 .550ms; orientation-input vertex maximum5
1.365ms; mapped list palette3 UNSAT .174ms. Distance-two matching palette4
SAT .236ms / palette5 UNSAT .577ms; proper3coloring palette4 UNSAT .230ms;
orientation-input palette6 UNSAT27.905ms. All decisions agree with historical
cases; each returned SAT witness independently verified outside the timer.
Optimized production wasm three runs: BMM13.198ms / RE(BMM)38.911ms, both
maximum10. Compare with prior wasm-opt production389ms, NOT the no-opt paired
prototype results. Full results: benches/coloring/results/native-integrated.jsonl
and wasm-integrated.jsonl. No new historical baseline reruns or universal claim.

157core+5server+2wasm=164workspace Rust tests pass. New regressions cover full
bit-comparison truth tables including disabled gates/negated literals, both
solvers shrinking/re-enabling colors, real larger-domain eager/lazy transitions,
RE(BMM)degree2/3/10 and independent64mask provenance oracle checking all future
source supersets. Existing original-input/exhaustive/neighbor-color/zero-degree/
cancellation oracles remain. Native server and optimized wasm rebuilt; frontend
typecheck/build pass. web/scripts/test-coloring.mjs now includes RE(BMM)degree10
and passes39checks through BOTH wasm and native server, including bounds.
Temporary server stopped. Rustfmt and git diff --check pass; ts-rs's incidental
trailing whitespace in unchanged generated files cleaned after tests.

README/FEATURES/REVIEW/bench reports reflect integration. Historical four kernel
files frozen in benches/coloring/baseline so prepare.py still reproduces original
experiments after copying the current crate. Experimental selected copy still
passes cargo check --release. Production code has no diagnostic counters/flags.
No commits/pushes. Task complete; don't start additional sweeps or implement
other REVIEW ideas without a new user request. Future work includes calibrating
encoding budget on larger domains, valid distance-two row/column symmetry and
reuse of already computed bounds/certificates/maxima where mathematically valid.
