# Four RE steps on path 3-coloring

2026-11-13. Start with active `A A / B B / C C`, passive `A BC / B C`,
inferred degree two on both sides. Apply RE four times. The output has
8,565 labels, 4,313 active rows and 39 passive rows. All active rows are
concrete: 61 homogeneous rows and 4,252 pairs, each label occurring in one row.

The input variant below attaches `A B` on nodes **and** edges to this final
output problem, with unrestricted input/output permissions. It does not apply
four RE steps to an initially input-bearing problem.

## Fixed-palette investigation and integrated changes

The original native release query for 25 vertex colors was cancelled after
30.05 seconds and 67 candidates. Phase totals: encoding 0.168s, SAT 1.585s,
verification 28.298s. A second, instrumented 15-second run spent 13.72s
enumerating passive outputs, visiting 1,633,769 concrete multisets. Candidates
enabled about 214,093 of 214,125 possible color/output memberships. Constructing
even a two-label concrete multiset scanned large zero-filled count vectors.

Integrated into production:

- Vertex memberships have safe per-input support bounds: sum that input's
  multiplicities over all active input contexts. With `A B`, each input's bound
  is one; multiple contexts and repeated inputs can require larger supports.
- Every vertex color selects active realizations for all original input contexts;
  memberships must be supported by their union. Cardinality bounds alone moved
  the bottleneck into SAT and still timed out at 30s on the plain fixture.
- Shared active SAT encoding avoids matching machinery when all positions have
  identical domains. Concrete binary rows with different position domains use
  direct orientation selectors and indexed choices.
- A reusable generic binary checker unions row coverage with temporary bitsets.
  General passive verification uses it at degree two after containment fails,
  preserving actual input/color source literals. Maximized semantics are unchanged.

The ordinary fixed-25 solver then returned SAT after 107 candidates in 7.140s.
A freshly rebuilt release server, using `/api/stream` and the same parse/four-RE
sequence as the UI, returned SAT in **6.950s**, including request serialization
and streaming. RE preparation is outside those times. Witnesses are independently
checked. These are individual runs, not benchmark medians.

The large `A B` variant remains slower in the general solver: its last 30-second
screen reached 551 candidates without an answer (not UNSAT). Support bounds are
sound for this input promise; they do not guarantee a good SAT trajectory.
Small original-input, repeated-input and multiple-context regressions pass.
Follow-up on 2026-11-14, using the production core solver through the normal
timing driver: fixed 25 with this `A B` promise still had no answer at the
120-second cutoff (2,382 candidates; SAT 94.66s, verification 24.98s,
encoding 0.52s). A separate fixed-5 check also reached its 15-second cutoff
without an answer (1,717 candidates). These are interrupted searches, not
negative solvability results, and were not UI/server transport measurements.
Bounded clause batches and sharing binary failures across color renamings were
also tried; neither justified integration on these screens. A native phase-zero
experiment did not change the original dense trajectory. The backend is unchanged.

Environment: Rust 1.89 release, Apple ARM Linux VM with 10 exposed vCPUs,
CaDiCaL 2.2.1. The ordinary driver/server use the configured mimalloc v2 allocator.
No benchmarks ran concurrently with builds or tests.

## Exact maxima from the independent diagnostic

| Final problem | Exact maximum vertex-color palette |
| --- | ---: |
| No original inputs | **148** |
| Original input `A B` on nodes and edges | **235** |

These are **standalone diagnostic results**, not timings or results produced by
the UI maximum-search algorithm. The diagnostic is not integrated into that
algorithm. It uses RustSAT/CaDiCaL with a graph encoding and ordinary allocation.

Write `f(a)` for the other label in a's unique active row, allowing `f(a)=a`.
Write `P(a,b)` for concrete membership in the original passive union.

- Plain behaviors are active rows `{a,f(a)}`. Two behaviors are compatible when
  all four cross-output pairs satisfy P. Any strategy can shrink each color's
  domain to one active row, so its maximum palette equals this graph's clique number.
- With `A B` inputs, each color chooses an ordered active row: output `a` on A
  and `f(a)` on B. Behaviors a and b are compatible exactly when
  `P(a,f(b)) && P(f(a),b)`. This gives 8,565 candidate behaviors.
- Neither graph has a loop, so repeating a behavior cannot give an unbounded
  certificate. Condensed passive rows build both relations using bitset unions;
  no label-pair expansion is necessary during graph construction.

For the plain case, domination removes 3,904 behaviors. The remaining graph has
61 universal vertices and a 348-vertex component. Exact SAT finds a 148-clique
and rejects 149. A proper 148-coloring of the original behavior graph supplies
an additional, independently checked upper-bound certificate.

The plain maximum yields a 235-color input-aware strategy by using both
orientations of eligible paired behaviors. To reject 236, the diagnostic applies
domination, degree bounds and neighborhood-color bounds, then encodes the remaining
graph. Independent sets give color-class variables for a compact cardinality
bound. The first certification used 2,636 vertices and 512,192 nonedge clauses;
SAT rejected 236 in 0.213s. The plain numerical search took 0.542s.

The retained script reproduced **148 and 235 in 20.24s total**, excluding compilation
and RE preparation but including graph construction, reductions and direct witness
verification. Its final SAT stages took 0.573s and 0.277s. It saves the reduction
trace and solver instances, and aborts on an interrupted exact check. Both largest
strategies were independently checked directly against the original condensed rows.

## Why normal maximum search starts slowly

Maximum search first checks for an unbounded, color-independent strategy. For
this plain snapshot, the specialized zero-round precheck prepares an all-pairs
compatibility table over 4,621 self-compatible labels: **21,353,641 pair checks**.
A 10-second screen stayed in Preparing without testing a palette. With `A B`
inputs, the general zero-round precheck instead eagerly visits **73,359,225**
output-origin pairs while building its SAT constraints. Fixed queries skip this
precheck. Numerical maximum search retains its solver between palettes.

These precheck costs remain to be addressed: consider lazy compatibility in the
plain support search, budgeted eager clauses with exact refinement for inputs,
and the behavior-graph approach for its mathematically applicable cases. Do not
describe the independent graph diagnostic as an integrated replacement.

## Reproduce

Build the normal driver using `../run.py`, then run from the repository root:

```sh
python3 crates/core/benches/coloring/run.py --package /tmp/re3-path-probe --target /tmp/re3-path-target
RE3_DEFINITION=crates/core/benches/coloring/path_re/definition.json RE3_COLORING_KIND=vertex RE3_SAVE_PROBLEM=/tmp/path-re4.json /tmp/re3-path-target/release/re3-coloring-probe 2 4 25 30 general
```

Add `RE3_COLORING_INPUT=crates/core/benches/coloring/path_re/input-ab.json` to
attach the input promise after the RE sequence. Replace `25` with `maximum` to
profile the normal maximum precheck. A watchdog interruption proves no upper bound.

After starting a release server, `server.py --url http://127.0.0.1:8080/api`
reproduces the fixed-25 UI request sequence, using compact JSON like `JSON.stringify`.

Run the independent maximum diagnostic on the plain saved snapshot:

```sh
python3 crates/core/benches/coloring/path_re/maximum.py /tmp/path-re4.json --output /tmp/path-maxima
```

It builds `clique.rs` in a temporary standalone package with RustSAT 0.7.5 and
CaDiCaL 2.2.1. `--solver /path/to/path-clique` reuses a previously built executable.
This diagnostic requires degree two, unrestricted mapping, and concrete active
rows partitioning the alphabet into singleton loops or disjoint pairs. It checks
these preconditions and does not replace the general original-input solver.

`maxima.json` retains verified witnesses. `investigation.jsonl` retains stripped
profile/prototype records without multi-megabyte problem echoes.

Verification after integration: 178 core, 5 server and 3 wasm-adapter tests pass;
39 coloring and 30 stateless-cache checks pass on rebuilt native and wasm
transports. Native release server, wasm bindings and frontend were rebuilt.
