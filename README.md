# Round Eliminator 3

This is a new implementation on the `re3` branch. Start parses and validates
active/passive problem definitions, and the GUI displays the returned definition.
It supports plain problems and all three input variants described below.
Zero-round and coloring checks support all four variants and return strategy witnesses.
Passive label diagrams support output-only, input-output pairs, and input-aware outputs.
Passive maximization supports ordinary outputs and input-aware joint constraints.
Apply RE transforms any of the four problem-entry variants by one elimination step.
Other operations remain listed in [FEATURES.md](FEATURES.md).

## Problems and saved sessions

Each successful **Start** appends a new problem below the editor. Problems keep
independent analysis settings and results. Editing the form or starting another
problem leaves previous problems intact; a parse error also preserves them.
The **×** control closes a problem and cancels its running analyses.
The initial backend is Server on `localhost` and `127.0.0.1`, and WebAssembly on
other hosts; `?backend=server` or `?backend=wasm` overrides this default.
**Close all** does this for every open problem, keeping the editor contents.

Click a numbered problem header to collapse or expand it. Click section headers
to fold individual constraint sides, editors, or analyses. Headers also support
Enter and Space, with disclosure arrows indicating their state.
Collapsed sections within each problem collect in a compact vertical stack on
the right, or below the expanded sections on narrow screens. Expanding a header
returns its section to its original position without restarting computations.
Collapsed computations keep running. **Collapse all**, **Expand all**, and
**Collapse all except current** manage the list; “Current” marks the most recently
created or interacted-with problem. The last imported problem starts as current.
Bulk collapse and close controls are in the top toolbar, before **Import**.
Degree columns use **Deg.**, with the full name available as a tooltip.

**Export** downloads a versioned `round-eliminator-3.re3.json` session containing
the editor, open problem definitions, collapse state, analysis settings, completed
results, and proved coloring bounds. **Import** replaces the current session and
cancels its computations. Imported analyses start with their saved results or
retained bounds; no computation resumes automatically. Version-two sessions keep
one computational state per problem, separate from view controls. Version-one
files and links migrate their existing caches on import.
**Copy link** carries the same session as compressed UTF-8 data in the URL fragment,
without uploading it. Opening the link restores the session with either backend.
If clipboard access is unavailable, the UI offers a selectable link for manual
copying. Large sessions can produce links too long for the destination; use a
file export in that case. Files and links from the old GUI are not compatible.

Browser persistence is in `web/src/session.ts`; it reuses generated Rust types
for the problems, results and bounds. `SessionControls.vue` handles file/link
actions, `ProblemCard.vue` connects each problem to its independent analyses,
and `CollapsibleSection.vue` provides consistent section controls. Runtime keys,
workers, controllers and progress events are not part of the saved data.
Run `npm run test:session` in `web` for file/link round-trip checks.

The root is a virtual Cargo workspace. `crates/core` contains the portable
problem model, parser, and request handler; `crates/server` and `crates/wasm`
are thin delivery adapters. `web` contains the Vue interface.

The page background is white, the editor and numbered problem cards are gray,
and their sections are white. Pale gray headers with blue clickable titles match
the old GUI. The gray cards stay full width; their white sections hug their contents
and share one wrapping flow: constraints and analyses sit beside each other
when space permits, without stretched columns or equal-height panels. Editors and
diagram canvases resize horizontally and vertically; their white sections follow
the new width. The diagram graph automatically refits. Widths are bounded by the
screen, and controls wrap when a diagram is narrowed. Diagrams start with a
compact 256–320 px canvas, depending on the screen width. The default base font
is 16px. Coloring uses about 344 px so its title and selector stay readable while ordinary problems fit
across laptop screens.
Sections wrap when more space is needed. Expanded degree override fields retain
a readable width independent of resizable text areas.
Each problem header shows its active and passive degree sets, including when
the problem is collapsed. Graph class does not occupy a separate section in the
wrapping layout.
Click **New problem** to hide its body while keeping **Start**, problem type, and
backend controls available. Header controls act independently of folding.
**Examples**, graph-class overrides, and notation/sharing help are expandable;
analysis explanations live under **Coloring assumptions** and **Diagram assumptions**.
Editor text and running computations survive folding; only problem and analysis
collapse state belongs in saved sessions, while editor folding remains local.

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
├── algorithms/
│   ├── configurations.rs # generic enumeration, membership, and matching
│   ├── configurations/   # coverage, products, count alignments, and flow matching
│   ├── diagram.rs        # diagram types, methods, and three views
│   ├── diagram/          # parallel relation computation and tests
│   ├── graphs.rs         # SCC condensation and transitive reduction
│   ├── inputs.rs         # original input preparation shared by analyses
│   ├── maximize.rs       # passive maximization preparation and result schema
│   ├── maximize/         # generic pairwise closure and correctness tests
│   ├── round_elimination.rs # universal/existential transformation and side swap
│   ├── round_elimination/   # transformation and independent equivalence tests
│   ├── parallel.rs       # bounded native parallel work; serial wasm fallback
│   ├── sat.rs            # solver setup and generic formula operations
│   └── solvability/       # analyses and strategy-specific shared helpers
│       ├── mod.rs
│       ├── zero_round/
│       ├── coloring/
│       ├── constraint_encoding.rs # shared SAT matching for solvability
│       └── passive.rs
└── api/
    ├── mod.rs             # shared dispatch and JSON execution
    ├── protocol.rs        # request/operation/response/event schema
    ├── state.rs           # client-owned problem and completed-analysis cache
    ├── state_tests.rs     # cache reuse, scope and proof equivalence tests
    └── tests.rs           # API behavior and cancellation tests
```

Use `model::{Problem, LabelId, LabelSet, ...}` for model types and `api::{Request,
Response, ...}` for API types and execution. The server and wasm adapters depend
on this same API; folder organization does not change the JSON schema.

| File | Responsibility |
| --- | --- |
| `crates/core/src/model/labels.rs` | Numeric label IDs, sorted label sets, joint input-output labels, and shared label-name validation. |
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
| `crates/core/src/api/protocol.rs` | Shared state/operation envelope, response packets, progress/cache/proof events, and generated TypeScript types. |
| `crates/core/src/api/state.rs` | Client-owned `ProblemState` and completed analysis cache, separate from the mathematical model. |
| `crates/core/src/computation.rs` | Transport-independent cancellation flag and computation phases. |
| `crates/core/src/algorithms/sat.rs` | RustSAT formula builder, generic cardinality encodings, and target-specific backend setup. |
| `crates/core/src/algorithms/configurations.rs` | Concrete multiset enumeration, count-distribution stepping, condensed containment, and flow matching of offered labels to allowed configurations. |
| `crates/core/src/algorithms/inputs.rs` | Original active input multisets and condensed passive input families, shared by solvability and diagrams. |
| `crates/core/src/algorithms/solvability/constraint_encoding.rs` | Shared SAT matching for solvability, with optional activation gates. |
| `crates/core/src/algorithms/solvability/passive.rs` | Shared verification modes, emission families, containment, exact output checks, and blocking clauses. |
| `crates/core/src/algorithms/solvability/zero_round/mod.rs` | Local-rule types and SAT search with counterexample refinement. |
| `crates/core/src/algorithms/solvability/zero_round/prepare.rs` | Prepare initially empty active rules and their output domains. |
| `crates/core/src/algorithms/solvability/zero_round/verify.rs` | Complete witness verification using the general passive checker. |
| `crates/core/src/algorithms/solvability/coloring/mod.rs` | Coloring types, incremental palette search, cached proof bounds and streamed decisions. |
| `crates/core/src/algorithms/solvability/coloring/reuse.rs` | Restrict palette witnesses, expand order certificates, combine proved bounds and reuse zero-round strategies. |
| `crates/core/src/algorithms/solvability/coloring/encoding.rs` | Incremental SAT on color/input output sets; active and passive counterexample refinement. |
| `crates/core/src/algorithms/solvability/coloring/prepare.rs` | Visit color placements while preserving condensed original passive input choices. |
| `crates/core/src/algorithms/solvability/coloring/verify.rs` | Verify finite and repeatable coloring witnesses independently of SAT. |
| `crates/core/src/algorithms/configurations/coverage.rs` | Remove one label to obtain contexts; test universal coverage by a union of configurations. |
| `crates/core/src/algorithms/configurations/product.rs` | Join condensed input/output constraints through compatible incidence pairs, keeping multiplicities counted. |
| `crates/core/src/algorithms/configurations/alignments.rs` | Visit compatible count matrices with fixed margins, shared by products and maximization. |
| `crates/core/src/algorithms/configurations/matching.rs` | Counted flow matching with a supplied compatibility relation, shared by membership and dominance. |
| `crates/core/src/algorithms/maximize.rs` | Prepare ordinary or joint passive constraints; return a separate complete maximum. |
| `crates/core/src/algorithms/maximize/engine.rs` | Generic algebra, closure scheduling, degree frontiers, and ordered duplicate history. |
| `crates/core/src/algorithms/maximize/combination.rs` | Counted pair combinations, early intersection-exchange pruning, and optional derivations. |
| `crates/core/src/algorithms/maximize/antichain.rs` | Dominance checks, recent-dominator caches, and parallel batch rejection/removal. |
| `crates/core/src/algorithms/configurations/substitution.rs` | Substitute condensed part choices, preserving repetitions and removing impossible configurations. |
| `crates/core/src/algorithms/round_elimination.rs` | Build the set-label alphabet from a complete maximum, apply existential substitution, and swap the graph/input roles. |
| `crates/core/src/algorithms/diagram.rs` | Diagram schema, methods, canonical input integration, and input-aware projection. |
| `crates/core/src/algorithms/diagram/relation.rs` | Compare occurrence profiles and residual languages, with cached degree-one context unions. |
| `crates/core/src/algorithms/graphs.rs` | Library SCC grouping and transitive reduction, preserving isolated vertices. |
| `crates/core/src/algorithms/parallel.rs` | Ordered parallel results and caller-thread progress through Rayon's shared native pool. |

The model, parser, and API each keep their tests in a neighboring `tests.rs` module.
`algorithms::solvability` groups zero-round and coloring checks with their shared
strategy verification and the shared solvability SAT encoding. General constraint
operations, original input preparation, graph operations, and solver support live
directly in `algorithms`, so other analyses and transformations can use them
independently of solvability.
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
from pairs whose input component was `a`. Exact duplicate configurations are
discarded during constraint construction, including duplicates created by the
input projection. Parts and exponents are preserved;
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

## Example problems

The **Examples** buttons load editable definitions for MIS, maximal matching,
bipartite maximal matching, vertex coloring, and edge grabbing, including
orientation and edge-coloring input promises. Ordinary graph examples use
degree-three or degree-five vertices and degree-two edges; bipartite maximal matching uses
degree three on both sides. Degree fields stay blank so Start infers the graph
class from the constraints.

The mapped and paired list-coloring presets describe the same problem: every
vertex has a two-color list, and neighboring vertices must choose different
colors. Presets fill the editors; press **Start** to parse them through either
backend. Their definitions live in `web/src/presets.ts`.

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
The shared request is `Compute { state, operation }`. `ProblemState` contains the
immutable mathematical problem and an `AnalysisCache` of completed results.
`Operation::CheckZeroRound` has only `passive_mode`, defaulting to `general`;
the typed UI sends it explicitly. Rust selects any applicable saved maximum or
proof. The backend is stateless: no problem handles, server sessions, or resident
wasm cache are required. `sendForProblem` in `web/src/api.ts` attaches the full
state and merges incoming entries against the live cache, preserving concurrent
analyses. UI controls specify operations rather than cache dependencies.

Every response is a `ResponsePacket { response, cache }`. The response is the
requested result; cache contains only completed entries newly produced or derived
by this request. Existing state is not echoed. Parsing and transformations return
a mathematical problem; a transformed problem starts with empty caches. A
`cache_update` event publishes cumulative new source entries during computation,
independently of progress and partial coloring bounds. Completed entries survive
subsequent cancellation. IDs always refer to the source snapshot's label tables.

The GUI uses general verification. A saved maximum supplies containment proofs
without another maximization. Output-only maxima can verify ordinary output
families; joint maxima verify input-output families after projecting auxiliary
colors away. Counterexamples retain actual input/color source literals. General
mode retains concrete output blocking clauses instead of weakening them to
whole-family clauses. An explicit `maximized` mode remains available to callers
that guarantee the corresponding original constraint's maximality.

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
Temporary output parts with equal output domains are combined for containment
and multiset enumeration, even when their input/color origins differ. A rejected
multiset is allocated back to the original parts to build a sound blocking clause.
The displayed model is unchanged; configuration normalization remains deferred.

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

`algorithms::solvability::zero_round::verify_strategy` checks witness coverage, active validity,
mapping restrictions, and universal passive validity without a SAT solver.
Tests compare search results against exhaustive strategies on small instances.
The search has no imposed configuration or iteration limit; active input
enumeration and general passive output checking can still be exponential.

While running, the check button becomes an indeterminate progress indicator,
a description of the current phase, and a **Cancel** button. Completed SAT
candidate counts are shown when available; no percentage or completion time is
estimated. Results appear below the constraints. The zero-round strategy explanation
is a collapsed **Reading the strategy** tip, with the table visible. Cancellation
preserves the parsed problem and offers a retry. Editing the form does not change an existing
parsed problem or its analysis; pressing Start appends a separate problem.
Closing a problem or importing a session cancels the removed problems' work.

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

Both transports coalesce progress, cumulative cache updates and coloring-proof snapshots to at
most ten update batches per second, with an immediate final flush. Native work
publishes to latest-value slots without waiting for the stream consumer; a slow
browser cannot fill a progress queue and block the CPU thread. Serialization and
network delivery happen on the async side. Wasm throttles before serialization,
JS callbacks, and worker messages. Short phases and superseded snapshots may be
skipped; independent slots preserve the latest counter, completed source entries,
best proved bounds and witness, and final response
are preserved. Cancellation checks remain independent of this delivery rate.
Retain this snapshot contract for future partial-result types; additive events
would need a separate delivery policy.

## Coloring solvability

The parsed problem has a separate **Check coloring solvability** button. Choose
a fixed palette or search for the maximum palette. The strategy can use both
the given colors and all original incident inputs; it retains their promise,
mapping restrictions, and graph class. Orientation is represented by ordinary
input constraints and needs no separate orientation algorithm.

Completed checks are cached per problem, coloring kind, and palette choice.
Returning to a computed choice displays its result immediately; fixed palettes
with different sizes have separate entries. Files and links preserve the cache,
including results migrated from older sessions. Cancelled or failed checks remain
retryable. Under **Show strategy**, the table is visible while the **Reading the
strategy** explanation stays collapsed until opened.

| Given coloring | Promise |
| --- | --- |
| Vertex | One color at each active node; colors around each passive node are distinct. This is proper graph coloring and strong hypergraph coloring. |
| Graph edge | Colors differ at active nodes and agree across each degree-two passive edge. Other passive degrees are unsupported for this kind. |
| Incidence edge | Colors differ around both active and passive nodes. Supports arbitrary allowed degree sets. |
| Distance-two vertex | A node knows its own and all neighboring colors. Colors in each closed neighborhood are distinct. Requires passive degree 2. Supports fixed and maximum palettes. |

Only locally feasible palettes are queried: at least the maximum passive degree
for vertex coloring, maximum active degree for graph edge coloring, and maximum
degree on either side for incidence edge coloring, with at least one color.
Distance-two coloring requires at least maximum active degree plus one colors.
As with the zero-round check, listed input configurations are local obligations;
global realizability of the input promise is not decided.

SAT variables describe output-set membership for each `(color, original input)`.
For every full active input/color configuration, some allowed active output must
be obtainable from those sets. Passive validity must hold for every possible
combination. This still permits output choices to depend on the full active
input configuration and local ports, rather than fixing one output per label.
Active matching and passive containment/exact checks are shared with zero-round.
The general and maximized modes have the same preconditions described above;
the GUI uses general mode. Original passive input alternatives stay condensed.
Active color placements are checked lazily; no explicit colored input problem is
built. The three local-color solvers lexicographically order complete color
behaviors (over all original inputs), allowing ties, to remove color-renaming
symmetry. The ordering is gated so inactive colors impose no restriction.
Both these solvers and distance-two eagerly encode passive rows of degree at most
two when the estimated work fits a 100,000-origin-tuple budget per query; larger
cases use exact refinement. Cached eager clauses cover earlier palettes, and
extensions add only placements involving new colors. This budget selects an
encoding strategy; it does not limit the search or permitted palette size.
Distance-two does not use the row-ordering symmetry: renaming colors would also
require permuting its neighbor-color columns.

Distance-two witnesses instead index sets by `(own color, neighbor color,
original input)`. At an active node, neighbor colors are distinct and exclude
its own color. Across an edge, the endpoint indices reverse. On trees, the
remaining endpoint contexts can be combined independently, so this compressed
witness retains full local configuration-dependent strategies. Original passive
input choices remain condensed, and both verification modes are supported.
The UI shows both endpoint colors and explains that outputs must be chosen as
a whole valid active configuration, rather than independently at each port.
The implementation is in `coloring/distance_two.rs`, sharing SAT matching and
passive verification with the other checks. It does not support greedy coloring.

For distance-two maximum search, `coloring/order.rs` first checks a strategy using
only whether each neighbor's color is smaller or larger than the node's own.
It retains all original inputs and whole-configuration output choices. This
reduces to an ordinary zero-round problem with two temporary input labels per
original label, condensed active choices and opposite comparisons across each
passive edge. No numerical palette is constructed for this check. The UI displays
the resulting two tables as `Any / Smaller` and `Any / Larger`.

The completeness argument applies the [finite Ramsey theorem](https://math.mit.edu/~fox/MAT307-lecture05.pdf)
to the compact numeric witness. Classify each unordered color pair by both
directional output-set tables, including every original input. There are finitely
many such classifications. A sufficiently large palette contains a homogeneous
subset of size at least maximum active degree plus one (and two for an edge).
Every smaller/larger neighborhood pattern is realized by choosing the center's
rank in this subset. Its constant directional tables therefore supply an order
certificate. Conversely, an order certificate works for every feasible palette.
Under the existing local/tree semantics, failing this check proves that the
numerical maximum is finite; the implementation does not calculate a Ramsey bound.
It then uses the same consecutive-palette search and streamed bounds as the other
coloring kinds, retaining one incremental SAT solver and gating each active
obligation by all colors in that neighborhood. Smaller queries disable memberships
involving colors outside their palette. Exact checks and cancellation apply
throughout, and the order witness has its own non-SAT verifier.

For the three color kinds without neighbor information, a maximum search first
checks for a color-independent zero-round strategy. If
one exists, every locally feasible palette works, and the witness gives a single
repeatable behavior. Otherwise it tries consecutive palettes from the local
minimum until the first UNSAT query proves the maximum. This applies to all four
coloring kinds: large jumps can make a single query far more expensive despite
reducing the number of queries. Each query reuses the same incremental SAT
solver; activation assumptions allow subsequent smaller queries. With finite
labels and bounded degrees, an unbounded palette would contain enough identical
output behaviors to give a color-independent strategy, so this search terminates
without an arbitrary palette cutoff. Large instances can still be expensive.

After each decision, both transports send a `partial_result` event containing
`coloring_bounds`: a proved solvable lower bound with its witness, an inclusive
upper bound when known, and the palette being tested. The UI displays these
during the next query and preserves them after cancellation. Cancellation never
counts as UNSAT. Completed results report a maximum, unbounded solvability, no
feasible palette, or the answer to a fixed-palette check. Witness IDs refer to
the existing problem tables; the response does not echo or reparse the problem.

`algorithms::solvability::coloring::verify_witness` checks the strategy with flow matching and
general passive verification, without SAT. Tests also compare the compact
encoding with independently constructed explicit colored input problems.

### Reusing coloring proofs

Cached results stay scoped to the same problem and coloring kind. Fixed-palette
strategies restrict to smaller feasible palettes; unsolvability extends to larger
ones. Distance-two restriction trims both own-color and neighbor-color indices;
order certificates expand using smaller/larger comparisons. Completed maxima and
unbounded certificates answer related fixed queries without SAT.
Maximum search starts above the largest already proved solvable palette and uses
known upper bounds, testing consecutive unresolved palettes only. Adjacent known
SAT/UNSAT decisions can determine a maximum without another search. A cached
zero-round strategy certifies every coloring kind by ignoring its auxiliary
colors; completed negative coloring results also rule out a zero-round strategy.
The ordinary maximum search's zero-round preflight is retained for future requests.
Interrupted searches retain a proved interval and its witness per coloring kind
in shared analysis state. Retry resumes at the next unknown palette; files and
links retain these proofs. Neither interruption nor an in-flight palette supplies
an upper bound. Current UI controls do not duplicate the mathematical bounds.

Run `npm run test:state -- http://127.0.0.1:8080/api` and `npm run test:client-api`
in `web` to check stateless reuse and concurrent/cancelled cache delivery.

## Passive maximization

Press **Maximize** inside a problem to compute its passive maximum. The action
becomes progress and Cancel while running, then displays the maximized passive
constraint, using input-output pairs for input-aware results. The Maximization
section collapses by default when its configuration count exceeds 3/2 of the
original passive output count; explicit saved fold choices take precedence.
The displayed problem stays unchanged. Each immutable problem snapshot retains
its complete result, including in exported files and links; older sessions start
without a maximum. Cancelled computations remain retryable and provide no maximum.

Unrestricted mappings use ordinary output maximization, independently for each
declared passive degree. Restricted mappings use the condensed relation of valid
input-output pairs: its input projection satisfies the original promise, its
output projection satisfies the passive output constraint, and each pair respects
the mapping. The existing counted product constructs that relation without
enumerating complete input configurations. The original input promise is retained
even when some inputs admit no outputs. Joint maximality is a different guarantee
from output-only maximality.

`algorithms/maximize.rs` prepares snapshots and defines `MaximizationResult`;
`maximize/engine.rs` performs pairwise closure with one union and intersections
elsewhere. Working configurations normalize repeated values and keep multiplicities
counted. `Operations<T>` supplies union, intersection, ordering and admissibility
for a finite ordered algebra; custom operations must satisfy the documented laws.
The optional observer receives accepted derivations without imposing tracking
allocations on ordinary use. Empty values are supported internally for other
algebraic computations; normal problem parts continue to require nonempty sets.

`maximize/combination.rs` caches intersections and rejects dominated two-by-two
exchanges while exploring partial count matrices, before enumerating their
completions. The generic conflict-aware search lives in
`configurations/alignments.rs`. Generation checks the immutable global duplicate
history before local containment and flow work. Dense, untraced alignments also
prune operand choices with the same union; sparse alignments and traced runs
retain their operand order. `maximize/antichain.rs` remembers recently useful
dominators. Its native `antichain/live.rs` workers publish admitted configurations
immediately and mark obsolete ones removed. Other workers scan the newest entries
first and skip removed entries. Containment checks run outside the short list
lock; admission checks every concurrent append before publishing. Final survivors
are compacted after the workers finish. With derivation tracking, admissions remain
ordered so intermediate accepted configurations still have their traces.
The engine visits only pairs involving a new configuration, amortizes scheduling
over bounded batches, and queries large duplicate histories concurrently before
ordered insertion. Native work shares Rayon's bounded pool, with adaptive range
splitting and bounded result batches to amortize scheduling and channel traffic.
Progress reports completed-item counts on the caller once per received batch;
cancellation remains independent of progress. Small work stays serial.
Native generation batches adapt to measured generation cost, targeting about
10 ms per batch and learning from expensive partial batches at round ends.
The size stays between 512 and the worker-scaled cap (at most 4,096), with each
adjustment bounded to a factor of two. This publishes duplicate history earlier
for expensive generation while amortizing dispatch/pruning for cheap generation.
Each degree learns separately. Serial, wasm and derivation-tracking runs retain
their fixed batches; no wasm clock or timing-dependent trace ordering is needed.
Untraced native pruning also adapts its worker count. Summed worker elapsed time
estimates the cost per offered candidate for the next batch, targeting about
12 ms per worker, between two workers and the shared pool size. This is a measured
scheduling heuristic, not an exact runtime prediction or a CPU-overhead guarantee.
Small pruning batches retain ordinary serial insertion.
`maximize/packed.rs` interns alphabets with at most 64 distinct labels into `u64`
masks and 65–128 labels into `u128`. Wider alphabets use four-word masks up to 256
or immutable shared word arrays above 256 when average source set size is at least
the word count; sparse wide sets retain vectors. This density rule selects storage,
not the searched configurations. The masks implement union, intersection and
subset operations. The adapter decodes and sorts
the maximum back into normal label sets; the model and wire format stay unchanged.
Dynamic wide masks use dictionary/varint duplicate history: repeated values are
interned once and configuration keys store exact value IDs/counts. This reduces
memory and matching work without a Hash/Serialize requirement. Other algebras
retain full keys. Key buffers grow beyond their inline capacity without a limit.
Packed generation uses 512-item minimum ranges for degrees whose normalized seed
counts are all one, and 256 otherwise, on large maps to reduce scheduling and channel
traffic; custom algebras retain normal task splitting and the same generic engine.
Wasm uses the same closure serially in its request worker. Generic
matching rejects insufficient compatible capacity before allocating a flow graph.
For up to 128 compatibility cells it caches successful comparisons in a stack
bitmask; larger networks retain the same exact flow procedure. Parent combination
finds useful joins first, rejects impossible component balances before exchange
preparation, and reuses alignment conflicts, margins, counts and traversal frames.
Dense algebras certified by a common admissible meet also share exact generated
keys within each untraced batch and retain covered local rejections in history.
This saves repeated matching at the cost of more history storage. Sparse searches
keep their existing history policy; traced execution preserves ordered admissions.
Both targets retain cancellation and coalesced progress. Scheduling thresholds
and the optional exchange-precomputation cap never limit the mathematical search.

RE, diagrams and solvability checks consume applicable saved maxima automatically.
Exact output diagrams use the fast occurrence test on an output-only maximum.
Exact joint/input-aware diagrams use a joint maximum directly. A joint maximum
never certifies an output-only diagram; cheap joint diagrams continue to lift
output arrows. No analysis implicitly computes a maximum it did not request.

The native server uses pinned mimalloc v2, with option 26 (abandoned reclamation
on free) disabled before starting Tokio, matching the benchmark configuration.
Allocator configuration belongs to the server executable; core and wasm do not
choose an allocator.

Run `npm run test:maximization -- http://127.0.0.1:8080/api` in `web` to check all
presets and edge cases through wasm and the server. Omit the URL for wasm only.
`cargo bench -p round-eliminator-3 --bench maximization` records a native baseline;
compare runs with `RAYON_NUM_THREADS=1` and the normal CPU count.
For measured old/new RE and maximization comparisons, including harder cases,
see [the native benchmark report](crates/core/benches/re_comparison/README.md).
The [review experiment report](crates/core/benches/re_comparison/investigation/review-improvements.md)
records the later alignment, history and wider-packing comparisons.
The [four remaining experiment report](crates/core/benches/re_comparison/investigation/final-four.md)
records flow reuse, wider masks, overlap, compression and task-grain comparisons.
Flow reuse and overlap were tested and left out of production.
The [profiling investigation](crates/core/benches/re_comparison/investigation/README.md)
explains the original regressions, compares optimization trials, and records
the production integration. Performance claims apply to the measured fixtures.

## Apply RE

Each problem header has **Apply RE** (the old **Speedup** operation). It appends a
new problem, leaving the source and its analysis results intact. The new problem
starts with fresh analysis caches and supports further RE steps. A cached source
maximum is reused; otherwise it is computed first and saved as soon as complete.
The action shows progress and Cancel, including when the source card is folded.
Cancelled or failed work appends no problem and can be retried. A source maximum
already completed before cancellation remains reusable.

For unrestricted mappings, each set in the passive maximum becomes a new label.
For restricted mappings, a joint part R contributes its distinct nonempty sets
R(i) = {o | (i,o) belongs to R}. These sets become labels, and R becomes choices
among their IDs, preserving multiplicity. Identical sets share an ID. A new set S
is allowed at input i exactly when S is a subset of the previous allowed outputs.
The existential side replaces an old active part O by all new sets S intersecting
O; if no such set exists, the configuration is removed. Counts stay condensed.

Active and passive roles, declared graph degrees, and input constraints are
swapped. Input names and the full original input promise are preserved, including
admissible inputs with no outputs. Fresh output names are A–Z, a–z, 0–9, then L62,
L63, etc.; names do not recursively expand source sets on repeated steps.
The conversion is zero-round equivalent to unpruned input-aware RE under the
retained input promise, as documented in `round_elimination.rs`. Its tests compare
both directions against an independent exhaustive oracle in 1,024 tiny cases.

`Operation::ApplyRe` receives the same `ProblemState` as every other operation.
Its completed source maximum is published through `cache_update` and retained
in the final response packet, while `response` contains the new problem.
`ProblemTransformations.vue` owns the header controls and cancellation;
`sendForProblem` stores source entries, and `App.vue` appends a new empty-cache card.
Existing session export/import and links retain both cards normally.
Run `npm run test:re -- http://127.0.0.1:8080/api` in `web` to check both backends,
or omit the URL for wasm only.

## Basic fixed point (temporary)

**Basic FP (temporary)** constructs the old Basic candidate and appends it as a
new problem. It computes the default partial output diagram, closes its arrows
transitively, and forms the lattice of all successor-closed subsets, including
the empty set. Original concrete configurations are seeded with their principal
closures. The active side uses intersection as join and reverse inclusion; the
passive side uses union as join and inclusion. Both share the generic maximizer.
Passive values expand to their downward choices, then the old button's cleanup
removes weak active choices, dominated rows and labels appearing on only one side.

The button supports output-independent validity, including independent input
promises and nonregular graph classes, which remain unchanged. Input-dependent
mappings need a separate fixed-point definition, so the temporary button is
hidden for those problems and the API rejects them. Cancel interrupts either
closure or cleanup; closing its source card also cancels the operation. Folding
keeps it running. The resulting problem uses normal import/export/link support.
Set-based label names retain the meaning of the construction, without extra
metadata in the mathematical model.

`fixed_point.rs` orchestrates the construction; `fixed_point/lattice.rs` owns the
finite lattice and its algebra. Generic reachability stays in `graphs.rs`, and
output simplification stays in `simplification.rs`. The request is
`Operation::BasicFixedPoint`, dispatched by both normal adapters. Basic FP retains
its representation-dependent default partial diagram: substituting a stronger
cached exact diagram would change the lattice construction. Run
`npm run test:fixed-point -- http://127.0.0.1:8080/api` to verify both transports.

For maximization and Apply RE, the progress bar counts completed pairs against
all pairs queued for the current closure pass, independently of batch size.
It fills from 0% to 100% and resets for a new pass or phase, with the pass number
displayed. SAT searches keep indeterminate progress because their remaining work
is unknown. Progress still uses the existing coalesced, throttled transport.
RE conversion reports transformed parts against its known total too, including
when a cached maximum lets it skip the pair-closure phase.

## Passive label diagrams

Choose a diagram type and its method, then press **Compute diagram**. Only that
relation and its necessary dependencies are computed; each response contains
one diagram. Changing the choice never starts a computation automatically.
Previously computed choices reappear immediately from a cache belonging to that
problem. Layout changes do not require recomputation.
For independent output validity, input-related choices are hidden when the input
promise makes them equivalent to output-only. On nonregular graphs, an input
label can occur only at certain degrees, so input-output pairs may still convey
different strength. Input-aware outputs are redundant when every degree with
allowed outputs also has allowed inputs. Earlier cached equivalents are projected
to output-only results without recomputing comparisons or SCCs.
All arrows concern passive validity; they do not assert preservation of active
output requirements.

| View | Arrow meaning |
| --- | --- |
| Output-only | Replace one occurrence of A by B in every allowed passive output configuration. Inputs are ignored. |
| Input-output pairs | Replace `(i,A)` by `(i,B)`, keeping input i fixed, in every admissible passive joint configuration. Both pairs must be permitted by the mapping. |
| Input-aware outputs | The pair replacement holds for every input permitting A, and every such input also permits B. |

The input-aware relation is derived from the pair relation, including permissions
which would be lost by intersecting only inputs admitting both outputs. The
input promise is checked locally; no global realizability analysis is performed.
An admitted pair with no admissible passive context has vacuous replacement
arrows to other outputs admitted at the same input. Trivial unrestricted input
reduces both input-related types to the output-only relation.

**Partial** uses the cheap occurrence test; without a cached output maximum,
missing arrows are unknown. **Exact** compares residual context languages.
Both automatically use a saved output-only maximum when available, making the
occurrence test exact and sharing the same cache entry. An earlier partial
result is upgraded on the next computation rather than reused as complete.
Neither choice performs maximization. A joint input-output maximum accelerates
exact input-related diagrams but cannot certify output-only arrows.
For passive degree 2, contexts are single labels and their unions are cached for
set-inclusion checks. Higher degrees use containment, cheap counted
counterexamples, and lazy exact multiset enumeration as needed.
Older saved assumption-based choices migrate to Exact; their results are
recomputed because their completeness relied on an unchecked assumption.

Input-output **Cheap** lifts established output arrows through the mapping;
it can miss arrows even with an exact output-only diagram. Input-output **Exact**
joins the input promise and output constraint using matrices of part counts and
compatible pairs. It keeps condensed sets and repetitions, then applies the same
context comparison, without computing a separate output-only relation. Cheap
input-output checks need output-only arrows but do not reduce or return an extra
diagram. Input-aware checks derive their relation from the pair relation and
reduce only the requested input-aware graph. Large products and union coverage can
still be combinatorial.
No arbitrary configuration or iteration cutoff is imposed.

Native context preparation and row comparisons use Rayon's shared worker pool;
`RAYON_NUM_THREADS` controls its size, otherwise Rayon uses the available CPUs.
Cheap comparisons stay serial when scheduling overhead would dominate; general
exact coverage and large occurrence workloads use workers.
Results are deterministic despite parallel completion order. Progress is reported
on the calling thread, and cancellation is checked in workers and generic
configuration operations. Wasm executes serially in the existing cancellable
browser worker. SAT solver threading is unchanged.

Every result contains the full reflexive relation and SCC membership. The display
groups SCCs and removes transitive arrows between groups using petgraph. A
**Partial** label means missing arrows are unknown. Cytoscape.js provides selection,
dragging, panning and zooming; Dagre provides hierarchical layout.
**Physics** is the default for new diagrams. D3-force supplies local repulsion,
collision avoidance and edge springs. There is no gravity or component packing,
so distant disconnected nodes stay where they are dragged. Nearby nodes from
different components still repel. Settling after dragging preserves the viewport.
**Free** stops automatic positioning and keeps nodes where they are placed.
Switching to Free preserves the current positions; fresh free diagrams start
on a compact grid, and **Reset layout** restores that grid. Layout changes
never recompute the diagram relation.
The visualization library is loaded only when a diagram is displayed.
**Export to Clipboard**, **Download text**, and **Download PNG** export the selected
view's groups and reduced arrows. The full relations remain in exported sessions.
The selected type, layout, methods, cached results, and section collapse state
are saved. Cache keys include only settings affecting the selected relation.
Sessions from before diagram support initialize an empty analysis; earlier
three-diagram results are restored as three cached choices.

`DiagramAnalysis.vue` owns request state; `DiagramView.vue` owns visualization,
selection, layouts and exports; `diagramPhysics.ts` owns the force setup, tested
with `npm run test:physics`. `diagram.ts` formats canonical labels and graph
text; `files.ts` supplies the download helper also used by session export.
`npm run test:diagram -- http://127.0.0.1:8080/api` checks the same diagram fixtures
through wasm and the server; omit the URL for wasm-only checks.

Rust uses `DiagramKind::{OutputOnly, InputOutput, InputAware}` and the protocol
uses `output_only`, `input_output`, and `input_aware`. Session import migrates
earlier names, including cached results, so existing files and links remain usable.

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
`npm run test:presets -- http://127.0.0.1:8080/api` checks every GUI preset
and the equivalence of the mapped/paired examples. Omit the URL for wasm only.
`npm run test:coloring -- http://127.0.0.1:8080/api` checks all coloring kinds,
original input restrictions, both passive modes on maximized fixtures, and
streamed bounds through both transports.
The preset script imports TypeScript directly and requires Node 22.18 or newer.
Build wasm before building the frontend so that `web/dist` contains the latest
wasm artifacts.

## Algorithm experiments (2026-10-14)

The [seven-experiment report](crates/core/benches/algorithm_review/README.md)
records traversal, diagrams, zero-round alternatives, symmetry, lattice scaling
and search resumption. Unique multiset distributions avoid visited-row storage;
exact diagrams share lazy residual contexts and coverage for equal profiles,
retaining eager degree-two comparisons when sharing offers little benefit.
Plain problems with passive degree at most two use an exact compatible-support
search with an explicit stack; problems with inputs retain general SAT.
Basic FP keeps fast small-lattice tables and uses direct operations for larger
lattices. Membership SAT and both distance-two symmetry prototypes were verified
but not adopted because their runtime tradeoffs did not justify a general default.
