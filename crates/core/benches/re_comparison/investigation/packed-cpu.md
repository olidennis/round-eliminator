# Keeping the large fixture below two seconds with less extra CPU

This is the historical serial-pruning prototype. The subsequent
[live parallel pruning comparison](live-pruning.md) recovers parallel pruning
and integrates a measured scheduling policy with packed sets.

2026-10-13, native Linux/aarch64 VM, ten logical CPUs, Rust 1.89.0 release,
configured mimalloc v2 with option 26 = 0. These are temporary prototypes;
production performance policies and model storage were not changed.

The requested progress bar and Basic FP button were completed and verified first.
The target here is the fixed second maximization of the old large `test_all()`
fixture: **elapsed <= 2 seconds and parallel CPU overhead <= 30%**, relative to
the same representation's fully serial execution. Parsing, UI and full RE
conversion are outside kernel elapsed time. Aggregate CPU includes process setup
and teardown. Every run checks its full canonical maximum against the earlier
reference, not only its 153 retained configuration count.

## A prototype meets the target

Three-sample medians, reversing policy order for the middle sample. One full
maximum per process, no concurrent build/test/CPU work:

| Policy | Elapsed | Aggregate CPU | Extra CPU over its own serial version |
| --- | ---: | ---: | ---: |
| Current production (previous measurements) | 2.000 s | 10.060 s | 4.258 s (+73.4%) |
| Packed sets, ordinary parallel generation/pruning | 1.199 s | 5.946 s | 2.625 s (+79.0%) |
| Packed sets, serial pruning, generation grain 128 | 1.930 s | 4.412 s | 1.090 s (+32.8%) |
| **Packed sets, serial pruning, generation grain 256** | **1.910 s** | **4.290 s** | **0.969 s (+29.2%)** |
| Packed sets, fully serial | 3.319 s | 3.321 s | 0 |

The packed serial baseline is newly measured with the same generic engine and
default serial batching. Current production's previous serial CPU is 5.803 s;
do not compare packed parallel CPU against that slower representation to calculate
overhead. The recommended prototype saves **57.4% total CPU** versus the previous
production parallel result while staying under two seconds. Absolute additional
CPU drops from 4.258 to 0.969 seconds. Individual selected-policy runs span
1.909–1.917 seconds and 4.287–4.303 CPU seconds.

## What changes and why

1. **Pack small label alphabets into `u64`.** This fixture uses 58 distinct labels.
   Intern their sorted IDs to bit positions. Union/intersection/subset checks then
   use bit operations; working configurations clone scalar values rather than
   allocating vectors for every intermediate set. Convert back at the boundary
   and sort decoded configurations into the usual canonical order. The same
   generic closure, conflict search and petgraph matcher are reused. This is an
   internal representation, not a change to the problem model. A constraint with
   more than 64 distinct labels takes the existing path; 64 is not a search limit.

2. **Amortize cheap generation with larger Rayon ranges.** On indexed maps with
   at least 512 entries, the selected experiment uses a 256-item minimum and
   maximum range. Smaller maps retain normal scheduling. Generation batches and
   global history boundaries remain separate and bounded, with normal adaptive
   batching. This reduces result messages, channel synchronization and worker
   scheduling; cancellation still runs within each pair and matrix search.

3. **Use ordered serial global pruning in this regime.** It immediately learns
   from admitted candidates and removes obsolete configurations. The parallel
   version compares frozen snapshots, separate dominator caches and new peers,
   doing additional containment checks. Before packing, serial global pruning
   was too slow to meet the time budget. Faster set operations now make this
   choice affordable, while generation continues to run in parallel.

The first isolated trials illustrate the tradeoff: packed sets plus parallel
pruning with four or six pruning chunks used roughly 5.5–5.6 CPU seconds,
still too much against a 3.34-second serial baseline. Packed serial pruning with
ordinary generation scheduling took 2.102 seconds and 4.892 CPU seconds. Larger
generation ranges bring both elapsed and CPU below the requested limits.

## Scope of the recommendation

This establishes a suitable policy for the requested fixture, rather than a
globally optimal scheduling rule. Coarse ranges can hurt irregular expensive
generation, and serial pruning can hurt other algebras, notably Basic. A future
integration should select this regime using measured workload cost or expose an
explicit CPU-saving mode, retain the current alternatives where appropriate,
and compare matching/Basic before changing shared scheduling defaults.
No new SAT backend, custom matching algorithm, allocator, shared mutable history,
public model format or input semantics is required.

## Records and reproduction

- [packed-efficient-large-10.jsonl](packed-efficient-large-10.jsonl): the three
  repeated parallel policies in the main table.
- [packed-efficient-large-1.jsonl](packed-efficient-large-1.jsonl): repeated fully
  serial packed reference.
- `packed-large-{10,1}.jsonl`: first packed/pruning-worker trials.
- `packed-grain-large-{10,1}.jsonl`: isolated range/pruning comparisons.
- Each `.results/` directory retains complete canonical outputs.
- Current production references are `adaptive-partial-large.jsonl` and the
  new-serial rows of `integrated-test-all-second-results.jsonl`.

```sh
python3 crates/core/benches/re_comparison/investigate_packed.py \
  /tmp/re3-packed-phases --build
python3 crates/core/benches/re_comparison/diagnose_current.py \
  --binary /tmp/re3-target-linux/release/re-packed-phases --case large \
  --workers 10 --samples 3 \
  --variants packed packed-serial-pruning-grain128 packed-serial-pruning-grain256 \
  --output /tmp/re3-packed-efficient-large-10.jsonl
python3 crates/core/benches/re_comparison/diagnose_current.py \
  --binary /tmp/re3-target-linux/release/re-packed-phases --case large \
  --workers 1 --samples 3 --variants packed \
  --output /tmp/re3-packed-efficient-large-1.jsonl
```

The experimental adapter is `packed_sets.rs`; `investigate_packed.py` inserts
it and scheduling controls only into temporary library copies. It is deliberately
kept out of production until its integration policy is chosen.
