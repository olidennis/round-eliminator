# Why Basic's relative parallel CPU overhead increased

2026-10-13, native Linux/aarch64 VM, ten logical CPUs, Rust 1.89.0 release,
configured mimalloc v2 with option 26 = 0. Production code is unchanged by
this investigation. The initial analysis measured Basic; the subsequent batch-size
sweep also measures the large ordinary-maximization fixture.

## Percentages versus absolute additional CPU

Reusing the previous production measurements:

| Implementation | Serial CPU | Parallel CPU | Additional CPU | Relative increase |
| --- | ---: | ---: | ---: | ---: |
| Old | 225.250 s | 272.056 s | 46.806 s | 20.8% |
| New, before recent improvements | 331.709 s | 418.500 s | 86.791 s | 26.2% |
| New, integrated improvements | 41.729 s | 66.281 s | 24.553 s | 58.8% |

The percentage is `(parallel CPU - serial CPU) / serial CPU`. The serial
baseline became much smaller: new serial uses 5.40 times less CPU than old serial,
while new parallel uses 4.10 times less CPU than old parallel. Consequently the
relative overhead increased even though its absolute amount fell by **48%**
compared with the old version. The remaining extra work is still real.

## Controlled measurements on current production kernels

`investigate_cpu.py` instruments temporary copies of the actual current modules,
using the existing comparison driver and configured allocator. Its controls change
only generation batch size or global pruning concurrency. Phase-only builds compile
away per-operation counters. Separate counter-enabled runs count mathematical
work. Every run compares complete decoded canonical active/passive results, not
just the 102/14 row counts. No parsing, initialization, UI or RE conversion is
included in kernel elapsed time; total process CPU includes setup and teardown.

The normal serial closure generates **512 parent pairs per batch**; ten workers
generate **4,096**. Global history becomes visible to generation only after the
whole batch finishes. Native Rayon task/result batches are a separate scheduling
mechanism; changing their sizes does not change this history update boundary.

Phase-only measurements (single runs for the two serial controls and serial
pruning; three-sample medians for the two parallel policies):

| Policy | Elapsed | Aggregate CPU |
| --- | ---: | ---: |
| Serial, 512-pair batches (normal) | 41.360 s | 41.358 s |
| Serial, 4,096-pair batches | 48.987 s | 48.983 s |
| Parallel, 4,096-pair batches (normal) | 8.124 s | 65.805 s |
| Parallel, 512-pair batches | **7.613 s** | **55.198 s** |
| Parallel generation, serial pruning, 4,096-pair batches | 13.497 s | 59.477 s |

The three-sample sequence alternates the parallel policies' order. These are
diagnostic runs rather than replacement production benchmark medians. Smaller
parallel batches consume **16% less CPU** and finish **6% sooner** on Basic.
Relative to the serial control, overhead falls from about **59% to 33%**.
Serial pruning saves CPU but substantially increases elapsed time.

### CPU by phase

Approximate process CPU from `/proc/self/stat` at 100 Hz, first phase-only sample:

| Policy | Generation CPU | History/global pruning CPU |
| --- | ---: | ---: |
| Serial, 512 | 33.50 s | 7.82 s |
| Serial, 4,096 | 41.17 s | 7.80 s |
| Parallel, 4,096 | 51.41 s | 14.37 s |
| Parallel, 512 | 42.59 s | 12.54 s |
| Parallel generation, serial pruning, 4,096 | 51.31 s | 8.15 s |

The normal parallel versus serial difference is approximately **17.9 CPU seconds
in generation** and **6.6 seconds in global processing**. Making batches larger
even with no parallel execution adds 7.7 seconds to generation. Holding the
4,096-pair batch size fixed leaves roughly 10.2 seconds of extra generation CPU
under parallel execution. That remainder includes task/runtime and memory/cache
effects; these measurements do not individually attribute those mechanisms.

## Operation counts identify algorithmic redundancy

Degree-seven active saturation, separate counted runs:

| Operation | Serial, 512 | Parallel, 4,096 | Parallel, 512 |
| --- | ---: | ---: | ---: |
| Parent pairs | 255,150 | 255,150 | 255,150 |
| Complete matrices / constructed configurations | 16,423,911 | 16,423,911 | 16,423,911 |
| Early history rejections | 15,636,121 | 15,056,401 | 15,636,121 |
| Local duplicates | 338,222 | 598,687 | 338,222 |
| Local containment checks | 2,571,247 | 6,024,264 | 2,571,247 |
| Pair-local survivors | 189,214 | 320,941 | 189,214 |
| Late global duplicates | 82,925 | 215,792 | 82,925 |
| Global containment checks | 5,862,732 | 9,489,532 | 7,981,487 |
| Flow calls, local plus global | 3,242,974 | 6,004,587 | 3,821,205 |

Normal parallel execution performs **134% more local containment checks**,
**62% more global checks** and **85% more flow calls** than serial execution.
This is not extra mathematical enumeration: parent pairs, complete matrices
and constructed configurations agree exactly.

With 4,096 pairs in flight before history updates, a previously generated
configuration can repeatedly undergo local matching in other pair jobs. Exactly
579,720 fewer candidates are rejected early by history. Reducing the parallel
batch to 512 makes all generation-side counts equal the serial control's counts,
confirming the history-lag cause.

Global pruning adds a second source of redundancy. Its workers compare an
immutable antichain, use separate recent-dominator caches, and test new candidates
against each other. Serial insertion immediately updates the antichain after each
accepted candidate. Counter categories:

| Global checks | Serial, 512 | Parallel, 4,096 | Parallel, 512 |
| --- | ---: | ---: | ---: |
| Recent cache | 626,117 | 629,184 | 662,765 |
| Existing-row scans | 3,821,104 | 4,619,511 | 4,817,587 |
| Old-row removal | 1,415,511 | 2,302,018 | 2,045,388 |
| New-peer checks | Included in ordered insertion | 1,938,819 | 455,747 |

The serial-pruning control at 4,096 pairs keeps generation counts equal to normal
parallel execution but reduces global checks to **5,834,805** and total flow calls
to **4,992,624**. Its global CPU drops from about 14.4 to 8.2 seconds. Existing-row
scan/cache changes and temporary admission order mean individual categories need
not shrink uniformly when changing batch size.

## Why the old percentage is a different comparison

The old generator publishes constructed configurations immediately to a shared
`DashSet`, making duplicate information available during concurrent generation.
Native pruning also reads a live append-only antichain with removal flags and
recent-dominator caches, rather than a frozen batch snapshot.

The old fully serial measurement compiles its original wasm loop natively.
That loop first reduces candidates for each first parent, then inserts them into
the global constraint, using the ordinary inclusion scan without the native
pruning workers' caches. Thus old serial and old parallel are different pruning
implementations, not a single identical algorithm with threads toggled. The 21%
figure does not isolate thread/runtime overhead. The precise contribution of
those old pruning differences was not separately measured in this follow-up;
no old serial or allocator sweep was rerun.

## Next improvement to consider

Smaller generation batches are a measured improvement for Basic. A policy should
adapt to duplicate rates or workload cost: the earlier large ordinary-set fixture
benefited from larger batches, and globally forcing 512 regressed that fixture.
Publishing duplicate information earlier without forcing equally frequent global
pruning is another option, but a mutable shared history needs correctness,
contention and derivation-order evaluation. Production is left unchanged until
that implementation choice is requested.

## Follow-up: generation batches from 256 to 4,096

Current production kernels, ten workers, configured mimalloc v2. Three samples
per size, reversing the order for the middle sample, with no concurrent benchmark
or build. Every complete canonical result agrees. These are generation parent-pair
batches, not Rayon task grains. The large fixture measures its fixed second-step
maximization kernel; Basic measures both custom saturations. Neither includes
the full RE transformation sequence.

Large fixture medians:

| Generation pairs per batch | Elapsed | Aggregate CPU | System CPU |
| --- | ---: | ---: | ---: |
| 256 | 5.514 s | 19.502 s | 7.023 s |
| 512 | 3.555 s | 14.084 s | 3.579 s |
| 1,024 | 2.580 s | 11.298 s | 1.937 s |
| 2,048 | 2.113 s | 10.053 s | 1.072 s |
| 4,096 (current default) | 1.974 s | 9.941 s | 0.769 s |

For the large fixture, 256 takes **2.79 times as long (+179%)** and consumes
**96% more total CPU** than 4,096. Versus 512, it takes 55% longer and consumes
38% more CPU. Generation elapsed rises from 0.651 to 2.964 seconds; history/global
processing rises from 1.239 to 2.455 seconds. Much of the added CPU is system
time (0.769 to 7.023 seconds). More frequent dispatch, finer Rayon ranges and
global pruning incur overhead; the timers do not individually attribute kernel
time to particular scheduler/channel operations. The earlier 2.80-second result
for 512 was a historical fixed-task-grain prototype, not current production.

Basic medians from the same sweep:

| Generation pairs per batch | Elapsed | Aggregate CPU |
| --- | ---: | ---: |
| 256 | 8.454 s | 55.608 s |
| 512 | 7.790 s | 55.713 s |
| 1,024 | 7.511 s | 57.113 s |
| 2,048 | 7.647 s | 60.287 s |
| 4,096 (current default) | 8.141 s | 65.877 s |

On Basic, 256 saves 15.6% CPU versus 4,096 but takes 3.8% longer. Its CPU is
essentially equal to 512 (0.2% lower), while elapsed is 8.5% worse. The 1,024-pair
setting is fastest among these five tested fixed settings, at the cost of 2.5%
more CPU than 512. No fixed setting is best on both fixtures.

The subsequent [adaptive scheduling experiments](adaptive-batches.md) find a
measured compromise: 10-ms generation batches with learning from expensive round
tails reduce Basic's CPU by 15% and elapsed by 7%, while the large fixture and
matching stay within 0.5% of their default results. This remains a benchmark
prototype in this measurement section; the selected policy was subsequently
integrated. See the adaptive report's production integration section.

Raw records: [generation-batches-large.jsonl](generation-batches-large.jsonl)
and [generation-batches-basic.jsonl](generation-batches-basic.jsonl), with
complete canonical outputs in the corresponding `.results/` directories.

```sh
python3 crates/core/benches/re_comparison/diagnose_current.py \
  --binary /tmp/re3-target-linux/release/re-cpu-phases --case large \
  --workers 10 --samples 3 \
  --variants batch256 batch512 batch1024 batch2048 baseline \
  --output /tmp/re3-generation-batches-large.jsonl
```

Repeat with `--case basic` and a separate output path for Basic. Build the current
phase-only binary with `investigate_cpu.py` as described below. No production
batch policy or allocator was changed.

## Records and reproduction

- [basic-cpu-phases.jsonl](basic-cpu-phases.jsonl): five controlled policies and repeated parallel comparison.
- [basic-cpu-counts.jsonl](basic-cpu-counts.jsonl): serial, parallel, smaller batches and serial-pruning operation counts.
- The corresponding `.results/` directories retain the complete canonical outputs.

```sh
python3 crates/core/benches/re_comparison/investigate_cpu.py \
  /tmp/re3-cpu-phases --fixed-batches --build
python3 crates/core/benches/re_comparison/diagnose_current.py \
  --binary /tmp/re3-target-linux/release/re-cpu-phases --case basic \
  --workers 10 --variants baseline batch512 --samples 3 \
  --output /tmp/re3-basic-parallel-cpu.jsonl
```

Build with `--counts` in a second directory and select `re-cpu-counts` for
operation counters. Use `--workers 1 --variants baseline batch4096` for serial
controls, or `--workers 10 --variants serial-pruning` for serial global pruning
with parallel generation. All operations are performed on the same Basic fixture.
