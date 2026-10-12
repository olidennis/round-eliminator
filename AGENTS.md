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
passive modes; the GUI uses general until maximization exists.

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
state in `ColoringAnalysis.vue`; both analyses use `ComputationProgress.vue`.
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
hand-written maximized fixtures and no maximization implementation yet.
Do not keep the expanded-input or direct SAT containment implementations in
production. Historical results are in `crates/core/benches/passive_inputs/results.csv`;
the runner now compares the two adopted modes without source patching.
Containment can safely accept a family without maximization, but failed
containment cannot reject it unless maximization is guaranteed. Temporary merging
of output parts with equal choices and origins is not model normalization.
The GUI currently sends general mode. Add a user choice when maximization exists;
its total cost includes maximizing the constraint. Do not add a stale maximization
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
degree-one contexts for passive degree 2. No maximization implementation yet.
Cheap input-output arrows lift known output arrows through mapping permissions;
exact arrows also consider the passive input promise. Vertices are admitted
`DiagramNode { input, output }` pairs; arrows always keep the input fixed.
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
`DiagramView.vue` lazily loads Cytoscape/Dagre for the selected diagram, two
layouts, selection and text/PNG exports. The user requires choosing the diagram
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
