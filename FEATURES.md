# Feature inventory

Source: `../round-eliminator/www/gui.js` on `llm-enhanced`, with `api.js` and
`round-eliminator-lib/src/serial.rs` used to identify behavior. Implemented
features are listed first; the remaining feature tables follow the future
design considerations. Buttons with the same label but different behavior
have separate entries.

## Implemented features

Implemented: active/passive text entry, Start, parsing and displaying exact-degree
constraints and label count, reporting parse errors, and canonical label sets
with union, intersection, difference, and subset operations. Starred configurations
are deliberately deferred at the user's request.

Also implemented: all four entry forms, converted to a shared mapped problem.
Plain entry uses trivial input; independent entry uses an unrestricted mapping.
Paired entry becomes ordinary output labels with a restricted mapping and an
explicit input projection. The UI displays the converted definition directly,
without original-entry metadata. Optional active/passive degree lists
define the graph class for all variants, including pairs; omitted lists use input degrees (output degrees for plain
problems). Pair problems derive their promise from the input projection. Checked
model construction, empty constraints, and degree-zero configurations are supported.
Structurally identical configurations are discarded, including duplicates created
by projecting paired inputs; full condensed-configuration normalization is deferred.
Zero-round solvability is implemented for all four variants, with a local-rule
witness. It uses RustSAT with CaDiCaL for native builds and BatSat for wasm,
and general passive checks without a maximization prerequisite. Passive inputs
stay condensed; both modes share containment proofs. General mode checks outputs
exactly after failed containment; an explicit maximized mode can reject immediately
when the caller guarantees maximization. The GUI uses general mode.
The check lives inside the parsed problem, with streamed phase progress, cancellation,
and retry on both server and wasm.
Start parses once; analysis requests reuse the structured problem through direct
Serde deserialization of trusted client snapshots.
Concrete configurations store label multiplicities. Enumeration keeps them
counted, and membership uses capacity-based library flow on native and wasm.

Inclusive exponent ranges (`^5..8`) are expanded during parsing, including zero
occurrences, for all problem variants, without a fixed expansion limit.
The GUI includes editable classical-problem presets for all four entry forms.
Preset degree fields are blank; graph classes are inferred from the constraints.

Coloring solvability is implemented for vertex/strong hypergraph colorings,
graph edge colorings, and incidence edge colorings, using both colors and
original inputs. Fixed palettes, maximum palettes, and unbounded certificates
are supported. Both passive verification modes share zero-round's matching and
passive checks; the GUI uses general mode. Progress and proved palette bounds
stream through both transports, with witnesses retained after cancellation.
Orientation zero-round and orientation coloring solvability are covered by
ordinary orientation input promises, without separate specialized algorithms.
Standard distance-two vertex coloring is supported for passive degree 2,
with each node knowing its own and its neighbors' colors. The check combines
these with all original inputs, keeps passive inputs condensed, and supports
fixed and maximum palettes, order-based unbounded certificates, and both
verification modes on native and wasm. Numerical queries share streamed bounds
and reuse an incremental SAT solver while extending the palette.
Maximum search for all four coloring kinds tries consecutive palettes after its
unbounded certificate check, stopping at the first UNSAT. This avoids costly
overshoots such as querying 20 directly after proving 10 colors solvable.

Diagram computation is implemented for all four entry forms and arbitrary
declared passive degrees. Classical output diagrams support the same occurrence
test as a partial computation or as an exact computation under the caller's
maximization guarantee, plus exact context inclusion without maximization.
Input-output pair diagrams offer cheap lifting of established output arrows
or exact checks against the input promise, output constraint, and mapping.
Input-aware output diagrams derive arrows that hold for every input admitting the source.
Missing arrows in a partial result remain unknown. Native comparisons use a
shared Rayon pool; wasm remains serial in its cancellable request worker.

Passive maximization is implemented as a separate reusable result. Unrestricted
mappings maximize outputs; restricted mappings maximize the condensed joint
input-output relation without enumerating complete input configurations. The
original definition and input promise stay unchanged. Native closure uses a
shared pool for larger workloads; wasm runs in its cancellable request worker.
The engine supports custom ordered operations, optional derivations, and empty
values for algebraic uses while ordinary problem parts remain nonempty.
Measured optimizations include early exchange pruning, capacity rejection before
flow allocation, recent-dominator caches, larger bounded batches, parallel native
global pruning with immediately shared admissions/removals, packed small alphabets,
adaptive pruning concurrency, and concurrent lookups for large duplicate histories. Ordered
derivation admissions and serial wasm execution are retained. Later review
experiments added early useful-join rejection, reusable alignment buffers and
conflicts, component-balance rejection, stack compatibility caching, selective
exact generated-history sharing, and two-word packing up to 128 labels. Dense
wider alphabets now use four-word or dynamic shared masks; sparse wide alphabets
retain vectors. Dynamic masks use exact interned/varint history, and packed
unit-count seeds use coarser tasks. There is no search limit. Flow reuse and
generation/pruning overlap were tested and rejected for the default path. See
`crates/core/benches/re_comparison/investigation/final-four.md`.
Apply RE (old Speedup) uses the source maximum to build a new set-label problem,
with input-aware fibers and the subset mapping when needed, then swaps side roles.
It appends a fresh problem and supports cancellation and reuse of the source cache.

### Completed entry and display controls

| Feature | Short description |
| --- | --- |
| Fluid compact layout | White page, full-width gray cards, content-sized white sections, and a 16px base font; constraints and analyses share one wrapping flow. |
| Collapse / Expand New problem | Click its gray header with a blue title to hide the editor body while retaining Start, type, and backend controls. |
| Resize editors and diagrams | Drag their bottom-right handles in either direction; white sections follow the resized content. Diagrams start compact (256–320 px), wrap without squeezing, and fit narrow screens. |
| Examples / Notation help | Expand example buttons and notation explanations when needed. |
| Graph class: inferred / custom degrees | Expand readable degree override fields; the collapsed summary indicates inferred or explicit degrees. |
| No input | Enter an ordinary output problem with trivial input. |
| Input with independent output validity | Enter separate input and output constraints with an unrestricted mapping. |
| Input/output pairs | Enter condensed sets of pairs and convert them to the canonical mapped representation. |
| Input-to-output mapping | Enter input/output constraints and the allowed output labels for each input label. |
| Graph class | Specify or infer possible degrees on each side; display the resolved degree sets in the problem header, including when collapsed. |
| Run on: Server / WebAssembly | Choose either backend; default to Server on localhost/127.0.0.1 and WebAssembly elsewhere, with explicit URL overrides. |
| Constraints and label counts | Show constraints by degree, label counts, and excluded degrees in numbered problem cards. |
| Allowed outputs for each input | Display the problem's restricted mapping; this differs from a transformation's source-to-result label mapping. |
| Parse errors | Report the affected editor field and line when available, preserving existing problems. |

### Completed example buttons

Examples fill editable text fields; Start performs parsing. Their graph classes
are inferred rather than explicitly specified.

| Feature | Short description |
| --- | --- |
| MIS | Maximal independent set on 3-regular graphs. |
| Maximal matching | Maximal matching on 3-regular graphs. |
| Bipartite maximal matching | Maximal matching with degree 3 on both bipartition sides. |
| 3-coloring (3-regular) | Proper vertex 3-coloring on 3-regular graphs. |
| 3-coloring with outdegree 1 | Proper 3-coloring given an orientation of outdegree 1 on 3-regular graphs. |
| 5-coloring with outdegree 2 (degree 5) | Proper 5-coloring given an orientation of outdegree 2 on 5-regular graphs. |
| Edge grabbing | Select one incident edge per vertex without selecting an edge at both ends. |
| Edge grabbing from sinkless orientation | Edge grabbing given an orientation with an outgoing edge at every vertex. |
| Edge grabbing from sinkless and sourceless orientation | Edge grabbing given an incoming and outgoing edge at every vertex. |
| Sinkless and sourceless orientation from edge grabbing | Produce an orientation with an incoming and outgoing edge using an edge-grabbing input. |
| Edge grabbing from 3-edge coloring | Edge grabbing on 3-regular graphs given a proper 3-edge coloring. |
| Input-aware label replacement (mapped) | Two inputs and three outputs: both input-related diagrams keep A → B but exclude the output-only arrow B → C. |
| List coloring (mapped) | Proper 3-coloring subject to input-dependent lists, using the mapped entry form. |
| List coloring (paired) | The equivalent list-coloring example using input/output pairs. |

### Completed analysis controls and results

| Feature | Short description |
| --- | --- |
| Maximize | Compute, cache and display the passive maximum, including input-output pairs. Collapse by default above 3/2 of the original row count. |
| Saved maximization | Retain the source maximum and reuse it automatically in RE, applicable diagrams and passive solvability checks; keep the displayed problem unchanged. |
| Stateless problem state | Send the problem and completed caches through one shared envelope; Rust selects reusable artifacts, and the client API merges new entries, including across cancellation. |
| Saved-state migration | Version-two files and links separate computational state from UI controls; import version-one sessions with their proofs and caches intact. |
| Apply RE (Speedup) | Apply one round-elimination step and append its result, reusing the source maximum; supports progress, cancellation, and all input variants. |
| Check zero-round solvability | Decide deterministic active-side zero-round solvability; show a short result and table, with strategy help collapsed by default. |
| Coloring assumptions / Diagram assumptions | Expand detailed explanations while keeping analysis controls compact. |
| Coloring (vertex / strong hypergraph) | Check strategies using the given vertex color and original inputs; colors differ around each passive node. |
| Edge Coloring Solvability (graph edges) | Check strategies given matching endpoint colors on degree-two passive edges, distinct at active nodes. |
| Edge Coloring Solvability (incidence edges) | Check strategies given incidence colors distinct at both active and passive nodes. |
| Distance-two vertex coloring | Check strategies using own and neighboring colors plus original inputs; requires passive degree 2. |
| Applicable coloring choices | Hide graph edge and distance-two coloring unless all passive degrees are 2. |
| Orientation solvability / coloring with orientation | Use an orientation as an ordinary input promise in the zero-round or coloring check. |
| Check a given palette | Decide solvability for a user-specified number of colors. |
| Find the maximum palette | Find the largest solvable palette, or certify unbounded solvability or that no feasible palette works. |
| Zero-round result and strategy | Display solvable/unsolvable status and outputs for each active input configuration in occurrence order. |
| Coloring result / Show strategy | Display the result and output-set certificate; its "Reading the strategy" explanation is collapsed by default. |
| Allocation-free active matching | Compare label-set overlap without constructing intersections; both native and wasm share the same matching code. |
| Resume coloring searches | Retain proved bounds and witnesses per kind through cancellation, files and links; retry skips proved numerical palettes. |
| Efficient multiset traversal | Skip duplicate tracking for provably unique distributions while retaining provenance-correct deduplication for overlaps. |
| Specialized zero-round search | Plain problems with passive degree at most two use compatible supports; problems with inputs retain SAT. |
| Shared diagram contexts | Lazily prepare equal occurrence profiles once and reuse their coverage answers; retain cheap eager degree-two comparisons. |
| Cached coloring checks | Reuse exact queries and related palette proofs; start maximum search from known bounds and reuse zero-round certificates. Preserve all entries in files and links. |
| Unbounded coloring certificates | Show repeatable behavior, or smaller/larger neighbor-color behavior for distance-two coloring. |
| Partial coloring results | Stream the current palette and proved bounds, retaining the latest witness and bounds on cancellation. |
| Computation progress | Display the phase and candidate count; both backends coalesce updates to ten batches per second without blocking CPU work on UI consumption. Final results, cumulative new cache entries and the latest proof snapshot are retained independently. |
| Cancel / Retry | Interrupt an analysis and allow a fresh attempt on the same parsed problem, on either backend. |
| Analysis errors | Display failures inside the relevant problem's analysis section. |

### Completed diagram controls and results

| Feature | Short description |
| --- | --- |
| Compute diagram / Full Diagram | Compute only the selected relation and its dependencies; changing the choice requires a button click. |
| Partial output diagram | Use cheap occurrence checks; a cached output maximum makes the result exact. |
| Exact output diagram | Use a cached output maximum when available; otherwise compare residual contexts, with passive-degree-two shortcuts. |
| Cheap input-output diagram | Lift known output arrows at inputs permitting both source and target outputs. |
| Exact input-output diagram | Check fixed-input replacement in admissible joint configurations while retaining condensed sets and counts. |
| Output-only / Input-output pairs / Input-aware outputs | Select output strength, pair vertices, or replacements valid for every admitting input before computing. |
| Reuse computed choices | Show previously computed type/method combinations immediately without another native or wasm request. |
| Applicable diagram choices | Hide redundant diagrams for independent validity; retain distinctions due to input-specific degrees or excluded degrees. |
| SCC grouping | Group mutually reachable labels into one displayed node; retain full membership and relation in the result. |
| Strength diagram | Display the reduced directed relation with node/edge selection, dragging, zooming, and panning. |
| Physics / Hierarchical / Free | Default to Physics with local repulsion and edge springs; distant disconnected nodes remain freely placed. Also offer Dagre hierarchy and Free movement. |
| Fit / Reset layout | Fit the graph to its viewport or recompute the selected layout. |
| Resize diagram / Reading the diagram | Resize the diagram in both directions within the page width; automatically refit it and expand guidance about grouped labels and arrows. |
| Export to Clipboard (diagram) | Copy the current view's equivalence groups and reduced arrows, with a manual-copy fallback. |
| Download text / Download PNG | Export the current diagram as readable graph text or a full-graph image. |
| Nodes and arrows as text | Expand an accessible text representation of the displayed diagram. |
| Diagram progress / Cancel / Retry | Show computation phases and completed comparison rows, cancel, or retry on either backend. |
| Saved diagrams | Include methods, selected type/layout, cached results, and section collapse state in session files and links. |

### Completed history and sharing features

All entries in this table are **implemented**. Old button names are included
to make their correspondence with the new UI explicit.

| Feature | Short description |
| --- | --- |
| Start / multiple problems | Append each parsed problem without replacing existing problems. |
| Close / Clear (now Close all) | Remove one or all displayed problems and cancel their analyses, retaining the editor. |
| Collapse / Expand | Click problem/section headers (or use Enter/Space); disclosure arrows show their state, and folding preserves computations. |
| Grouped collapsed sections | Collect a problem's collapsed sections in a vertical stack on the right, moving below expanded sections on narrow screens. |
| Bulk collapse controls | Collapse all, expand all, or keep only the current problem open; controls and Close all precede Import in the top toolbar. |
| Current problem | Mark the most recently created or interacted-with problem for bulk collapse controls. |
| Save (now Export) | Download the editor, open problems, collapse states, analysis settings/results, and proved bounds as a session file. |
| Load (now Import) | Restore a session file, replacing the current session and cancelling its computations. |
| Sharing help | Expand the explanation of saved sessions, imports, and shared links. |
| Export Link To Clipboard (now Copy link) | Copy a compressed session link, with manual copying available if clipboard access fails; opening it restores the shared session. |

Running computations are not resumed on import. The versioned RE3 files and
links differ from the old GUI's `.re` files and links.

### Completed fixed-point tools (temporary)

| Feature | Short description |
| --- | --- |
| Basic / Basic FP (temporary) | Construct the default-lattice candidate, run the dual closures and cleanup, and append a cancellable new problem on server or wasm. Supports output-independent validity and preserves inputs and graph class. |
| Determinate RE progress | Count pairs against the current closure pass's total, then count resolved parts during RE conversion (including cached maxima). Each stage fills from 0% to 100% and resets for the next pass or phase. |

## Things to think about in the future

Coloring performance investigation completed: color-behavior ordering, eager
passive degree-two clauses and grouping equal output domains with separate origins
help all four coloring kinds on native/wasm, including degree-ten RE(BMM).
The selected changes are now integrated, with exact lazy fallback for large eager
encodings; original input promises, cancellation and both backends are preserved.
See [the measurements](crates/core/benches/coloring/README.md) and `REVIEW.md`.

The current representation supports the implemented parsing and display features.
Revisit these design considerations as algorithms and transformations are added;
they do not require immediate changes. Configuration normalization remains
explicitly deferred at the user's request.

| Topic | Short description |
| --- | --- |
| Normalize condensed configurations | Merge identical parts and sort parts. Equality currently compares stored structure; this normalization will not decide all semantic equivalences. |
| Benchmark label-set storage | Maximization uses u64/u128 masks, density-dependent four-word/dynamic masks and sparse vector fallback; the model retains sorted vectors. Revisit the measured storage heuristics on other workloads. |
| Preserve label-set serialization | If storage changes, explicitly preserve the JSON array shape and generated TypeScript type instead of deriving them from the new storage. |
| Support long computations | Native searches have bounded concurrency and cancellable progress streams; wasm uses cancellable per-request workers. Consider worker reuse only if startup measurements justify it. |
| Optimize zero-round search | Compare membership tables per input against current port variables; plain degree-two support search is integrated; membership SAT was tested without selecting a general replacement, preserving original inputs and witness verification. |
| Optimize coloring search | Color-behavior ordering, affordable eager passive clauses and provenance-preserving output-domain grouping are integrated. Calibrate the eager encoding budget on larger domains, both safe distance-two symmetry prototypes were tested and rejected, and evaluate maximum-search initialization on other cached-query workloads. |
| Share generated duplicates earlier | Implemented for untraced dense algebras certified by a common admissible meet, including covered local rejections. Broad sharing regressed the large fixture; sparse searches keep their previous policy. Monitor history memory on other workloads. |
| Reuse matching/search preparation | Prepared alignments, component rejection and stack compatibility caching are implemented. Flow-network reuse was tested without a meaningful runtime gain and left out; petgraph still performs exact matching. |
| Further Basic FP scaling | Hybrid small tables/direct large operations are integrated. Revisit lookup/memoization policies on other workloads without limiting the lattice. |
| Further maximization performance | Density-dependent multiword masks, compressed dynamic-mask history and unit-count task grains are integrated. Flow reuse and bounded overlap were tested without selecting them. Evaluate the chosen policies on other workloads when requested; all four requested experiments are complete. See `crates/core/benches/re_comparison/investigation/final-four.md`. |
| Custom-algebra maximization | The shared generic engine supports live native pruning, early history and exchange rejection, and adaptive generation batches. Keep ordinary task splitting for custom algebras, ordered tracing, and serial wasm; benchmark other algebras before tuning their scheduling. |
| Native allocator portability | Native server now uses pinned mimalloc v2, option26=0 before Tokio starts, matching the comparison executable. Monitor platform memory/performance; core and wasm remain allocator-independent. |
| Evaluate parallel native SAT solvers | Deferred by the user. Benchmark a candidate such as CryptoMiniSat against CaDiCaL before integrating it; require incremental assumptions and cancellation, retaining RustSAT encodings and the wasm backend. |
| GPU computation | A possible future optimization requested by the user; select kernels only after profiling identifies suitable work. |
| Greedy distance-two coloring | Characterize compatible local views under the Grundy promise before choosing an exact strategy representation. |
| Further proof reuse | Completed proofs and cancelled numerical bounds are reused. Consider reusing failed distance-two order preflight separately. |
| Compact diagram storage | Lazy/grouped contexts are integrated. Consider representing transitive relations by SCCs and their DAG with explicit session migration. |
| Lazy active input checks | Investigate checking condensed active promises and adding only violated SAT obligations; exact coverage remains potentially exponential. |
| Cheap unbounded witness verification | Verify the repeated behavior directly against original constraints instead of enumerating redundant dummy color assignments. |
| UI computation and data handling | Consider a small shared controller composable and raw/shallow immutable problem/result snapshots; preserve independent caches, cancellation, and collapse behavior. |
| Runtime scheduling and progress | Consider mixed SAT/Rayon request budgets and clearer generation/pruning completion accounting; preserve coalescing and cooperative cancellation. |
| Cancellation within a flow call | Petgraph has no native interruption hook inside maximum flow. Cancellation is checked during construction and around each call; revisit if large networks cause noticeable delays. |
| Optimize exact joint diagrams | Benchmark count-matrix products and union coverage on large input promises before adding alternative encodings; condensed preparation can still be combinatorial. |
| Global realizability of input promises | The zero-round check uses every listed active input configuration as a local obligation. Consider whether a future mode should first exclude configurations unrealizable on globally valid input-labeled graphs. |
| Preserve input promises in transformations | Keep the graph class and explicit input promise independent of changes to outputs or the mapping; recomputing either can silently narrow the problem domain. |

The user explicitly deferred the remaining algorithmic investigations on
2026-10-14, including lazy active inputs, direct unbounded verification,
large exact joint diagram products, compact diagram storage, and further tuning.

The [implementation review](REVIEW.md) records source evidence, tradeoffs and a
suggested order for these ideas. Flow domination is stronger than semantic language
coverage: future pruning must preserve the complete maximum guarantee used by
fast verification and Apply RE. No new performance measurements accompanied the review.

## Unimplemented: problem entry, history, and sharing

| Feature | Short description |
| --- | --- |
| Starred configuration notation (deferred) | Allow `*` in a configuration to cover a range of degrees. |
| Edit | Copy a displayed problem back into the input editors. |
| Close warning or error | Remove diagnostics from the displayed history once history diagnostics exist. |
| Close action or sequence | Dismiss a performed-action entry or an entire nested transformation sequence. |
| Close reversible-edge report | Dismiss a reversible-edge search report independently of problems. |
| New / Old / Both | Switch between current labels, source labels, or both in constraints. |
| All / Gen | Switch passive constraint display between all label sets and generators. |

## Unimplemented: operations and analyses

| Feature | Short description |
| --- | --- |
| Speedup with Star Relaxation | Apply speedup using one-coordinate-star relaxation in the universal phase. |
| Inverse Speedup | Apply the inverse round-elimination transformation. |
| Merge (equivalent labels) | Merge labels identified as equivalent in the computed diagram. |
| All Different Labels | Split or distinguish labels that currently denote the same choices. |
| Delta Edge Coloring | Transform labels using an assumed degree-size edge coloring. |
| Speedup+Maximize | Run speedup followed by passive maximization. |
| Speedup+Maximize+Rename | Also rename the resulting labels by generators. |
| Rename by generators | Rename labels using generators of their diagram sets. |
| Rename | Apply names entered in the renaming table. |
| Remove `< >` | Strip generator brackets from proposed label names. |
| Fix Outdegree | Add an orientation of the specified outdegree and recompute related analyses. |
| Apply Marks | Test whether Marks' technique yields a lower bound. |
| Logstar Reversible Relaxations | Find relaxations reversible in O(log* n) rounds. |
| Logstar Reversible Relaxations (old) | Run the earlier reversible-relaxation search. |
| Logstar Reversible Edge Additions | Search for separately verified edges that can be added reversibly. |
| Recursive logstar reversible edge additions | Repeatedly apply verified reversible edge additions. |
| Reversible search limits | Configure the time limit and worker count for the native search. |
| Verify and apply | Recheck a reversible-edge certificate and apply its proposed additions. |
| Show reverse mapping | Expand a certificate's annotated-context to output mapping. |
| Copy certificate | Copy the reversible-edge certificate as JSON. |
| Add Active Predecessors | Add predecessor labels or configurations on the active side. |
| Add Active Pred & Flip | Add active predecessors and flip the problem sides. |
| Remove Trivial Lines | Remove configurations deemed trivial. |

## Unimplemented: manual simplification and hardening

| Feature | Short description |
| --- | --- |
| From diagram selection (Simplify) | Fill the source and target labels from a selected diagram edge. |
| Merge (Simplify) | Merge one selected label into another as a relaxation. |
| Add Arrow | Add a selected relation to the diagram as a simplification. |
| Remove (Harden) | Remove a chosen label, optionally replacing it with predecessors. |
| From diagram selection (Group Simplify) | Select a label group from highlighted diagram nodes. |
| Merge (simplify) | Merge a selected group of labels into a target label. |
| Unlabeled SubDiagram preset button | Fill the subdiagram editor with a built-in merge recipe. |
| Merge (SubDiagram) | Apply the typed subdiagram merge recipe; optionally recompute the diagram. |
| Always Recompute Full Diagram | Choose whether subdiagram merging recomputes the full strength diagram. |
| From diagram selection (Group Harden) | Select labels to harden from highlighted diagram nodes. |
| Remove (harden) | Remove the selected group of labels. |
| Keep (harden) | Keep only the selected group of labels. |
| Replace With Predecessors | Choose whether hardening substitutes predecessor labels. |
| Harden (Critical Sets) | Harden using critical sets, with coloring and search-step options. |
| Relax (Critical Sets) | Relax using critical sets, with coloring and search-step options. |
| Also maximize and rename by generators | Request maximization and generator-based renaming after a critical-set operation. |

## Unimplemented: fixed-point tools

Basic is listed among completed features above. Other fixed-point variants and
controls below remain unimplemented.

| Feature | Short description |
| --- | --- |
| Partial Fixpointing | Restrict fixed-point generation to a selected subset of labels. |
| Only determine triviality | Check triviality during fixed-point search without constructing the full result. |
| From diagram selection (Fixed Point) | Select partial-fixpoint labels from the strength diagram. |
| Loop | Generate a candidate while automatically repairing the default diagram. |
| Make fixed point good | Build a nontrivial lattice-based fixed-point relaxation; native only in the old UI. |
| Generate Default Diagram | Build the default fixed-point diagram, with optional extra arrows. |
| Generate Larger Default Diagram | Build the larger default diagram variant. |
| Generate Fixed Point (custom diagram) | Use a user-edited diagram to generate a fixed point. |
| Find best add arrow | Search for a useful diagram arrow to add. |
| Add from diagram selection (duplication) | Add selected diagram labels to a duplication group. |
| `?` duplication help | Show guidance for choosing labels to duplicate. |
| Delete (duplication group) | Remove one selected duplication group. |
| Track Expressions | Include tracked expressions in the duplication result. |
| Generate Fixed Point (duplication) | Build a candidate after duplicating chosen label groups. |

## Unimplemented: automated bounds, inputs, and duality

| Feature | Short description |
| --- | --- |
| Automatic Lower Bound | Search through problem transformations for a lower bound or fixed point. |
| Automatic Upper Bound | Search for an upper-bound sequence of transformations. |
| Bound search options | Set active/passive coloring assumptions, maximum labels, branching, and maximum steps. |
| Reverse Check | Run the zero-round input check in the reverse direction. |
| Use a SAT solver (input-check option) | Choose between SAT and the old alternative algorithm; current zero-round checks always use SAT. |
| Find smallest obstructing subinput | Find a smallest part of the input promise that already prevents a zero-round solution. |
| Dual | Compute a dual relative to a typed fixed-point problem. |
| Double Dual | Compute the dual twice using the fixed-point procedure. |
| Smallest Dual | Search for a smallest dual relative to the supplied fixed point. |
| Double Dual (diagram route) | Compute a double dual using a fixed-point problem or its diagram. |
| Make Label Different | Apply a selected-label log-star upper-bound transformation. |
| Get Label of Other Side | Reveal or encode the label at the opposite side. |
| MIS | Apply the MIS-based upper-bound transformation. |
| Automatic Logstar Upper Bound | Search for a log-star upper bound with label, depth, and size limits. |
| Just yes/no | Run that search as a Boolean decision instead of returning a sequence. |

## Unimplemented: information displayed for a computed problem

| Feature | Short description |
| --- | --- |
| Mergeable labels | List groups of labels that can be merged. |
| Logstar reversible relaxations | Display reversible label groups and labels that must be removed. |
| Fixed-point procedure status | Report whether a nontrivial fixed-point relaxation was obtained. |
| Marks technique status | Report whether Marks' technique gives a lower bound. |
| Tracked expressions | Display expressions recorded during fixed-point construction. |
| Renaming table | Show each current label and its source-label set. |
| Label mapping table (transformations) | Show each source label's result labels or removal after a transformation; separate from the implemented input-to-output mapping. |
| Searchable label picker | Select labels by current or source name for transformation tools. |
| Reversible-edge report | Show search statistics, verified alternatives, recipes, and mappings. |
| Action and sequence history | Display each transformation and nested automatic-search sequence. |
| Computation warnings | Display diagnostics beyond the parse errors already supported. |
