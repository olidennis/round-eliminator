# Allocation-free label overlap

Measured 2026-10-14 on Linux/aarch64, Rust 1.89 native release. One process,
`RAYON_NUM_THREADS=1`, pinned mimalloc v2 (0.1.52 / sys 0.1.49), option 26 = 0.
No builds or other agent benchmarks ran during measurement.

## Change and scope

`LabelSet::is_disjoint` merges two sorted scans and stops at the first common
label, without allocating. Active matching now uses `!is_disjoint` instead of
constructing an intersection and testing whether it is empty. Generic label
sets, source provenance, counted flow matching, cancellation and serialization
are unchanged. Real intersection operations are retained where the result is
needed. Maximization does not use this compatibility callback.

## Method

`prepare.py` makes two temporary copies of the current core. The before copy
reverts only the active matching callback to the allocating intersection; the
after copy uses production code. Both are built into one benchmark executable.
Preparation and one independent verification of each returned coloring witness
are outside timing; full coloring checks include solver construction and search.
Returned results, including strategies, must agree exactly across the copies.
The kernel benchmarks compile the actual production counted matcher with the
old and new compatibility callbacks; synthetic broad domains exercise both its
stack compatibility cache and its larger-matrix fallback.

Each row is the median of five samples per variant. Samples alternate which
variant runs first and run for at least 50 ms; tiny kernels batch 128 calls per
clock check. The ten full queries reuse the existing coloring investigation
fixtures, covering all four coloring kinds, explicit independent/mapped inputs,
and degree-ten BMM before/after RE. No cached proofs are used. Raw samples are
in [results.jsonl](results.jsonl). These are elapsed-time measurements, not
aggregate CPU, allocator counts, or a universal speedup claim.

## Kernel results

| Case | Before (ns) | After (ns) | Speedup |
| --- | ---: | ---: | ---: |
| overlap: singleton equal | 12.0 | 1.4 | 8.60× |
| overlap: singleton disjoint | 5.5 | 1.5 | 3.57× |
| overlap: 128 labels, first overlap | 149.1 | 1.4 | 105.76× |
| overlap: 128 labels, last overlap | 125.1 | 130.2 | 0.96× |
| overlap: 128 labels, interleaved disjoint | 254.7 | 207.2 | 1.23× |
| overlap: 128 labels, separated | 124.0 | 128.2 | 0.97× |
| matching: 8 sparse positions | 1793.6 | 1334.1 | 1.34× |
| matching: 8 broad positions | 11317.5 | 2235.8 | 5.06× |
| matching: 17 broad positions, uncached matrix | 91210.3 | 9530.0 | 9.57× |
| matching: 8 broad disjoint positions | 2047.5 | 1651.6 | 1.24× |

## Full coloring queries

| Case | Before (ms) | After (ms) | Speedup |
| --- | ---: | ---: | ---: |
| bmm-after | 14.994 | 14.782 | 1.01× |
| bmm-before | 0.816 | 0.808 | 1.01× |
| bmm-vertex | 0.979 | 0.958 | 1.02× |
| coloring-distance | 0.138 | 0.139 | 0.99× |
| list-vertex | 0.108 | 0.108 | 1.00× |
| matching-distance-five | 0.436 | 0.431 | 1.01× |
| matching-distance-four | 0.117 | 0.117 | 1.00× |
| matching-edges | 0.337 | 0.336 | 1.00× |
| orientation-distance | 27.496 | 27.598 | 1.00× |
| orientation-vertex | 1.062 | 1.060 | 1.00× |

## Decision

Keep the change. It is a small generic operation in the existing label-set
module, eliminates temporary compatibility intersections and improves the
matching fixtures by 1.24–9.57×. Full coloring medians differ by approximately
−1% to +2%: these fixtures do not establish a meaningful end-to-end gain.
Some long scans with a late/no overlap are roughly unchanged or a few percent
slower in the primitive benchmark; early overlap and allocation removal account
for the large kernel gains. No large application speedup is claimed.

## Verification

178 Rust workspace tests pass, including the independent exhaustive set-operation
oracle. Native/wasm adapters and frontend rebuilt; 39 coloring fixtures agree
across server and WebAssembly. No production benchmark flags were introduced.

## Reproduce

From the project root:

```sh
python3 crates/core/benches/label_overlap/prepare.py /tmp/re3-label-overlap --build
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/label-overlap-comparison
```

Optional `kernel` or `coloring` argument selects a subset. Build first, then run
one benchmark process at a time. Fresh before/after copies use current source;
historical measurements above remain the recorded 2026-10-14 run.
