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

### Completed entry and display controls

| Feature | Short description |
| --- | --- |
| No input | Enter an ordinary output problem with trivial input. |
| Input with independent output validity | Enter separate input and output constraints with an unrestricted mapping. |
| Input/output pairs | Enter condensed sets of pairs and convert them to the canonical mapped representation. |
| Input-to-output mapping | Enter input/output constraints and the allowed output labels for each input label. |
| Graph class | Specify possible degrees on each side, or infer them when the fields are blank. |
| Run on: Server / WebAssembly | Choose either backend for parsing and analyses through the same API. |
| Constraints and label counts | Show input/output constraints by degree, label counts, and excluded degrees, without a repeated problem-card title. |
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
| List coloring (mapped) | Proper 3-coloring subject to input-dependent lists, using the mapped entry form. |
| List coloring (paired) | The equivalent list-coloring example using input/output pairs. |

### Completed analysis controls and results

| Feature | Short description |
| --- | --- |
| Check zero-round solvability | Decide deterministic active-side zero-round solvability, with or without original inputs. |
| Coloring (vertex / strong hypergraph) | Check strategies using the given vertex color and original inputs; colors differ around each passive node. |
| Edge Coloring Solvability (graph edges) | Check strategies given matching endpoint colors on degree-two passive edges, distinct at active nodes. |
| Edge Coloring Solvability (incidence edges) | Check strategies given incidence colors distinct at both active and passive nodes. |
| Distance-two vertex coloring | Check strategies using own and neighboring colors plus original inputs; requires passive degree 2. |
| Orientation solvability / coloring with orientation | Use an orientation as an ordinary input promise in the zero-round or coloring check. |
| Check a given palette | Decide solvability for a user-specified number of colors. |
| Find the maximum palette | Find the largest solvable palette, or certify unbounded solvability or that no feasible palette works. |
| Zero-round result and strategy | Display solvable/unsolvable status and outputs for each active input configuration in occurrence order. |
| Coloring result / Show strategy | Display the result and output-set certificate, using whole active configurations to choose outputs. |
| Unbounded coloring certificates | Show repeatable behavior, or smaller/larger neighbor-color behavior for distance-two coloring. |
| Partial coloring results | Stream the current palette and proved bounds, retaining the latest witness and bounds on cancellation. |
| Computation progress | Display the phase and number of candidate strategies examined, with an indeterminate progress indicator. |
| Cancel / Retry | Interrupt an analysis and allow a fresh attempt on the same parsed problem, on either backend. |
| Analysis errors | Display failures inside the relevant problem's analysis section. |

### Completed history and sharing features

All entries in this table are **implemented**. Old button names are included
to make their correspondence with the new UI explicit.

| Feature | Short description |
| --- | --- |
| Start / multiple problems | Append each parsed problem without replacing existing problems. |
| Close / Clear (now Close all) | Remove one or all displayed problems and cancel their analyses, retaining the editor. |
| Collapse / Expand | Collapse whole problems, individual sections, or constraint sides without stopping computations. |
| Bulk collapse controls | Collapse all, expand all, or keep only the current problem open. |
| Current problem | Mark the most recently created or interacted-with problem for bulk collapse controls. |
| Save (now Export) | Download the editor, open problems, collapse states, analysis settings/results, and proved bounds as a session file. |
| Load (now Import) | Restore a session file, replacing the current session and cancelling its computations. |
| Export Link To Clipboard (now Copy link) | Copy a compressed session link, with manual copying available if clipboard access fails; opening it restores the shared session. |

Running computations are not resumed on import. The versioned RE3 files and
links differ from the old GUI's `.re` files and links.

## Things to think about in the future

The current representation supports the implemented parsing and display features.
Revisit these design considerations as algorithms and transformations are added;
they do not require immediate changes. Configuration normalization remains
explicitly deferred at the user's request.

| Topic | Short description |
| --- | --- |
| Normalize condensed configurations | Merge identical parts and sort parts. Equality currently compares stored structure; this normalization will not decide all semantic equivalences. |
| Benchmark label-set storage | Compare the current sorted-vector storage with bitsets on representative algorithm workloads before optimizing it. |
| Preserve label-set serialization | If storage changes, explicitly preserve the JSON array shape and generated TypeScript type instead of deriving them from the new storage. |
| Support long computations | Native searches have bounded concurrency and cancellable progress streams; wasm uses cancellable per-request workers. Consider worker reuse only if startup measurements justify it. |
| Optimize zero-round search | Benchmark the shared SAT search and counterexample checker on large instances before adding specialized paths or stronger refinements. |
| Optimize coloring search | Benchmark lazy color-placement checks and incremental SAT refinements before adding symmetry breaking or alternate encodings. |
| Evaluate parallel native SAT solvers | Deferred by the user. Benchmark a candidate such as CryptoMiniSat against CaDiCaL before integrating it; require incremental assumptions and cancellation, retaining RustSAT encodings and the wasm backend. |
| GPU computation | A possible future optimization requested by the user; select kernels only after profiling identifies suitable work. |
| Greedy distance-two coloring | Characterize compatible local views under the Grundy promise before choosing an exact strategy representation. |
| Expose maximized verification in the UI | Both verification modes are implemented and benchmarked. Once maximization exists, offer direct checking versus maximizing first, including that preparation cost. |
| Cancellation within a flow call | Petgraph has no native interruption hook inside maximum flow. Cancellation is checked during construction and around each call; revisit if large networks cause noticeable delays. |
| Global realizability of input promises | The zero-round check uses every listed active input configuration as a local obligation. Consider whether a future mode should first exclude configurations unrealizable on globally valid input-labeled graphs. |
| Preserve input promises in transformations | Keep the graph class and explicit input promise independent of changes to outputs or the mapping; recomputing either can silently narrow the problem domain. |

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
| Speedup | Apply one round-elimination step to a problem. |
| Speedup with Star Relaxation | Apply speedup using one-coordinate-star relaxation in the universal phase. |
| Inverse Speedup | Apply the inverse round-elimination transformation. |
| Maximize | Maximize the passive constraint, then recompute derived information. |
| Full Diagram | Compute the complete label-strength diagram without displaying a maximized passive side. |
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

| Feature | Short description |
| --- | --- |
| Partial Fixpointing | Restrict fixed-point generation to a selected subset of labels. |
| Only determine triviality | Check triviality during fixed-point search without constructing the full result. |
| From diagram selection (Fixed Point) | Select partial-fixpoint labels from the strength diagram. |
| Basic | Generate a fixed-point candidate using the default diagram. |
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
| Strength diagram | Draw directed label relations and allow node and edge selection. |
| Physics / Hierarchical | Toggle force layout and hierarchical diagram layout. |
| Export to Clipboard (diagram) | Copy strength-diagram nodes and arrows as text; separate from the implemented session export and Copy link. |
| Reversible-edge report | Show search statistics, verified alternatives, recipes, and mappings. |
| Action and sequence history | Display each transformation and nested automatic-search sequence. |
| Computation warnings | Display diagnostics beyond the parse errors already supported. |
