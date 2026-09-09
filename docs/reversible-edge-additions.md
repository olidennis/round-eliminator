# Logstar-reversible edge additions

The native GUI has a **Logstar Reversible Edge Additions** button beside the
existing reversible-merge buttons. It leaves those features and Loop unchanged.
The search adds allowed unordered label pairs to the edge constraint, including
same-label pairs such as `A A`. It does not add graph edges or merge labels.
The original node constraint and label names are preserved exactly.

Choose a time limit (default 60 seconds). Verified results appear incrementally
in one result card. Each row is an alternative: individually reversible pairs
must not be combined unless their union is itself verified. **Verify and apply**
reconstructs and rechecks the certificate on the server before returning the
relaxed problem. **Show reverse mapping** displays all annotated node contexts
and their ordered output occurrences. **Copy certificate** exports a replayable
JSON bundle. STOP retains results already verified. The Workers field controls
independent simultaneous attempts: 0 selects up to four available cores, 1 is
sequential, and explicit values up to 32 are accepted. This count applies to the
ordinary search targeting P. One additional independent worker prepares and
searches against RE²(P); it does not take a worker away from the ordinary search.
Each attempt uses the existing single-threaded MiniSat; this is a parallel
portfolio, not a new solver.

Native builds use a [locally patched CPU-time library](../vendor/cpu-time/LOCAL-PATCH.md)
for MiniSat's statistics. Regressing CPU-clock readings are clamped to zero
elapsed CPU time instead of panicking after a solve. Search deadlines still use
`std::time::Instant`; SAT results, verification and parallelism are unchanged.

## What is certified

Write the original problem as P=(N,E), and an edge relaxation as Q=(N,E union F).
A certificate gives a fixed preprocessing recipe followed by a node-local map
from the annotated Q back to P. P to Q is already an identity relaxation.

The supported primitives, on simple bounded-degree graphs, are:

* A proper (Delta+1)-vertex coloring of the whole graph. All incidences of a
  vertex carry the same color, and adjacent vertices have distinct colors.
* MIS on the whole graph or on edges selected by specified pairs of **original**
  endpoint labels. In particular, MIS on A-A edges and MIS on B-B edges are
  independent computations; A and B are never merged. All vertices are included
  in each subgraph; isolated vertices must be selected.
* One round of communication revealing the opposite incidence's full annotated
  state. Original labels and all preceding annotations are retained.
* Full-node context: expose the entire current node configuration on each port.
  Following this with communication reveals the neighbor's node configuration,
  rather than only its incident label. Local support pruning removes contexts
  and states that cannot occur in any legal global annotation.
* Oriented maximal matching on a selected subgraph. A matched node marks exactly
  one matched port and identifies itself as the head or tail; a matched edge
  connects opposite roles. No subgraph edge joins two unmatched nodes. A
  bounded-degree line-graph MIS and endpoint IDs compute this annotation.
* Greedy coloring of a selected subgraph, with neighbor colors on its edges.
  A color-c node must have a neighbor of every color below c. Obtain it by
  processing a proper coloring's classes in order and assigning each vertex the
  smallest color unused by its already processed neighbors. This is stronger
  than accepting arbitrary proper colorings. Outside-subgraph ports are marked
  explicitly and cannot witness a lower-color neighbor.
* Priority MIS: process original node-configuration types in a specified order,
  computing an MIS among each class's vertices that remain undominated by earlier
  selections. Every unselected vertex must point to a selected neighbor of an
  earlier or equal priority, not merely any selected neighbor.
* Distance-two ruling sets: compute MIS in the square of the selected subgraph.
  Centers are at distance at least three, and every vertex is within distance
  two of a center. The annotations include distances and neighbor distances;
  in particular, a distance-one vertex has exactly one center neighbor.

Subgraphs include individual allowed pairs, same-label pairs, label supports of
node configurations, and their connected components under label co-occurrence.
The portfolio also combines independent preprocessing stages without merging
their labels.

SAT can additionally synthesize arbitrary unions of edge-label pairs for one,
two, or three MIS stages jointly with the local mapping. Edge clauses are guarded
by the unknown subgraph predicates. Node contexts are conditionally enabled too:
a pointer to a selected neighbor is possible only when some eligible subgraph
edge exists. This avoids requiring mappings for contexts that disappear under
the chosen predicates. The Boolean context circuit is tested against every small
one/two-stage concrete predicate assignment. A successful predicate is decoded, and its
ordinary concrete MIS recipe is reconstructed, solved, and independently checked.
Search placeholders never appear in accepted certificates. Failure of this
bounded synthesis is not a proof against other preprocessing algorithms.

For each MIS stage, a selected node marks every port I. An unselected node
marks exactly one port P (a pointer to a selected neighbor), and all other ports
U. On subgraph edges the allowed role pairs are I-U, I-P, and U-U. Outside that
subgraph any I/U pair is allowed, but P is forbidden. These rules enforce
independence and maximality, not merely an arbitrary independent set. Every MIS
has a valid annotation: each unselected node chooses a selected neighbor.
Different MIS results coexist in the annotated node context and can be used
jointly by the mapping. The annotation construction represents every legal
preprocessing outcome, not a SAT-chosen favorable outcome.

The SAT mapper assigns an original output label to every incidence of every
annotated node context. Its output at an incidence may depend on that whole
context and on the occurrence, not just the original input label. Each context
must map to an original allowed node configuration. For every allowed annotated
edge, **all combinations of the two endpoint contexts and output occurrences**
must give an original allowed edge. A separate checker verifies these conditions
from the decoded table, without relying on SAT's selector variables. Port ties
can be resolved arbitrarily: the edge check covers every such pairing.

Consequently any legal Q labeling can be annotated by the recipe and mapped to
a legal P labeling. For a fixed degree, alphabet, and finite recipe, MIS and
coloring take O(log* n) deterministic rounds; the remaining communication and
local mapping cost constant time. This proves the claimed reverse reduction.
It does not claim to capture every possible O(log* n) reverse reduction.

### Parallel target RE²(P)

Alongside the usual target P, the native search now tries mapping the annotated
edge relaxation Q to RE(RE(P)). The additions are still to **P's edge constraint**;
the input is not replaced by a sped-up problem. Two ordinary speedups restore
the original node/edge degrees. A single speedup, which swaps those roles, is
not an additional target of this feature.

The RE² worker starts independently of the direct workers. It builds the exact
ordinary, **unsimplified** target once, with no star relaxation or GUI
simplifications, and then runs the same preprocessing/mapping portfolio against
it. The target construction reuses the ordinary union/intersection combinator
and speedup constructor, with a bounded single-worker closure. Partial closure
results are discarded. Its deadline is at most ten seconds and never beyond
the shared overall search deadline. Intermediate alphabets are capped at 64,
universal closure constraints at 1,024 rows, and seen closure rows at 100,000.
The other configured expansion and storage limits also apply.

A certificate targeting RE²(P) stores both intermediate problems and their
set-label dictionaries in an optional `target` field. Its mapping outputs are
labels of `target.second`, not original P labels. The GUI explicitly marks
this target and displays both dictionaries; applying it still returns P with
the certified edge additions and preserves P's node constraint and names.
Old certificates without `target` continue to mean a direct mapping to P.

Replay verifies the preprocessing and the mapping to the saved second target,
and independently checks both decoding steps. For each step it reconstructs
the complete existential constraint from the preceding node constraint and the
set-label dictionary. It also enumerates every underlying tuple of each
universal row and checks membership in the preceding edge constraint, without
trusting maximality flags or diagrams. Dictionary domains, nonempty sets,
constraint degrees and multiplicities are checked too. Decoder verification
has a 200,000-tuple limit per step and an interruptible deadline.

These checks establish the constructive reverse reduction: each vertex on the
existential side jointly chooses a legal tuple of underlying labels, and the
universal constraints guarantee that independent choices at neighboring
vertices are compatible. First the original edges decode RE²(P) to RE(P), then
the original nodes decode RE(P) to P. This costs only constant communication,
so the full reverse reduction remains O(log* n). Choosing one underlying label
independently at each port would **not** be the verified decoder.

Both lanes stream through the owner thread, with duplicate addition sets
coalesced into a single result. Direct results are published even while target
construction is running. An RE² construction limit or failure leaves direct
results intact; it is reported as an inconclusive stopped lane, not a negative
proof. STOP/callback unwinding cancels and joins both lanes and their attempt
workers. The overall deadline is shared, not restarted after construction.
Candidate counts now count candidate/target checks; `re2_mapping_attempts`
reports the RE² share of `mapping_attempts`. Native callers can set
`Options.re2 = false` to run only the original search; the GUI enables both by
default. The extra target can increase CPU/memory use and is not guaranteed to
improve every input.

### Sequential two-endpoint repair

Another recipe actively repairs newly allowed pairs instead of merely attaching
annotations. For every possible bad central edge and every possible assignment
at its two endpoints and boundary, the checker asks whether its endpoints can
choose legal node configurations that fix the central edge, keep every currently
good boundary edge good, and keep every other boundary edge legal in Q. This is
an exhaustive local check, not a favorable choice of boundary conditions.

The check factors through sets of feasible output labels at the central port.
It retains inclusion-minimal sets and requires every pair of sets at the two
endpoints to contain an originally allowed edge. Larger sets cannot make this
condition harder. Every actual boundary therefore has a repair, recoverable by
enumerating the finitely many legal output tuples.

Compute a proper coloring of the square of the line graph (a strong edge
coloring), then process its colors sequentially. In one class, the updated
endpoints form an induced matching, so no repair updates another repair's boundary
vertex. No good edge becomes bad. After every class has been processed, all bad
edges are gone. The number of phases is constant for fixed degree, and the
coloring costs O(log* n). The certificate replays this universal repair check
before the final node-local mapping. This rule changes working label assignments
but keeps the alphabet and node constraint; it never merges labels. It is also
why the graph-model assumption above explicitly excludes parallel edges.

## Search policy and limits

The recipe order is deterministic; parallel completion and wall-clock cutoffs
can change which witness is retained. Missing pairs are interleaved across
recipes so every candidate sees cheap attempts early. Every primitive family
is included, followed by priority orders, information exchange, and bounded
combinations. All inequivalent orders of up to five node types are included; for 6–16 types,
bounded rotations of both order directions are tried. Node-support and pair
lists, combinations, and context sizes are bounded; this is not enumeration of
every possible recipe. A candidate stops after its first verified certificate.
Orders that agree on all comparisons between node types joined by a selected
edge are equivalent for priority MIS and are not retried. Priority MIS is also
combined with neighbor-state exchange.
The search then grows up to four deterministic chains of jointly addable sets, rechecking
each proposed union. It does not enumerate every subset or prove maximality.

Defaults are 60 seconds overall, 1.5 seconds per attempt, 128 candidate sets,
4,096 annotated node configurations, 1,024 states, 200,000 SAT variables,
2,000,000 clauses, and 50,000 conflicts per SAT call. Recipes have at most 16
stages. Large Cartesian products and annotation descriptions are bounded too.
Retained mapping tables have conservative text budgets of 2 MB per certificate
and 16 MB per report, so streaming results cannot accumulate unbounded proofs.
Supported inputs currently have node degree 1–6, edge degree 2, and 1–64 labels.
The label cap is a resource guard, not a solver or representation limitation.
Certificate edge-predicate lists support all 2,080 unordered pairs (including
same-label pairs) over 64 labels; the other search and storage limits still apply.
The GUI exposes overall time and workers; the native API exposes the other
principal limits through `reversible_edges::Options`. All workers share the
overall deadline. STOP and callback unwinding cancel and join all attempt workers
and their SAT threads. Two-endpoint repair additionally caps enumeration at
200,000 boundary views.

An unlisted addition is **not certified**, never proved irreversible. Even an
UNSAT mapping query rules out only that particular preprocessing recipe. Partial
reports and bounded attempts are distinguished from completion of the scheduled
search. Explicit tuple expansion can be the bottleneck with many MIS stages;
limits skip such attempts without publishing an unverified result.

## Terminal and API

From the repository root:

```sh
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example reversible_edges -- search problem.txt 60 4 > report.json
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example reversible_edges -- apply report.json 0
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example reversible_edges -- verify copied-gui-certificate.json
```

The native API is `reversible_edges::search(problem, options, events, publish)`
and `reversible_edges::apply(problem, certificate, events)`. The wire requests
are `ReversibleEdges: [problem, options]` and
`ApplyReversibleEdges: [problem, certificate]`; search responses carry cumulative
`ReversibleEdges` reports. The search/apply paths currently require the native
default-feature server; the new button is hidden in WASM mode.

The CLI's optional arguments are time in seconds and worker count. Omit the worker
count, or use 0, for automatic selection.

Tests cover combined independent A/B-subgraph MIS computations (where one MIS
alone does not suffice), an MIS-required reverse certificate, coloring/state
exchange, rejection of forged mappings, JSON/GUI replay, and STOP cleanup.
A negative union regression starts with allowed pairs AB and CD: AC alone and
AD alone have direct reverse mappings, but their union admits an A-C-D triangle
and is correctly rejected for this bipartite target.

Stronger-variant tests enumerate oriented maximal matchings and square-MIS
outcomes on short cycles; check greedy-color and priority-parent restrictions;
replay every new certificate family; and check joint predicate synthesis,
two-endpoint repair (three-coloring succeeds, two-coloring fails), and parallel
versus sequential verified result sets.

## Measurements on the seven-label degree-four example

The expanded portfolio was benchmarked on
`examples/fixpoint_sat/hard_nonexistence.txt`. The same 18,785 attempts, with
1,515 resource-bounded attempts, took 93.700 s with one worker, 47.914 s with two,
and 24.601 s with four on the development machine, without concurrent builds.
That is 1.96x and 1.95x for the two successive core doublings. All three runs
returned zero certificates. These are measurements of a bounded portfolio, not
an impossibility result. Subsequent priority-order deduplication and the addition
of priority-MIS neighbor exchange change the exact attempt count.
Conditional-context synthesis was also strengthened after these measurements.
With those changes and priority-MIS neighbor exchange, a later run completed
12,055 attempts (2,917 resource-bounded) and still returned no certificate for
this example. The unsuccessful variants remain enabled for other inputs.
