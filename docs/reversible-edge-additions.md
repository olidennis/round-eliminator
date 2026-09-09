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
JSON bundle. STOP retains results already verified.

## What is certified

Write the original problem as P=(N,E), and an edge relaxation as Q=(N,E union F).
A certificate gives a fixed preprocessing recipe followed by a node-local map
from the annotated Q back to P. P to Q is already an identity relaxation.

The supported primitives, on loopless bounded-degree graphs, are:

* A proper (Delta+1)-vertex coloring of the whole graph. All incidences of a
  vertex carry the same color, and adjacent vertices have distinct colors.
* MIS on the whole graph or on edges selected by specified pairs of **original**
  endpoint labels. In particular, MIS on A-A edges and MIS on B-B edges are
  independent computations; A and B are never merged. All vertices are included
  in each subgraph; isolated vertices must be selected.
* One round of communication revealing the opposite incidence's full annotated
  state. Original labels and all preceding annotations are retained.

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

## Search policy and limits

The search is deterministic apart from wall-clock cutoffs. Missing pairs are
tested breadth-first through recipes: direct mapping, proper coloring, whole-graph
MIS, individual label/pair MIS stages, joint per-label MIS stages, and versions
with a final state exchange. Original-label pair predicates are used throughout.
It then grows up to four deterministic chains of jointly addable sets, rechecking
each proposed union. It does not enumerate every subset or prove maximality.

Defaults are 60 seconds overall, 1.5 seconds per attempt, 128 candidate sets,
4,096 annotated node configurations, 1,024 states, 200,000 SAT variables,
2,000,000 clauses, and 50,000 conflicts per SAT call. Recipes have at most 16
stages. Large Cartesian products and annotation descriptions are bounded too.
Retained mapping tables have conservative text budgets of 2 MB per certificate
and 16 MB per report, so streaming results cannot accumulate unbounded proofs.
Supported inputs currently have node degree 1–6, edge degree 2, and 1–32 labels.
The GUI exposes the overall time; the native API exposes the other principal
limits through `reversible_edges::Options`. Synthesis uses the existing MiniSat
dependency, not a new solver. SAT solves are interruptible and joined on STOP.

An unlisted addition is **not certified**, never proved irreversible. Even an
UNSAT mapping query rules out only that particular preprocessing recipe. Partial
reports and bounded attempts are distinguished from completion of the scheduled
search. Explicit tuple expansion can be the bottleneck with many MIS stages;
limits skip such attempts without publishing an unverified result.

## Terminal and API

From the repository root:

```sh
cargo run --release --manifest-path round-eliminator-lib/Cargo.toml \
  --example reversible_edges -- search problem.txt 60 > report.json
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

Tests cover combined independent A/B-subgraph MIS computations (where one MIS
alone does not suffice), an MIS-required reverse certificate, coloring/state
exchange, rejection of forged mappings, JSON/GUI replay, and STOP cleanup.
A negative union regression starts with allowed pairs AB and CD: AC alone and
AD alone have direct reverse mappings, but their union admits an A-C-D triangle
and is correctly rejected for this bipartite target.
