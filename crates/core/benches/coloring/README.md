# Coloring solvability investigation

2026-10-14. Investigate incidence edge coloring on the **degree-ten** GUI BMM
example after one RE, then check whether the improvements help other coloring
kinds. These are controlled **prototypes**, not production changes.

## Reproduction and cause

Original active: `M U^9 / P^10`. Original passive: `M UP^9 / U^10`.
One RE produces four labels, with active `A D^9 / B C^9` and passive
`AB BCD^9 / D^10`. There are still only two configurations per side. The size of
the transformed problem is not itself the explanation for the slowdown.

The native maximum search proves palette ten immediately, then stalls while
proving eleven UNSAT. At the 30-second cooperative deadline it has examined
63,215 candidates, added 63,211 passive blocking clauses, and encoded just
three active obligations. The main formula has 349 variables and 63,894 submitted
clauses. Solving accounts for 27.32 seconds, verification for 2.68 seconds.
The initial color-independent/unbounded check takes less than a millisecond.
Cancellation is **not** an UNSAT answer.

Every observed passive blocking clause has ten literals. It excludes the
particular simultaneous emissions of the bad tuple and their supersets; it
does not automatically exclude its other color renamings or related bad tuples.
The lazy loop stops at the first passive placement's first counterexample,
so many candidates revisit the same ten-color subset. CaDiCaL's choices produce
a poor refinement path here. This is not a single long, fully encoded UNSAT
solve, nor a progress-message bottleneck.

Production wasm BatSat proves the same maximum ten in median 389 ms with 1,807
candidates. The same BatSat backend run **natively** takes 295 ms and also uses
1,807 candidates (one screening run). This isolates backend/refinement choices
as the main reason for the huge native/wasm difference on this fixture.
It does not establish that BatSat is generally a better solver.

## Tested improvements

1. **Order whole color behaviors.** Vertex, graph edge and incidence edge coloring
   share one membership solver. Auxiliary color names can be permuted independently
   of original inputs, so sort their complete input/output membership bit vectors
   lexicographically, allowing equal vectors. Neighboring rows use a gate for the
   successor color; disabled colors impose no ordering. Every strategy orbit has
   a sorted representative, including with original input promises/mappings.
   The RE(BMM) maximum uses 112 candidates instead of over 63,000 before timeout.
   Ordering alone takes median 179 ms.
2. **Encode passive degrees at most two eagerly.** Reuse the existing shared
   `passive::visit_forbidden` implementation already used by zero-round search.
   Encode all emission origins, not just those true in the current SAT model.
   On extension only new color placements need encoding. Negative membership
   clauses remain valid when colors are disabled. Higher degrees retain lazy
   refinement. This benefits all local-color kinds and distance-two coloring,
   whose passive degree is always two. No original input-label enumeration or
   explicit large colored problem is introduced.
3. **Group identical output domains independently of provenance.** Current
   temporary parts merge only when output labels **and** emission sources agree.
   Distinct colors consequently keep equal output sets separate: ten `{B,D}`
   slots can enumerate 1,024 assignments representing only eleven multisets.
   Normalize equal output sets for containment and counted enumeration, keeping
   the original source lists separately. For a rejected count distribution,
   distribute its counts back over those original parts and collect their actual
   sources. Thus a clause still proves a realizable forbidden tuple with the
   correct input/color origins. This is exact and requires no maximized constraint.

**Recommend integrating these three together.** Keep CaDiCaL native and BatSat
wasm. All three are temporary kernel changes created by `prepare.py`; model,
protocol, UI, production solver settings and allocator are unchanged.

Simple row sorting is **not** applied to distance-two behaviors: renaming a color
permutes both own and neighbor indices. A valid symmetry scheme there would need
simultaneous row/column permutations and careful palette gating. Eager passive
encoding already gives gains without that extra machinery. Order certificates
for unbounded distance-two solvability retain their existing algorithm.

## Native confirmation

Linux/aarch64, ten logical CPUs, Rust 1.89 release, pinned mimalloc v2 with
option26=0, CaDiCaL 2.2.1. One measured child at a time; no builds or other heavy
jobs overlap measurements. Three confirmation samples, with before/after order
reversed where paired. Table timings cover the complete coloring check, excluding
parsing, RE preparation, independent witness verification, transport and UI.
CNF/candidate/phase instrumentation is identical across compared native kernels.

| Query | Before | Three-change prototype | Before / prototype candidates | Decision |
| --- | ---: | ---: | ---: | --- |
| Incidence, BMM degree 10, maximum | 75.075 ms | 1.079 ms | 1,040 / 13 | Maximum 10 |
| Incidence, RE(BMM), maximum | unfinished at 30 s | **15.875 ms** | 63,215 at cutoff / 112 | Maximum 10 |
| Vertex, RE(BMM), fixed 10 | 273.675 ms | **1.335 ms** | 1,034 / 13 | UNSAT |
| Graph edge, matching degree 10, maximum | 2.643 ms | **0.602 ms** | 30 / 3 | Maximum 10 |
| Vertex, 5-coloring with orientation input, maximum | 8.334 ms | **1.395 ms** | 119 / 9 | Maximum 5 |
| Vertex, mapped list coloring, fixed 3 | 0.379 ms | 0.219 ms | 19 / 1 | UNSAT |
| Distance two, matching degree 3, fixed 4 | 0.548 ms | 0.243 ms | 29 / 5 | SAT |
| Distance two, matching degree 3, fixed 5 | 1.925 ms | **0.667 ms** | 49 / 11 | UNSAT |
| Distance two, 3-coloring degree 3, fixed 4 | 0.480 ms | 0.285 ms | 22 / 4 | UNSAT |
| Distance two, orientation-input 5-coloring, fixed 6 | 54.120 ms | **28.259 ms** | 196 / 38 | UNSAT |

The slow baseline is one 30-second cutoff, not a measured completion time.
The prototype's same cutoff never supplies its mathematical upper bound: it
finishes a genuine UNSAT proof at eleven. Complete process CPU on this case is
30.008 seconds at the baseline cutoff versus median 0.020 seconds for the
completed prototype; this includes preparation and witness verification.
No extra coloring threads or parallel work are introduced.

Raw native measurements are in `results/native.jsonl`, including complete events,
witnesses, clause sizes, phase times, CPU time and binary hashes. The final
grouped variant has an initial screen plus three confirmations; use its **last
three** rows per case for the table. Fixtures and their exact queries are in
`cases.json`. These are workload measurements, not universal speedup guarantees.

Eager pair encoding can be expensive for large color, input, or output domains;
its possible-origin pair count can grow quadratically. Integration should retain
lazy refinement as a fallback when the estimated encoding cost is large, without
imposing a mathematical search limit. Group normalization also has a cost when
all domains differ. These experiments establish benefits on the recorded cases,
not a measured crossover for those larger regimes.

## Wasm confirmation

Matched baseline/prototype adapters use release Rust with `--no-opt` (no wasm-opt),
Node workers and BatSat. Initialization, problem parsing, and RE are outside the
query timer; generic events and final JSON serialization are included. Both sides
use the same flags; production wasm-opt results above are separately recorded.

| Incidence maximum query | Matched baseline | Three-change prototype | Candidates |
| --- | ---: | ---: | ---: |
| BMM degree 10 | 29.998 ms | 13.181 ms | 124 → 36 |
| RE(BMM) | **397.194 ms** | **40.273 ms** | 1,807 → 157 |

Three samples, alternating variant order. Other kinds also agree and improve
in wasm; those are **single-run screens** in `results/wasm-comparison.jsonl`.
For example, orientation-input distance-two checking changes 91 → 34 ms,
and vertex RE(BMM) 273 → 11 ms. Do not report those as multi-sample medians.
Production module runs are in `results/wasm-production.jsonl`.

## Other experiments and tradeoffs

- Learn each passive placement's first failure per candidate: RE(BMM) completes
  in 508 ms / 1,375 candidates. Combine with sorting: 256 ms / 21 candidates,
  **slower despite fewer candidates**, because verification does more work.
  Do not adopt unrestricted all-placement batching as the default: larger palettes
  may have very many placements. A bounded/adaptive clause batch could be tested
  separately if later justified.
- Maximization alone: one native screen still unfinished after 15 seconds
  (~12,000 candidates). Sorting plus maximized-mode checking takes 301 ms /
  1,079 candidates. Fast containment helps each check, but its whole-family
  blocking clauses are broader/weaker than concrete counterexample clauses.
- Precompute the passive maximum, then retain General's concrete blocking clauses:
  sorting takes 62 ms / 112 candidates; add near-homogeneous output probes and
  it takes 32.5 ms. Maximum preparation is about 1.3 ms on this tiny case.
  The grouped general prototype wins at 15.9 ms **without** this extra computation.
  Reusing correct cached artifacts remains a separate integration task, especially
  for restricted mappings that require an input-aware maximum.
- Borrow nearly homogeneous witness probes from generic `covers_with`:
  helps verification in some cases but alone still times out at ten seconds on
  RE(BMM). Sorting/eager/probes gives 161 ms; exact domain grouping is much better.
  No need to adopt the probe variant on the strength of these results.

These screens are preserved alongside the main results; `results/screens.json`
contains the initial BatSat/maximized-mode runs. Fewer candidate strategies is
not itself a runtime or CPU comparison.

## Correctness and organization

The grouped prototype passes **31 native solvability tests**, including the
existing exhaustive plain/input/paired references, explicit colored-input and
neighbor-color oracles, witness checks, graph-class/input preservation, both
passive modes and cancellation. New tests cover RE(BMM)'s degree 2/3/10 palette
boundary and disabling/re-enabling colors in both incremental solvers.

A separate provenance test checks all 64 emission subsets for three original
input parts with multiplicities 1/2/4 and equal output domains. An independent
count oracle forbids exactly three A outputs. Every returned clause is checked
against **all future emission subsets containing its sources**; none admits a
valid future assignment. This catches treating one color/input's source as if
it belonged to another after grouping.

The unchanged wasm coloring test script passes **38 checks** against the grouped
temporary adapter, including streamed bounds and original inputs. All timed SAT
decisions also verify their witnesses outside the timer; baseline/prototypes
agree on every completed query.

For integration, the generic bit-vector comparator belongs with SAT helpers;
eager passive clauses should reuse `solvability/passive.rs`, with color-placement
logic in the two coloring solvers. Grouping/provenance belongs in that same shared
passive checker and already uses generic `Configuration<LabelSet>` normalization.
Do not normalize or alter the displayed immutable model to achieve this speedup.

## Reproduce

```sh
python3 crates/core/benches/coloring/prepare.py /tmp/re3-coloring-selected --variant symmetry-eager-grouped --wasm
python3 crates/core/benches/coloring/run.py --package /tmp/re3-coloring-selected/driver --core /tmp/re3-coloring-selected/core
python3 crates/core/benches/coloring/measure.py --binary /tmp/re3-target-linux/release/re3-coloring-probe --variant symmetry-eager-grouped --case bmm-after --samples 3 --output /tmp/coloring-native.jsonl
cargo +1.89.0 test --release --manifest-path /tmp/re3-coloring-selected/core/Cargo.toml algorithms::solvability
RUSTUP_TOOLCHAIN=1.89.0 wasm-pack build /tmp/re3-coloring-selected/wasm --target web --release --no-opt --out-dir pkg --out-name round_eliminator_3_wasm
RE3_CASE=bmm-after RE3_WASM_MODULE=/tmp/re3-coloring-selected/wasm/pkg/round_eliminator_3_wasm.js node crates/core/benches/coloring/wasm.mjs bmm maximum 30
```

Prepare `baseline` the same way for comparison. Other variants isolate ordering,
batching, eager degree-two clauses, probes, grouping, and native BatSat. Build
everything **before** measuring. `prepare.py` applies diagnostic changes to
temporary source copies; it never edits production. Production algorithm
integration is pending the user's next instruction.
