# Live parallel pruning and packed sets

2026-10-13, Linux/aarch64, ten logical CPUs, Rust 1.89.0 release, configured
mimalloc v2 with option 26 = 0. The selected changes are integrated.

## Selected implementation

- Ordinary maximization packs alphabets of at most 64 distinct labels into
  `u64` masks. Larger alphabets use vector sets. Decoding restores normal label
  sets and canonical ordering; this does not alter the model or impose a limit.
- Packed generation uses minimum 256-item Rayon ranges for maps of at least
  512 parent pairs. Generic custom algebras retain normal task splitting.
  Adaptive bounded generation batches and duplicate history are retained.
- Native pruning workers share an append-only list of immutable entries with
  atomic removal flags. Each worker keeps a cache of useful dominators and takes
  eight candidates at a time from an atomic queue, balancing irregular work.
- Workers refresh entry handles, scan newest first, and skip removed entries.
  Matching runs outside the list lock. Before appending, a worker validates any
  entries published since its scan; equivalent concurrent candidates cannot both
  survive. A newly appended candidate immediately becomes available to others.
  It then marks earlier configurations it dominates as removed.
- Workers finish before the coordinator compacts survivors. Removed entries
  remain valid cached dominators because a stronger replacement covers them.
  Final output is sorted in the common engine.
- Worker counts adapt using summed worker elapsed time per offered candidate
  from the preceding batch. The selected target is 8 ms per worker, clamped
  between two workers and the shared pool size. This cost estimate includes
  waiting inside a worker; it is a scheduling heuristic, not measured CPU.
- Small batches, serial execution and wasm keep ordinary insertion. Tracing
  retains the ordered admission path. Generation and global pruning still
  alternate by batch; the old two-pool pipeline was not copied.

This recovers immediate pruning feedback without a new append-only collection
dependency, unsafe code, extra worker pools, or mutable configuration payloads.

## Final uninstrumented measurements

Three-sample medians for matching and the large fixture, alternating serial and
parallel order. Matching executes 30 kernels per process. Basic parallel uses
three samples; its newly measured serial reference uses one expensive sample.
Whole-process user + system CPU includes every thread, setup and output checks.
Kernel elapsed time excludes parsing, transport and UI. Each run compares the
complete canonical maximum, not just the retained configuration count.

| Kernel | Serial elapsed | Serial CPU | Parallel elapsed | Parallel CPU | Extra CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Large fixture, fixed second maximum | 3.298 s | 3.300 s | **1.226 s** | **4.277 s** | **29.6%** |
| Matching, fixed step-ten maximum | 35.30 ms | 35.40 ms | 16.69 ms | 54.72 ms | 54.6% |
| Basic, both custom saturations | 41.356 s | 41.353 s | 7.211 s | 51.534 s | 24.6% |

The large fixture meets the requested <=2 s and <=30% extra CPU targets with
parallel pruning. Its absolute extra CPU is 0.977 s. The previous integrated
vector implementation measured 2.000 s / 10.060 s CPU there, versus its own
5.803 s serial CPU (+73.4%). Basic previously measured 7.565 s / 55.892 s CPU;
matching measured 26.50 ms / 120.17 ms CPU. These earlier uninstrumented numbers
are reused from the adaptive-batch report, not remeasured with the old binary.

Matching's total CPU falls by more than half, but its relative overhead remains
above 30% because packing also makes serial execution much faster. Do not claim
that this policy meets the CPU target on every workload. Further generation
scheduling work for short packed kernels belongs in future investigations.

## Alternatives tested

Quiet three-sample prototype medians, with light phase instrumentation:

| Large-fixture policy | Elapsed | CPU |
| --- | ---: | ---: |
| Vector sets, snapshot pruning | 1.987 s | 9.984 s |
| Packed sets, snapshot pruning | 1.208 s | 5.966 s |
| Packed sets, live pruning with ten workers, coarse generation | 0.933 s | 5.102 s |
| Packed sets, live pruning with four workers, coarse generation | 1.033 s | 4.483 s |
| Packed sets, live pruning with two workers, coarse generation | 1.275 s | 4.260 s |
| Packed sets, serial pruning, coarse generation | 1.910 s | 4.290 s |
| Packed sets, live pruning, selected 8-ms worker policy | 1.235 s | 4.305 s |

On Basic, live pruning with ten workers takes 6.37 s, versus 7.54 s for
snapshot pruning. Fixing pruning to two workers instead takes 8.76 s. This
confirms why serial or fixed two-worker pruning should not be a universal rule.
The selected adaptive policy trades some of the fastest variant's latency for
less aggregate CPU while still improving all three measured production kernels.

Worker targets of 2, 5, 8, 20 and 40 ms were tested. The 20/40-ms targets
underutilized expensive Basic pruning. The 5-ms target met the large-fixture
budget in the instrumented prototype, but final uninstrumented execution used
30.8% extra CPU. The selected 8-ms target leaves more margin and still improves
Basic. A universal scheduling optimum is not established by these fixtures.

## Correctness and records

Native tests compare concurrent pruning with serial insertion on counted,
multi-part configurations, exercise cached removed dominators and duplicate
admissions, and force a concurrent-admission race with a barrier. Cancellation
during comparison returns no maximum. Packed tests cover all 64 bit positions,
noncontiguous IDs, maximum multiplicities, zero degree, and a 65-label fallback.
Existing exhaustive universal-maximization and custom-algebra tests are retained.

Final verification: 141 core + 5 server + 2 wasm-adapter tests pass. Rebuilt
adapters agree on 18 maximization, 18 RE and 6 Basic FP fixtures. Frontend
typechecking/build, Rust formatting and whitespace checks pass.

- `live-integrated8-work.jsonl`: final uninstrumented matching/large serial and
  parallel measurements.
- `live-integrated8-basic-{10,1}.jsonl`: final uninstrumented Basic measurements.
- `live-quiet-{large,basic,matching}-10.jsonl`: quiet policy comparisons.
- `live-adaptive*-{large,basic,matching}-10.jsonl`: scheduling-target experiments.
- `live-integrated-work.jsonl`: intermediate integrated 5-ms policy, superseded
  by the final 8-ms measurements.
- **Exclude `live-large-10.jsonl` timing data:** the user reported a concurrent
  heavy computation during those initial trials. Output comparisons remain valid.

`investigate_packed.py` restores the pre-live baseline, then inserts experimental
adapters into a temporary copy. Baseline `maximize.rs`, `engine.rs`,
`antichain.rs`, and `parallel.rs` are frozen under
`baseline/2026-10-13/pre-live/`. Historical experiments stay reproducible after
integration. The production implementation is in `maximize/packed.rs` and
`maximize/antichain/live.rs`.

To build the final uninstrumented benchmark, then measure ordinary maxima:

```sh
python3 crates/core/benches/re_comparison/run.py --allocator mimalloc-v2 \
  --build-only --target-dir /tmp/re3-target-linux
python3 crates/core/benches/re_comparison/compare_work.py \
  --binary /tmp/re3-target-linux/release/re-comparison-mimalloc-v2 \
  --workers 1 10 --samples 3 --output /tmp/re3-live-work.jsonl
```

Run benchmarks alone. The allocator above belongs to the benchmark executable;
this change does not select a production allocator. CPU time is a proxy for
total work, not a hardware energy measurement.
