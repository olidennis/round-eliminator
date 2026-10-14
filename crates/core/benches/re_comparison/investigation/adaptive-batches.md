# Adaptive generation batches

2026-10-13, current production kernels in temporary instrumented copies, native
Linux/aarch64 VM, ten logical CPUs/workers, Rust 1.89.0 release, configured
mimalloc v2 with option 26 = 0. Production was unchanged during these experiments;
the selected policy is now integrated, as recorded below.

## Recommended measured policy

Use measured generation cost to choose the next parent-pair batch. Expensive
generation benefits from earlier duplicate-history updates; cheap generation
benefits from amortizing dispatch and global pruning across more pairs.

The best tested compromise targets approximately **10 ms of generation per
batch**, with these rules:

1. Start with the existing 4,096-pair cap, separately for each degree.
2. Estimate pairs per 10 ms from the batch just generated. Bound changes to a
   factor of two and keep batch sizes between 512 and 4,096 on this ten-worker VM.
3. Learn from full batches and from partial batches that already took at least
   10 ms. Ignore cheap round tails, whose dispatch overhead can distort the rate.
4. Publish survivors to duplicate history and prune globally after each batch,
   using the existing ordered coordinator and immutable worker snapshots.

The target and count bounds govern scheduling; they are neither a search limit
nor a completion deadline. The closure still processes every required pair.
The policy does not inspect fixture names, algebra type, or custom empty-set rules.
The production implementation retains existing serial/wasm scheduling and
derives the native upper cap from worker count; the experimental overrides here
are for the ten-worker comparison, not proposed public configuration settings.

## Controlled repeated results

Three-sample medians, with the middle sample reversing policy order. Matching
averages 30 kernels in each process to amortize startup. Other cases execute
one kernel per process. Canonical complete results agree across all variants,
including the earlier fixed-batch reference outputs. Elapsed is kernel time;
CPU is aggregate process user plus system time, divided by iterations for matching.
Parsing, RE conversion, transport and the GUI are outside kernel elapsed time.

| Case | Current elapsed | Adaptive elapsed | Current CPU | Adaptive CPU |
| --- | ---: | ---: | ---: | ---: |
| Basic, both custom saturations | 8.129 s | **7.565 s** | 65.931 s | **55.892 s** |
| Large fixture, second-step maximum | 2.007 s | **2.000 s** | 10.056 s | **10.060 s** |
| Matching, fixed step-ten maximum | 26.400 ms | **26.503 ms** | 120.023 ms | **120.167 ms** |

Basic is **6.9% faster with 15.2% less CPU**. Relative to the previous controlled
serial CPU of 41.358 s, parallel overhead is about **35%**, rather than about
59%. The large fixture and matching changes are within 0.5% for both elapsed and
CPU, so no meaningful regression or improvement is established there.

The [fixed-batch sweep](basic-parallel-cpu.md) explains why a common fixed size
fails: 256 takes 5.514 s and 19.502 CPU seconds on the large fixture, versus
1.974 s and 9.941 CPU seconds at 4,096. Adaptive scheduling retains large batches
for cheap generation while obtaining essentially the CPU cost of smaller fixed
batches on Basic. It does not eliminate frozen-snapshot global pruning's extra
work and does not establish a universally optimal target for other workloads.

## Alternatives tested

- **Target 5 ms, with partial-batch learning:** Basic 7.725 s / 56.096 CPU seconds;
  large 2.003 s / 10.051 CPU seconds (three-sample medians). No advantage over
  10 ms on the measured workloads.
- **Ignore all partial batches:** targets 5/10/20 ms reduced Basic CPU only to
  roughly 59 s. The 10-ms median was 7.963 s / 59.269 CPU seconds. Small early
  rounds could already be expensive but did not update the limit. Including
  expensive partial batches lets the policy react before the first full batch.
- **Allow the limit only to shrink within a degree:** without partial-batch
  learning, 10-ms Basic was 8.164 s / 59.943 CPU seconds (three samples).
  With partial learning, the initial trial was 7.812 s / 56.443 CPU seconds.
  Keeping the ordinary bidirectional adjustment produced the better compromise.
- **Separate duplicate-history updates from global pruning:** publish history
  after each smaller generation batch but buffer candidates until at least
  4,096 parent pairs have been processed. With the revised shrinking policy, the
  initial Basic trial was 7.705 s / 58.317 CPU seconds. This did not reduce CPU
  further; it is excluded from the recommendation. Three earlier buffered trials
  also showed no CPU advantage. All buffered results were mathematically equal.

Repeated comparisons are distinguished from single exploratory trials above.
Per-operation counters were not rerun for the selected adaptive policy: the
CPU comparison measures cost rather than an exact count of algebra operations.

## Production integration and old/new comparison

At the user's request, the selected rule is now in `maximize/engine.rs`.
Caller-local batch state is reset for each degree. Native untraced generation
measures elapsed time only around generation, then uses the same bounded rate
adjustment as the prototype. The cap scales with worker count. Serial, wasm,
nested Rayon calls and observed derivations retain their prior fixed batches;
the trace's admission order does not depend on timing. No API, model, algebra,
allocator, global-pruning algorithm or progress/cancellation policy changes.
This small private scheduling helper stays with the closure engine; shared Rayon
dispatch remains in `parallel.rs`.

Verification: **128 core, 5 server and 2 wasm-adapter Rust tests**, including
expensive-tail learning, rate adjustment bounds and zero-duration behavior.
Rebuilt wasm and release server pass all **18 maximization and 18 Apply RE
fixtures** through both transports. The frontend typechecks and builds, with the
updated wasm included in `web/dist`.

The user requested reusing existing timings rather than measuring again. The
following combines the selected prototype's three-sample medians with the earlier
integrated old-parallel/new-serial medians and old fully serial references.
These measure identical fixed maximization inputs, or both Basic saturations;
they do not measure full RE sequences. Basic old serial is one expensive run;
other entries are three-sample medians. All use configured mimalloc v2.

| Case | Old parallel elapsed | New adaptive elapsed | Old parallel CPU | New adaptive CPU |
| --- | ---: | ---: | ---: | ---: |
| Basic | 27.496 s | 7.565 s | 272.056 s | 55.892 s |
| Large fixture, second-step maximum | 19.636 s | 2.000 s | 191.012 s | 10.060 s |
| Matching, step-ten maximum | 103.219 ms | 26.503 ms | 439.834 ms | 120.167 ms |

Aggregate CPU overhead is `(parallel CPU - serial CPU) / serial CPU`:

| Case | Old serial CPU | New serial CPU | Old additional CPU | New additional CPU |
| --- | ---: | ---: | ---: | ---: |
| Basic | 225.250 s | 41.729 s | 46.806 s (+20.8%) | 14.163 s (+33.9%) |
| Large fixture | 145.416 s | 5.803 s | 45.595 s (+31.4%) | 4.258 s (+73.4%) |
| Matching | 333.874 ms | 81.751 ms | 105.960 ms (+31.7%) | 38.416 ms (+47.0%) |

Absolute extra CPU is smaller in all three new cases. Relative percentages are
higher because the new serial baselines are much faster. The old fully serial
baseline is its existing wasm saturation loop compiled natively, rather than
its native pipeline with one worker. Its serial and parallel pruning differ,
so the percentage includes algorithmic differences as well as threading costs.
The old ten-worker limit launches ten generation and ten pruning workers on
the same ten-CPU VM; new uses one ten-worker Rayon pool.

Sources: `integrated-{matching,test-all-second,basic}-results.jsonl`,
`{matching,test-all-second,basic}-old-serial-mimalloc-v2-results.jsonl`, and
the `adaptive-partial-*` records below. No timing runs were added during integration.

## Records and reproduction

- [adaptive-partial-basic.jsonl](adaptive-partial-basic.jsonl): selected policy,
  5-ms comparison and baseline, three samples each; initial alternative trials.
- [adaptive-partial-large.jsonl](adaptive-partial-large.jsonl): corresponding
  large-fixture measurements.
- [adaptive-partial-matching.jsonl](adaptive-partial-matching.jsonl): three
  samples of 30 kernels for baseline and the recommended adaptive policy.
- `adaptive-batches-{basic,large}.jsonl`: earlier full-batch feedback trials.
- `adaptive-sticky-{basic,large}.jsonl`: earlier shrinking/buffered feedback trials.
- Each corresponding `.results/` directory retains complete canonical outputs.

```sh
python3 crates/core/benches/re_comparison/investigate_adaptive.py \
  /tmp/re3-adaptive-phases --build
python3 crates/core/benches/re_comparison/diagnose_current.py \
  --binary /tmp/re3-target-linux/release/re-adaptive-phases --case basic \
  --workers 10 --samples 3 --variants baseline adaptive10-partial \
  --output /tmp/re3-adaptive-basic.jsonl
```

Repeat with `--case large`; for matching use `--case matching --iterations 30`.
The diagnostic builder's environment flags and optional batch trace are confined
to temporary benchmark copies. `investigate_adaptive.py` restores the frozen
pre-adaptive engine in `baseline/2026-10-13/fixed-batch-engine.rs`, keeping the
original alternatives reproducible. `investigate_cpu.py --fixed-batches` selects
that engine for the historical fixed-size/CPU-overhead experiments; without the
flag it instruments current production, and a batch override disables adaptation.
