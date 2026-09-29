# Round Eliminator 3 project brief

This branch is a ground-up rewrite. The source worktree is `/workspace/round-eliminator`
on `llm-enhanced`; this worktree is `/workspace/round-eliminator-3` on `re3`.
Keep both worktrees. Read only `round-eliminator-lib`, `round-eliminator-server`,
`round-eliminator-wasm`, and the GUI files needed to understand them in the old tree.

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
  overflow, label references, and mapping completeness. Deserialize text requests,
  then construct validated models; do not bypass invariants by deriving model
  deserialization. Graph classes may contain degrees with no valid outputs.
- `LabelSet` has private sorted-vector storage and set operations. Use it as the
  default abstraction and benchmark realistic workloads before changing storage.
  Its JSON shape is an array. CondensedConfiguration normalization remains deferred.
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
