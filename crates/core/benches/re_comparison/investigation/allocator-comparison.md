# Basic fixed-point saturation and allocator comparisons

Native aarch64 Linux VM, ten logical CPUs, Rust 1.89.0, release builds.
Measurements run sequentially. Both libraries use the same global allocator
inside each comparison executable. The earlier comparison sweeps checked complete
canonical results across implementations, worker counts and allocators. The old
serial follow-up checks complete results between its repeated samples; its row
counts also agree with the earlier runs. Earlier canonical outputs were not retained
for a direct comparison against this follow-up.

This updated report omits mimalloc v3. **Only old serial with mimalloc v2 was
newly measured**; all other timings below are reused from the previous report.

## Scope and allocators

The old server selects jemalloc on Linux and mimalloc elsewhere. The old library
does not select an allocator. Earlier comparison executables used the **system
allocator for both libraries**; those measurements isolate the algorithms without
reproducing the old server's allocator setting.

`run.py --allocator` builds separate native executables with the system allocator,
`tikv-jemallocator 0.6.1`, or `mimalloc 0.1.52` / `libmimalloc-sys 0.1.49`.
These versions match the old server's lockfile. The reported mimalloc trials
explicitly enable its v2 backend and include the old `mi_option_set(26, 0)`
setting: v2 assigns option 26 to `abandoned_reclaim_on_free`, where zero disables
abandoned-memory reclaiming during a free. The setting is applied to the experiment
on Linux as well. These are Linux measurements, rather than measurements of the
Mac server or its system allocator.

One new worker disables both parallel generation and pruning. Ten new workers
share one bounded pool. The old worker limit of ten creates ten generation plus
ten pruning workers; both stages share the same ten logical CPUs. Production
scheduling policies remain active. Parsing, shared initialization, result
conversion and serialization are excluded from kernel wall time. Aggregate CPU
time includes the entire measured process and all its threads, divided by kernel
repetitions. It is a measure of occupied CPU time, not an instruction count.

The old native pipeline still runs generation and pruning concurrently at worker
limit one. For **fully serial old** execution, the runner copies the old library
into a temporary package and switches only the two configuration guards in
`maximize.rs`, compiling its existing wasm serial loop **natively**. This uses
the same compiler, hardware and configured mimalloc v2. Neither original source
tree is changed. Both worker environment limits are one; shared initialization
is outside the kernel timer. The old serial loop has per-parent intermediate
pruning rather than the native pipeline's pruning worker/cache, so this compares
the two existing old implementations as well as their concurrency.

No production allocator or fixed-point UI feature is added by this experiment.
Allocator selection belongs to the benchmark executable; dependencies remain
outside the production workspace.

## Requested Basic fixed-point problem

```text
A^5 X^2
B^5 Y^2
C^5 Z^2

AX BYCZ
BY CZ
XYZ^2
```

The adapter follows the Basic route in the old `fixpoint.rs`:

1. Compute the original passive diagram and its default lattice of
   successor-closed sets, including the empty set. This input gives **27 values**.
2. Expand original constraints with `all_choices(true)`, then map their singleton
   labels into the lattice. There are **3 active and 15 passive seed rows**.
3. Saturate the active side in reverse inclusion: its join is set intersection,
   and its meet is set union. All values, including empty, are admissible.
4. Saturate the passive side using ordinary inclusion, union and intersection.

Both engines use the same HashMap join/meet tables and HashSet reachability
callbacks. The new generic engine operates on scalar lattice IDs; the old one
wraps IDs in singleton Groups, as its original `procedure` does. The empty set
has an ordinary lattice ID rather than an empty legacy Group. Tracking is off.
Results decode IDs back to closed sets before complete equality checks.

The measured total is the sum of both saturations, rather than standard
passive-only maximization or an RE step. Shared initialization takes approximately
2 ms and is reported separately. All completed runs produce **102 active and
14 passive configurations**; almost all elapsed time is in the active saturation.
This expensive case uses **one complete run per setting**, not three-run medians.

### System allocator

| Implementation | Elapsed | Aggregate CPU |
| --- | ---: | ---: |
| New, fully serial | 371.947 s | 371.919 s |
| New, ten workers | 49.370 s | 471.236 s |
| Old, worker limit ten | 33.134 s | 327.316 s |

New parallel execution is **7.53× faster than new serial**, using **27% more
aggregate CPU**. The old engine is faster and uses less CPU than the new
parallel engine on this custom algebra. This contrasts with the ordinary
maximization inputs below; their performance ranking does not carry over here.

Raw system results: [basic-system-results.jsonl](basic-system-results.jsonl).
Custom allocator results: [basic-custom-allocator-results.jsonl](basic-custom-allocator-results.jsonl).

### All allocator results

| Allocator | New serial | New parallel | Old serial | Old parallel |
| --- | ---: | ---: | ---: | ---: |
| System | 371.947 s | 49.370 s | — | 33.134 s |
| jemalloc | 346.233 s | 46.600 s | — | 28.236 s |
| mimalloc v2, abandoned reclaim = 0 | 331.741 s | 43.823 s | 225.283 s | 28.070 s |

| Allocator | New serial CPU | New parallel CPU | Old serial CPU | Old parallel CPU |
| --- | ---: | ---: | ---: | ---: |
| System | 371.919 s | 471.236 s | — | 327.316 s |
| jemalloc | 346.202 s | 444.890 s | — | 279.043 s |
| mimalloc v2, abandoned reclaim = 0 | 331.709 s | 418.500 s | 225.250 s | 277.202 s |

With configured v2, new parallel is **7.57× faster than new serial** while
using **26% more CPU**. It still takes **1.56× the old elapsed time** and uses
**1.51× the old CPU time**. Allocator choice helps, but does not close the
custom-algebra gap. These are single runs; small differences between custom
allocators require repeated measurements before establishing a ranking.

Old serial is **1.47× faster than new serial** on this custom algebra. Old
parallel is **8.03× faster than old serial**, using **23% more aggregate CPU**.
Old serial's 225.283 s consists of 225.283 s active and 0.000485 s passive
saturation (rounded). A dash means old serial was not measured with that allocator.

## Ordinary matching and large-fixture maximization

These use the same frozen step-ten matching and step-two large-fixture snapshots
as the earlier investigation, with ordinary union/intersection and nonempty
output parts. Tables give **three-sample medians**. Each matching sample averages
30 kernels in one process; the large fixture runs once per process. Variant order
reverses on alternate samples. Values are for **one fixed maximization**, not
ten matching RE calls or two large-fixture RE calls.

| Input | Allocator | Old, serial | Old, ten-worker limit | New, serial | New, ten workers |
| --- | --- | ---: | ---: | ---: | ---: |
| Matching step 10 | System | — | 113.96 ms | 140.33 ms | 50.76 ms |
| Matching step 10 | jemalloc | — | 106.27 ms | 131.08 ms | 50.36 ms |
| Matching step 10 | mimalloc v2, abandoned reclaim = 0 | 333.71 ms | 104.64 ms | 130.52 ms | 48.33 ms |
| Large fixture step 2 | System | — | 24.683 s | 8.393 s | 5.678 s |
| Large fixture step 2 | jemalloc | — | 19.879 s | 5.894 s | 5.064 s |
| Large fixture step 2 | mimalloc v2, abandoned reclaim = 0 | 145.433 s | 19.159 s | 5.694 s | 5.010 s |

With mimalloc v2, new serial is **2.56× faster than old serial** on matching
and **25.54× faster** on the large fixture. All old serial samples contain
52 matching maxima or 153 large-fixture maxima, agreeing with earlier row counts.

### Aggregate CPU with mimalloc v2

| Input | Old serial | Old parallel | New serial | New parallel |
| --- | ---: | ---: | ---: | ---: |
| Matching step 10 | 333.87 ms | 444.97 ms | 130.61 ms | 180.66 ms |
| Large fixture step 2 | 145.416 s | 186.399 s | 5.696 s | 18.527 s |

Relative to their own serial implementations, old parallel uses **33% more CPU**
on matching and **28% more** on the large fixture. New parallel uses **38% more**
on matching and **225% more** on the large fixture. These ratios include the
differences in pruning and scheduling, not just parallel execution overhead.

Custom allocation improves the serial large-fixture result much more than the
parallel result. Jemalloc reduces new serial elapsed time by approximately 30%,
new parallel by 11%, and old parallel by 19%. Matching changes are smaller:
approximately 6–7% for new serial, and less than 5% for new parallel across the tested backends.

### Aggregate work on the large fixture

| Allocator | New serial CPU | New parallel CPU | CPU multiplier | Elapsed speedup |
| --- | ---: | ---: | ---: | ---: |
| System | 8.400 s | 20.877 s | 2.49× | 1.48× |
| jemalloc | 5.897 s | 17.510 s | 2.97× | 1.16× |
| mimalloc v2, abandoned reclaim = 0 | 5.696 s | 18.527 s | 3.25× | 1.14× |

Thus the parallel speed-versus-work tradeoff is less favorable after improving
the serial allocator. System CPU in the parallel new engine remains roughly
5.5–5.9 s with the reported allocators; the custom allocators mainly reduce user CPU.
This does not separately identify allocation, scheduling or synchronization
costs. The earlier redundant-comparison counts remain relevant; changing the
allocator does not remove those comparisons.

Raw records: [matching-allocator-results.jsonl](matching-allocator-results.jsonl)
and [test-all-second-allocator-results.jsonl](test-all-second-allocator-results.jsonl).
Separate v2 records are [matching-mimalloc-v2-results.jsonl](matching-mimalloc-v2-results.jsonl),
[test-all-second-mimalloc-v2-results.jsonl](test-all-second-mimalloc-v2-results.jsonl),
and [basic-mimalloc-v2-results.jsonl](basic-mimalloc-v2-results.jsonl).

The v2 rows were collected subsequently in a separate three-sample sequence,
reversing implementation order between samples. They were not interleaved with
the other allocators, so small cross-allocator differences have that limitation.
The v2 results do not isolate the effect of setting 26 independently of backend
version: every v2 run uses the requested zero setting.

Old serial follow-up records:
[matching-old-serial-mimalloc-v2-results.jsonl](matching-old-serial-mimalloc-v2-results.jsonl),
[test-all-second-old-serial-mimalloc-v2-results.jsonl](test-all-second-old-serial-mimalloc-v2-results.jsonl),
and [basic-old-serial-mimalloc-v2-results.jsonl](basic-old-serial-mimalloc-v2-results.jsonl).
Only these seven processes were newly measured: three matching samples of 30
kernels, three large-fixture samples, and one Basic run. Serial CPU approximately
equals wall time in every case. Historical raw records retain v3 measurements,
but their results are excluded from this updated report.

## Reproduction

Build each executable through the existing comparison runner, using a tiny
untimed-for-this-report smoke sequence:

```sh
python3 crates/core/benches/re_comparison/run.py --allocator system \
  --case matching --steps 1 --samples 1 --modes new-re \
  --target-dir /tmp/re3-target-linux --output /tmp/re3-build-system.jsonl
python3 crates/core/benches/re_comparison/run.py --allocator jemalloc \
  --case matching --steps 1 --samples 1 --modes new-re \
  --target-dir /tmp/re3-target-linux --output /tmp/re3-build-jemalloc.jsonl
python3 crates/core/benches/re_comparison/run.py --allocator mimalloc \
  --case matching --steps 1 --samples 1 --modes new-re \
  --target-dir /tmp/re3-target-linux --output /tmp/re3-build-mimalloc.jsonl
```

Repeat the build command with `--allocator mimalloc-v2` to enable the v2
backend on the same pinned Rust wrapper.

Then reproduce the measured comparisons:

```sh
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --case basic --samples 1 --output /tmp/re3-basic-allocators.jsonl
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --case matching --samples 3 --output /tmp/re3-matching-allocators.jsonl
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --case test-all-second --samples 3 --output /tmp/re3-large-allocators.jsonl
```

Add `--allocator mimalloc-v2` to run the separate v2 trial; default allocator
selection runs system, jemalloc and v3.

The Basic system pilot ran new serial, new parallel, then old. The later custom
allocator trials ran old, new parallel, then new serial. With one sample each,
this does not remove ordering effects or establish small allocator differences.
The matching/large comparisons interleave allocators and alternate the whole
variant order across their three samples. The runner's 600-second per-process
watchdog is measurement tooling, not an algorithmic cutoff; timed-out runs are
recorded separately and cannot provide a complete-result guarantee.

To reproduce only the old serial v2 follow-up, build without running a smoke
measurement, then select the serial executable explicitly:

```sh
python3 crates/core/benches/re_comparison/run.py --allocator mimalloc-v2 \
  --old-serial --build-only --target-dir /tmp/re3-target-linux
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --allocator mimalloc-v2 --old-serial --case matching --samples 3 \
  --timeout 1800 --output /tmp/re3-matching-old-serial-v2.jsonl
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --allocator mimalloc-v2 --old-serial --case test-all-second --samples 3 \
  --timeout 1800 --output /tmp/re3-large-old-serial-v2.jsonl
python3 crates/core/benches/re_comparison/compare_allocators.py \
  --allocator mimalloc-v2 --old-serial --case basic --samples 1 \
  --timeout 1800 --output /tmp/re3-basic-old-serial-v2.jsonl
```
