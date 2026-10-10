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
- The model has plain, independent-input, paired, and mapped variants. Input and
  output label tables are separate. Store input/output pairs directly as two
  numeric IDs; do not intern pairs through another table.
- `Part<L>`, `CondensedConfiguration<L>`, `Constraint<L>`, and `LabelSet<L>` share
  container code for two concrete element types: `LabelId` and `LabelPair`.
- Use two concrete aggregate types in `problem/constraint_pair.rs`:
  `ConstraintPair` owns one label table and ordinary active/passive constraints;
  `InputOutputConstraintPair` owns separate input/output tables and constraints
  on direct label pairs. Their constructors validate names and references.
  Keep these types distinct rather than adding a generic aggregate or wrappers.
- Problem variants compose these validated types. `PairedProblem` stores an
  `InputOutputConstraintPair` and its projected input promise as an ordinary
  `ConstraintPair`. The projection retains a copy of the input names and IDs.
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

The user approved removing the zero-round `View` struct. General borrowed
accessors (`graph_class`, `input`, `allowed_outputs`) live on `Problem`.
Algorithm-specific context preparation and absent-input conventions remain as
functions in `algorithms/zero_round/prepare.rs`. Keep the private `Validity`
selector for ordinary and pair constraints; the problem model must not depend
on zero-round witness types or SAT preparation.

The current decision criterion requires every listed active input configuration
at an allowed degree to have a rule, and all compatible passive combinations to
be valid. It does not solve global realizability of the input promise. Returned
rules sort input occurrences by label ID and break ties by local port order;
outputs align with this order. Plain rules use local port order directly.
The protocol request `CheckZeroRound(Problem)` takes the already parsed definition
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
