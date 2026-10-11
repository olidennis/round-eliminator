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

## Zero-round solvability decisions

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
(`output_allowed`) live on `Problem`. Algorithm-specific context/domain preparation
and absent-input conventions remain in `algorithms/zero_round/prepare.rs`.
The former `Validity` selector is removed: search and verification use the shared
ordinary output constraints directly. The model must not depend on zero-round
witness types or SAT preparation.

Module placement after the organization review:

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
  enumeration remain algorithms in `algorithms/configurations.rs`.
- Generic SAT cardinality operations (`Formula::exactly_one`, `Formula::at_most`)
  and their tests belong in `algorithms/sat.rs`, alongside backend setup. Use
  RustSAT Ladder for at-most-one and Totalizer for larger bounds. Complete native
  and wasm checks on 19 fixtures favored the library over the former custom
  sequential counter; see `crates/core/benches/README.md`. Keep a shared variable
  manager and check cancellation while streaming generated clauses to the solver.
  RustSAT CollectClauses exposes only an out-of-memory error: the private adapter
  stores the actual cancellation/solver error, aborts generation through that
  channel, then restores the original error before returning to callers.
  The zero-round matching encoding belongs in `zero_round/encoding.rs`.
- Zero-round `mod.rs` holds the public types, entry points, and search loop;
  `prepare.rs` prepares rules, condensed passive inputs and output domains;
  `passive.rs` owns emission families, passive checks and blocking clauses;
  `verify.rs` verifies complete witnesses using general mode. Generic counted
  containment belongs in `algorithms/configurations.rs`, alongside membership.
  `zero_round/tests.rs` holds search tests and independent exhaustive helpers;
  `zero_round/tests/passive.rs` groups passive-family and blocking-clause tests.
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
independent of subsequent editor changes. Replacing the parsed problem
cancels its running analysis. Cancellation leaves a retry button.

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
