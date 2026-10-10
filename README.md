# Round Eliminator 3

This is a new implementation on the `re3` branch. Start parses and validates
active/passive problem definitions, and the GUI displays the returned definition.
It supports plain problems and all three input variants described below.
The zero-round check supports all four variants and returns a local-rule witness.
Other operations remain listed in [FEATURES.md](FEATURES.md).

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
| `crates/core/src/problem/mod.rs` | Problem variants, shared graph/input/mapping accessors, and mapping validation. |
| `crates/core/src/problem/configuration.rs` | Concrete multisets stored as sorted label/multiplicity entries. |
| `crates/core/src/problem/constraint.rs` | Generic checked parts and condensed configurations; grouping by computed degree. |
| `crates/core/src/problem/constraint_pair.rs` | Ordinary and input/output constraint pairs, their label tables and reference validation, and input projection. |
| `crates/core/src/problem/graph.rs` | Resolved active/passive degree sets. |
| `crates/core/src/parser/mod.rs` | Text request types, variant assembly, degree and mapping parsing, and parse errors. |
| `crates/core/src/parser/notation.rs` | Shared row/repetition grammar and the label/pair readers. |
| `crates/core/src/protocol.rs` | Shared request dispatch, generated response/progress types, and conversion of errors into API responses. |
| `crates/core/src/computation.rs` | Transport-independent cancellation flag and computation phases. |
| `crates/core/src/algorithms/sat.rs` | RustSAT formula builder and target-specific backend setup. |
| `crates/core/src/algorithms/configurations.rs` | Concrete input multiset enumeration and exact membership in condensed constraints. |
| `crates/core/src/algorithms/zero_round/mod.rs` | Public result types and shared SAT search with counterexample refinement. |
| `crates/core/src/algorithms/zero_round/prepare.rs` | Zero-round input preparation and shared validity operations on ordinary or pair constraints. |
| `crates/core/src/algorithms/zero_round/sat.rs` | Cardinality and active configuration matching encodings. |
| `crates/core/src/algorithms/zero_round/verify.rs` | Exact passive checks and independent witness verification. |

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
`Configuration<L>` stores a single multiset as `(label, multiplicity)` entries;
`A^2 B` has two entries, not three occurrences. Construction merges repeated
labels, sorts entries, discards zero counts, and checks degree overflow. It works
with ordinary labels and direct input/output pairs. Algorithms expand occurrences
only where port positions matter, such as local-rule witnesses.

## Zero-round solvability

Press **Start** to display the parsed problem, then choose **Check zero-round
solvability** inside that problem to search for a deterministic active-side strategy in the port-numbering model. An active node knows its own degree,
local ports, and all incident inputs; it does not know its neighbors' degrees.
Each witness row specifies outputs for one active degree and input multiset.
Input occurrences are ordered by label ID, breaking ties by local port order;
the UI shows the required input order. Outputs follow that same order. Without
inputs, outputs use local port order. Repeated input labels can receive different
outputs, and an output can depend on the full active input configuration.

Start parses the text once, including exponent-range expansion. Subsequent
zero-round requests carry that displayed structured problem through either
transport. Serde deserializes the model directly; the client is trusted to send
the valid snapshot returned by Rust. Constructors validate newly built models.
Editor changes do not affect an already displayed problem.
The zero-round response contains only the solvability result and any witness;
witness label IDs refer to the submitted problem's label tables.

The check uses RustSAT with packaged CaDiCaL 2.2.1 for native builds and pure Rust
BatSat for wasm. Target-specific dependencies keep CaDiCaL out of wasm builds.
Solvers may produce different valid witnesses and require different numbers of
refinement iterations. The encoding
uses RustSAT literal types and generic solver interfaces; backend construction
and cancellation setup live in `algorithms/sat.rs`. A different incremental
backend can run the same encoding and refinement loop. It selects outputs
and matches them to active configuration parts using capacities, without
enumerating output permutations. Passive degrees up to two are encoded eagerly.
For higher degrees, an exact checker searches for a forbidden combination of
outputs and adds a clause excluding it; solving and checking repeat until a
valid strategy is found or SAT proves unsolvability. Membership is tested against
the union of all allowed configurations, so passive maximization is unnecessary.
Repeated equal inputs are checked using output multisets, avoiding permutations.
Concrete membership uses petgraph's Dinic maximum flow: distinct labels and
condensed parts are vertices, and multiplicities are capacities. Large exponents
do not expand into flow vertices. Single-label and single-part cases are checked
directly; insufficient capacity rejects a candidate before running flow.
See [matching benchmarks](crates/core/benches/README.md) for measurements and
the comparison with the previous occurrence matcher.

Only degrees in the graph class participate. With inputs, the input promise
determines the local configurations to consider. Every listed active input
configuration at an allowed degree is an obligation; the check does not first
decide which input configurations have global graph realizations. A passive
input with no compatible active neighbors creates no obligation. Degree-zero
configurations are checked separately, including on the passive side.

`algorithms::zero_round::verify_strategy` checks witness coverage, active validity,
mapping restrictions, and universal passive validity without a SAT solver.
Tests compare search results against exhaustive strategies on small instances.
The search has no imposed configuration or iteration limit; concrete input
enumeration and passive checking can still be exponential.

While running, the check button becomes an indeterminate progress indicator,
a description of the current phase, and a **Cancel** button. Completed SAT
candidate counts are shown when available; no percentage or completion time is
estimated. Results appear below the constraints. Cancellation preserves the
parsed problem and offers a retry. Editing the form does not change an existing
parsed problem or its analysis; pressing Start replaces it and cancels its work.

Native requests run in a blocking pool with concurrency bounded by available
CPUs. The UI uses `POST /api/stream`, which streams Rust-defined
`ComputationEvent` values over SSE. Closing the stream cancels queued or running
work; the permit is held until the worker exits. The same cancellation flag is
checked during parsing range expansion, input enumeration, clause generation,
SAT solving, and passive checking. Cancellation is cooperative at these checks.
`POST /api` remains a convenience endpoint returning only the final JSON result.

Wasm runs each request in a dedicated worker and reports the same progress events
through messages. Cancellation terminates that worker, including its synchronous
wasm execution. Both transports use the same Rust request dispatch.

## Run

From this directory:

Native builds require a C++ compiler and libclang: RustSAT compiles the bundled
CaDiCaL sources and generates its bindings with bindgen. On macOS, install the
Xcode Command Line Tools; on Debian/Ubuntu, install `build-essential` and
`libclang-dev`. Wasm builds use BatSat and do not need these native dependencies.

```sh
cargo test --workspace
wasm-pack build crates/wasm --target web --out-dir ../../web/public/wasm --out-name round_eliminator_3_wasm --release
cd web && npm ci && npm run build && npm run test:wasm
cd .. && cargo run -p round-eliminator-3-server
```

Open `http://127.0.0.1:8080`. The selector beside Start chooses Server or
WebAssembly; both send the same JSON request through the core request handler. For frontend development, run `npm run dev` in `web` while the Rust
server runs; Vite proxies `/api` to it. The WebAssembly build is needed before
selecting WebAssembly.

`cargo test --workspace` also regenerates the Rust-defined TypeScript types in
`web/src/generated`. Commit changes to those files alongside protocol changes.

To compare parsed definitions and solvability decisions for all variants, run the server, then run
`npm run test:wasm -- http://127.0.0.1:8080/api` from `web`.
Build wasm before building the frontend so that `web/dist` contains the latest
wasm artifacts.
