# Round Eliminator 3

This is a new implementation on the `re3` branch. Start parses and validates
active/passive problem definitions, and the GUI displays the returned definition.
It supports plain problems and all three input variants described below.
Algorithms and computed properties are not
implemented yet; see [FEATURES.md](FEATURES.md).

The root is a virtual Cargo workspace. `crates/core` contains the portable
problem model, parser, and request handler; `crates/server` and `crates/wasm`
are thin delivery adapters. `web` contains the Vue interface.

## Constraint text

Each nonempty line is an allowed configuration. Whitespace separates parts.
One-character labels can be written together as choices, as in `AB`; use
parentheses for a longer label, as in `(red)(blue)`. A suffix `^n` repeats a
part `n` times. For example, `M U^9` has exact degree 10. Lines of different
degrees are accepted, including on one side. Starred configurations are
currently rejected.

An inclusive exponent range such as `A^5..8` expands into four configurations,
with 5, 6, 7, and 8 occurrences. Ranges in different parts vary independently:
`A^1..2 B^2..3` expands into four configurations. This works with label choices
and input/output pairs too. Zero occurrences omit the part (`A^0` or a range
including zero); a line with all parts omitted becomes `()`.
Endpoints must be nonnegative integers in ascending order. There is no fixed
limit on the number of expanded configurations. The model stores only exact
multiplicities, and degree inference uses the expanded configurations.

A line containing `()` denotes the empty multiset, the unique degree-zero
configuration. A blank constraint has no allowed configurations. These are
different: an empty constraint does not permit degree-zero configurations.

## Problem variants

| Variant | Entry and meaning |
| --- | --- |
| No input | Output active/passive constraints. |
| Independent output validity | Separate input and output constraints. The supplied input is promised to satisfy the input constraints; the output must satisfy the output constraints. |
| Input/output pairs | Active/passive constraints on pairs. `(red,X)(blue,Y)^2` independently chooses one of those two pairs twice. Input names and output names use separate tables. The input is promised to satisfy the projection onto the first components. |
| Input-to-output mapping | Separate input and output constraints, plus one `input -> outputs` line per input label. For example, `(red) -> AB` permits A or B on input red. An empty right side permits no output. |

In pair notation, labels inside `(input,output)` may be longer than one character
and must not contain whitespace, commas, parentheses, `^`, or `*`.
In a mapping, use `(name)` for longer labels, as in ordinary constraints.
Mapping entries may introduce labels absent from the constraints. Every input
label must have exactly one mapping entry.

## Graph class

List possible active and passive degrees separately, separated by spaces or
commas. Leaving a field blank infers that side's degrees from the input
constraints, or from the output constraints for a problem without inputs.
Inference happens once when constructing the problem. Output validity does not
subsequently change the graph class.

An allowed degree with no output configurations has no legal output at a node
of that degree. Configurations at excluded degrees remain in the definition but
are irrelevant to the chosen graph class; the UI marks them accordingly.
With inputs, the graph must additionally admit the promised input labeling.
For pair problems, blank degree fields use the input projection. Explicit degree
lists restrict the graph class in addition to the promise of a valid input labeling.

If inference finds no configurations, it produces an empty degree set, allowing
no nodes on that side. Degree zero is explicitly supported using `()`.

## Core layout

| File | Responsibility |
| --- | --- |
| `crates/core/src/labels.rs` | Numeric IDs and sorted sets of IDs or direct pairs. |
| `crates/core/src/problem/mod.rs` | Problem variants, graph/input promises, and input-to-output mapping validation. |
| `crates/core/src/problem/constraint.rs` | Generic checked parts and condensed configurations; grouping by computed degree. |
| `crates/core/src/problem/constraint_pair.rs` | Ordinary and input/output constraint pairs, their label tables and reference validation, and input projection. |
| `crates/core/src/problem/graph.rs` | Resolved active/passive degree sets. |
| `crates/core/src/parser/mod.rs` | Text request types, variant assembly, degree and mapping parsing, and parse errors. |
| `crates/core/src/parser/notation.rs` | Shared row/repetition grammar and the label/pair readers. |
| `crates/core/src/protocol.rs` | Shared request dispatch and conversion of parse errors into API responses. |

The model and parser each keep their tests in a neighboring `tests.rs` module.
The constraint containers are generic over two actual element types: `LabelId`
and `LabelPair`. A pair contains two IDs indexing separate input/output tables.
Two concrete types own label names and validate references: `ConstraintPair`
has one label table and ordinary active/passive constraints;
`InputOutputConstraintPair` has separate input/output tables and constraints on
pairs. Problem variants compose these validated types. `PairedProblem` keeps
its input projection as an ordinary `ConstraintPair`, including a copy of the
input names with the same IDs, alongside its `InputOutputConstraintPair`.
Model fields are private; construction checks invariants. Equality is structural,
and configuration normalization is still deferred.
`CondensedConfiguration` denotes a family of multisets: for example, `AB^2`
represents `AA`, `AB`, and `BB`, rather than one concrete configuration.

## Run

From this directory:

```sh
cargo test --workspace
wasm-pack build crates/wasm --target web --out-dir ../../web/public/wasm --out-name round_eliminator_3_wasm --release
cd web && npm ci && npm run build && npm run test:wasm
cd .. && cargo run -p round-eliminator-3-server
```

Open `http://127.0.0.1:8080`. The selector beside Start chooses Server or
WebAssembly; both send the same JSON request to `execute_json` in the core
crate. For frontend development, run `npm run dev` in `web` while the Rust
server runs; Vite proxies `/api` to it. The WebAssembly build is needed before
selecting WebAssembly.

`cargo test --workspace` also regenerates the Rust-defined TypeScript types in
`web/src/generated`. Commit changes to those files alongside protocol changes.

To compare native and wasm responses for all variants, run the server, then run
`npm run test:wasm -- http://127.0.0.1:8080/api` from `web`.
Build wasm before building the frontend so that `web/dist` contains the latest
wasm artifacts.
