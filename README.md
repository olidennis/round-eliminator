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

The core has four areas: `model` defines the domain types, `parser` reads notation,
`algorithms` analyzes and transforms problems, and `api` exposes that work to
callers. `computation.rs` is the shared execution context used by parsing,
algorithms, and API dispatch. It stays one file because progress and cancellation
form a small, cohesive module. In Rust, `computation.rs` and a directory such as
`api/mod.rs` are both modules; a directory groups a module's related files.
The model uses the equivalent `model.rs` entry file with its children in `model/`.

```text
crates/core/src/
├── lib.rs                 # module declarations and convenient public exports
├── computation.rs         # shared progress and cooperative cancellation
├── model.rs               # model module declarations and public exports
├── model/
│   ├── problem.rs         # Problem, Input, OutputMapping, and ValidationError
│   ├── labels.rs
│   ├── constraint.rs
│   ├── constraint_pair.rs
│   ├── configuration.rs
│   ├── graph.rs
│   └── tests.rs
├── parser/                # text entry, notation, and paired conversion
├── algorithms/            # matching, SAT support, and problem algorithms
└── api/
    ├── mod.rs             # shared dispatch and JSON execution
    ├── protocol.rs        # request/response/event schema
    └── tests.rs           # API behavior and cancellation tests
```

Use `model::{Problem, LabelId, LabelSet, ...}` for model types and `api::{Request,
Response, ...}` for API types and execution. The server and wasm adapters depend
on this same API; folder organization does not change the JSON schema.

| File | Responsibility |
| --- | --- |
| `crates/core/src/model/labels.rs` | Numeric label IDs, sorted label sets, and shared label-name validation. |
| `crates/core/src/model.rs` | Model module declarations and public exports. |
| `crates/core/src/model/problem.rs` | Shared mapped problem, compact input/mapping enums, accessors, and construction errors. |
| `crates/core/src/model/configuration.rs` | Concrete multisets, condensed configurations, and their parts; multiplicity validation. |
| `crates/core/src/model/constraint.rs` | Collections of allowed configurations; grouping and lookup by degree. |
| `crates/core/src/model/constraint_pair.rs` | Ordinary active/passive constraint pairs, their label tables and reference validation. |
| `crates/core/src/model/graph.rs` | Resolved active/passive degree sets. |
| `crates/core/src/parser/mod.rs` | Four text entry forms, shared model assembly, degree and mapping parsing, and parse errors. |
| `crates/core/src/parser/paired.rs` | Private temporary pair types, validation, input projection, and conversion to ordinary output labels. |
| `crates/core/src/parser/notation.rs` | Shared row/repetition grammar and the label/pair readers. |
| `crates/core/src/api/mod.rs` | Shared request dispatch and JSON entry points used by both adapters. |
| `crates/core/src/api/protocol.rs` | Request, response, progress-event, and API error schema; generated TypeScript types. |
| `crates/core/src/computation.rs` | Transport-independent cancellation flag and computation phases. |
| `crates/core/src/algorithms/sat.rs` | RustSAT formula builder, generic cardinality encodings, and target-specific backend setup. |
| `crates/core/src/algorithms/configurations.rs` | Concrete input multiset enumeration and exact membership in condensed constraints. |
| `crates/core/src/algorithms/zero_round/mod.rs` | Public verification mode/result types and shared SAT search with counterexample refinement. |
| `crates/core/src/algorithms/zero_round/prepare.rs` | Prepare active rules and retain condensed passive input families. |
| `crates/core/src/algorithms/zero_round/encoding.rs` | Encode active configuration matching for the zero-round search. |
| `crates/core/src/algorithms/zero_round/passive.rs` | Shared passive emission families, containment, exact output checks, and blocking clauses. |
| `crates/core/src/algorithms/zero_round/verify.rs` | Complete witness verification using the general passive checker. |

The model, parser, and API each keep their tests in a neighboring `tests.rs` module.
Short modules keep small test suites inline; larger algorithm suites use neighboring
test modules. Matching benchmarks and their shared native/wasm fixtures live in
`crates/core/benches`.
All four text-entry forms produce a single `Problem`:

```rust
struct Problem {
    graph_class: GraphClass,
    input: Input,
    output: ConstraintPair,
    mapping: OutputMapping,
}
```

`Input` is `Trivial` or `Explicit(ConstraintPair)`. Trivial input permits a single
input configuration at every degree in the graph class, without allocating a
dummy label table or storing those configurations. `OutputMapping` is
`Unrestricted` or `Restricted(Vec<LabelSet>)`; restricted entries are indexed by
input ID (one implicit entry for trivial input). Plain entry uses trivial input
and an unrestricted mapping. Independent entry uses explicit input and an
unrestricted mapping. Mapped entry supplies both explicitly.

Paired entry is converted once during parsing. Each distinct pair becomes a
separate ordinary output label: `(a,X)` and `(b,X)` remain different labels even
though their original output components agree. The input promise is the original
input projection. The mapping permits input `a` to emit only labels originating
from pairs whose input component was `a`. Parts and exponents are preserved;
conversion never enumerates their concrete configurations. Output names such as
`a,X` are displayed as `(a,X)` in ordinary constraints. They are ordinary names,
without pair semantics after conversion. The UI displays the converted mapped
problem and witnesses directly. No original-entry metadata is retained; future
transformations can change the same problem that users see.

The constraint containers remain generic so the parser can reuse them with its
private `LabelPair` type. `LabelPair` and `InputOutputConstraintPair` live inside
`parser/paired.rs`, with separate temporary name tables for input and output.
They are discarded after projection and conversion, have no Serde or TypeScript
derives, and are absent from the public model and protocol. Pair-specific tests
live beside them in `parser/paired/tests.rs`. Algorithms on `Problem` use ordinary
output IDs. `ConstraintPair` owns one label table and ordinary active/passive
constraints.
Model fields are private; construction checks invariants. Equality is structural,
and configuration normalization is still deferred.
`CondensedConfiguration` denotes a family of multisets: for example, `AB^2`
represents `AA`, `AB`, and `BB`, rather than one concrete configuration.
`Configuration<L>` stores a single multiset as `(label, multiplicity)` entries;
`A^2 B` has two entries, not three occurrences. Construction merges repeated
labels, sorts entries, discards zero counts, and checks degree overflow. Its generic container also supports temporary parser elements. Algorithms expand occurrences
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
The shared analysis request has `data: { problem, passive_mode }`, where
`passive_mode` is `"general"` or `"maximized"`. Rust defaults an omitted mode to
`"general"`; the typed UI sends it explicitly. Both transports accept the same
request. The GUI currently uses general mode. Maximized mode is available to
callers that can guarantee the passive constraint is maximized; it does not
perform maximization or store a maximization flag on the problem. A UI choice
can be added when maximization is implemented.
The zero-round response contains only the solvability result and any witness;
witness label IDs refer to the submitted problem's label tables.

The check uses RustSAT with packaged CaDiCaL 2.2.1 for native builds and pure Rust
BatSat for wasm. Target-specific dependencies keep CaDiCaL out of wasm builds.
Solvers may produce different valid witnesses and require different numbers of
refinement iterations. The encoding
uses RustSAT literal types and generic solver interfaces; backend construction
and cancellation setup live in `algorithms/sat.rs`. Cardinality bounds use
RustSAT's Ladder (at most one) and Totalizer (larger bounds), sharing a variable
manager and streaming clauses with cancellation checks. Complete native and wasm
measurements are in the [benchmark notes](crates/core/benches/README.md#zero-round-cardinality-encodings). A different incremental
backend can run the same encoding and refinement loop. It selects outputs
and matches them to active configuration parts using capacities, without
enumerating output permutations. Passive degrees up to two are encoded eagerly.
For higher degrees, both modes retain passive input configurations in condensed
form and transform each part into the union of outputs its input choices can
receive. They first try containment in a listed passive configuration, using
label-set inclusion and capacity matching between parts.

In **general mode**, failed containment triggers lazy enumeration of output
multisets and exact membership checks against the whole passive constraint. A
union of listed configurations can cover a family even when no single target
contains it. In **maximized mode**, failed containment proves a violation and
blocks the selected emissions immediately. This requires all maximal passive
configurations to be listed, up to permutation; an incorrect caller guarantee
can give an incorrect decision. Both modes share the SAT search, preparation,
progress, and cancellation. Neither expands passive input families into concrete
input multisets. Active input contexts are still enumerated to define local rules.
Temporary output parts with the same choices and origins are combined; model
configuration normalization remains deferred.

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
The search has no imposed configuration or iteration limit; active input
enumeration and general passive output checking can still be exponential.

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
