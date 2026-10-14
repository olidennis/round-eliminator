# Four remaining maximization experiments

2026-10-13. User requested switching the server allocator first, testing precisely
these four categories, then stopping. Other review items stay deferred.

## Scope and method

Native server now pins mimalloc0.1.52 v2/sys0.1.49 and sets option26=0 before Tokio
workers start. Option26 is abandoned_reclaim_on_free in the pinned v2 C header;
the older extended Rust constants do not name it. The setting matches the
benchmark executable. Core and wasm acquire no allocator. Server release build,
v2 feature-tree inspection and20server/wasm maximization smoke fixtures pass.

The experiments use temporary copies restored from baseline/2026-10-13/final-four,
including12ms pruning, u64/u128 packing, prepared alignments, stack compatibility
and selective generated history. Production carries no experiment flags.
Ten-CPU Linux/aarch64, Rust1.89 release/mimallocv2 option26=0. One measured child
at a time without builds/tests/heavy work. Three samples, reversed variant order
on alternate samples; matching repeats30kernels/process. Full canonical maxima
must agree; wall time measures the kernel, aggregate child user+system CPU and
peakRSS include fixture preparation and all workers. This is work proxy, not energy.

## 1. Flow-network reuse

Retain a petgraph Graph and destination-index buffer per thread, clear them for
new topology and reuse capacity after necessary rejection. Keep petgraph Dinic,
including its fresh internal scratch arrays. Reentrant compatibility callbacks
fall back to fresh storage rather than panicking on a RefCell borrow. Compare
complete maximization on matching, large and both custom Basic saturations.

No meaningful improvement on the three native workloads: large baseline1.111s
/3.066CPU vs reuse1.108s/3.055CPU; Basic6.146/41.383 vs6.194/41.272; matching
14.796ms/49.646CPUms vs14.777/49.785. Differences are small/noisy, and retaining
per-thread storage/reentrancy handling adds complexity. **Not adopted.**
Raw records four-{large,basic,matching}.jsonl.

## 2. Sets beyond128 labels

Compare fixed four-word masks (129..256, vectors above256) and dynamic immutable
Arc<[u64]> words (unbounded alphabet, cloned values share their word payload).
Public LabelSet/model/wire shapes stay unchanged. Use dense twin-label large
fixtures at232/522labels and genuinely sparse independent triangle fixtures at
174/522labels. Twin aliases always occur together; they measure representation
cost on an existing closure shape, not arbitrary application workloads.
Stored old problem sections are unchanged driver scaffolding: these synthetic
fixtures are for resource-new only, NOT old-library comparison.

Four-word masks and dynamic masks both improve dense wide cases, but lose on
sparse sets. Sparse174: vectors8.9ms vs four words15ms/dynamic11ms (initial
low-word-first order). Sparse522: vectors80ms vs dynamic109ms with high-word-first
order. **Keep vectors for sparse wide sets.** Dense232 with high-word-first
four words:2.870s/9.745CPU→1.567/5.214. Dense522 dynamic:4.012/14.803→2.381/7.082.
Fixed masks win below256; dynamic masks extend the approach beyond256 without
an alphabet/search limit. Numeric high-word-first ordering improved dense-mask
traversal compared with the initial derived low-word-first ordering.

**Adopted:** four-word masks129..256 and shared dynamic masks above256 when
average source set cardinality is at least the word count; vectors otherwise.
This is a storage heuristic, not a proven universal crossover or search cutoff.
Raw four-wide232{-high}, four-wide522, four-sparse{174,522}.jsonl.

## 3. Overlap generation/pruning

Reserve5or8generation workers in the SAME ten-worker Rayon pool, leaving workers
available for concurrent pruning. A bounded channel supplies completed chunks;
parents and global history stay frozen until the batch finishes. The caller
removes exact duplicates and updates the live antichain. Abort drops the receiver,
cancels producers and joins scoped work; tracing and serial execution use the
ordinary ordered path. No second pool or oversubscription.

Test both immediate consumption and buffered pruning at1024fresh candidates.
Buffering avoids forcing tiny fragments through mostly serial global insertion.

**Not adopted.** Large immediate consumption: baseline1.111s/3.066CPU;
5/5workers3.655/5.595 and8/2workers3.623/5.553. Buffered consumption also lost:
5/5workers4.069/5.755,8/2workers3.997/5.735 vs baseline1.113/3.065 (three samples).
The pipeline's changed admission order and repeated pruning fragments outweigh
its overlap; reserving workers cannot recover the latency on these cases.

Basic buffered (ONE expensive screening sample each): sequential phases
6.139s/40.999CPU;5/5workers9.489/36.746;8/2workers7.911/37.399. Lower CPU is a
possible tradeoff, but the elapsed regression is large, so the default retains
its faster complete-batch approach. Results all agree. Raw four-large,
four-buffered-{large,basic}.jsonl. These trials do not prove that every possible
pipeline design is slower.

## 4. History compression and small-workload scheduling

Compression interns exact ordered values, replacing repeated configuration
payloads with dictionary IDs/counts encoded as varints. Lookup uses an inline
128-byte key, falling back without a limit; stored keys own compressed bytes.
Equality stays exact, not fingerprint-based, and the generic algebra needs no
Hash/Serialize bounds. Global history is frozen during generation; the existing
shared generated history stays unchanged. Test matching, large and Basic, plus
wide vector workloads where keys carry larger values.

Scheduling compares packed generation grains256/512/1024. It changes task size,
not the visited pairs/configurations; custom-algebra splitting remains unchanged.
Check matching and large latency, CPU and memory before selecting a grain.

Broad compression is not selected: large1.111s/3.066CPU→1.203/3.257 while
RSS71.78→48.07MiB; Basic6.146/41.383→6.246/41.640; matching14.796/49.646ms→
15.317/50.499ms. It saves memory at a runtime cost there. It helps vector-wide
cases, and combining it with the selected dynamic-mask backend helps BOTH:
522-label dynamic2.376s/7.104CPU/RSS221.08MiB→compressed dynamic
2.135/6.329/RSS47.03MiB (three alternating samples).

**Adopted compression only for dynamic wide masks**, including value interning.
Other representations/custom public algebras retain full keys. New private
maximize/history.rs provides the exact strategy with Ord-only values and no
wire/model changes. Raw four-compressed-words.jsonl plus the earlier case files.

Global grains512/1024 slow matching from14.796ms to16.846/17.828 without meaningful
CPU reduction. On large,512 preserves time (~1.120s) while reducing CPU3.066→2.938;
1024 slows time1.215s and increases RSS (~85MiB vs72). **Adopted a conditional512
grain only for packed degrees whose normalized seeds all have unit counts.**
Repeated-count cases keep256, custom algebras keep1, and traced execution stays
ordered. Unit-count large:1.113s/3.065CPU→1.118/2.949; matching unchanged in the
unit-grain test. A scheduling heuristic, not a universal guarantee.
Raw four-{large,matching}, four-buffered-large, four-unit-matching.jsonl.

## Reproduce

```sh
python3 crates/core/benches/re_comparison/final_four/prepare.py /tmp/re3-final-four --build
python3 crates/core/benches/re_comparison/final_four/run.py --binary /tmp/re3-target-linux/release/re-final-four --case large --workers 10 --samples 3 --variants baseline flow pipe5 pipe8 compressed grain512 grain1024 --output /tmp/four-large.jsonl
```

Records `four-*.jsonl` and their
adjacent .results store timing/CPU, complete normalized results and per-trial
peakRSS/binarySHA256. Native before executable /tmp/re3-four-before is saved for
uninstrumented production confirmation. Old RE1 numbers are not remeasured.
The frozen final-four and previous review experiment copies both pass
`cargo check --release` after restoring their historical wrappers.
Stop after these four categories and required integration/verification gates.


## Final verification and stopping point

All four categories have now been tested. No fifth optimization or unrelated
feature is pursued. Selected changes: allocator, density-aware wider masks,
compressed dynamic-mask history and unit-count packed task grain. Rejected:
flow retention, overlap, broad compression and unconditional coarser tasks.

149core+5server+2wasm-adapter=156 Rust tests pass. Added exact compressed/full
history parity with IDs/counts around varint boundaries, u32::MAX and keys beyond
inline capacity, using a non-Hash/non-Serialize value type. Wider dense closure
matches the vector oracle at129/256/257/522 labels with noncontiguous IDs,
word-boundary joins, zero degree and huge counted multiplicities. Sparse fallback
still has no limit. Existing uniform-domination, algebra, cancellation and
parallel/trace tests remain.

Native server and wasm rebuilt.23maximization fixtures (through522labels),18RE
and6BasicFP fixtures agree on both transports. Frontend typecheck/build pass.
All public model/API shapes and input promises remain unchanged. The reusable
history strategy lives under maximize; packed storage stays in packed.rs, and
allocator policy in the server executable.

Uninstrumented final confirmations are recorded in four-final-*.jsonl; their
complete results must match the saved before executable, independent of profile
flags. Final timing table is appended after those confirmation runs.


### Integration correction

The first uninstrumented four-word integration used generic BitAnd/Not wrappers
and checked every word for subset/emptiness; it ran3.15s/15.37CPU on wide232,
worse than the vector baseline. These are retained separately in
four-integration-generic-quad.jsonl, NOT final results. The production adapter now
uses the directly benchmarked FourOperations, borrowed operands and early-exit
subset/admissibility loops. The final four-final-wide232.jsonl supersedes that
failed integration. Mathematical/canonical results agreed in both versions.
Do not infer that a generic wrapper is equally fast without its own measurement.

## Uninstrumented final confirmation

| Kernel | Before elapsed | Final elapsed | Before CPU | Final CPU | Before RSS | Final RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| large | 1.114 s | 1.123 s | 3.070 s | 2.928 s | 71.80 MiB | 75.88 MiB |
| matching | 14.922 ms | 14.826 ms | 50.625 ms | 49.835 ms | 35.75 MiB | 34.87 MiB |
| basic | 6.149 s | 6.124 s | 41.320 s | 41.298 s | 34.56 MiB | 34.46 MiB |
| wide232 | 2.898 s | 1.587 s | 9.884 s | 5.034 s | 213.16 MiB | 125.33 MiB |
| wide522 | 4.018 s | 2.137 s | 14.826 s | 6.029 s | 386.18 MiB | 52.62 MiB |
| sparse174 | 9.191 ms | 9.190 ms | 10.751 ms | 10.789 ms | 31.73 MiB | 32.11 MiB |
| sparse522 | 81.280 ms | 80.771 ms | 84.349 ms | 83.782 ms | 33.73 MiB | 34.11 MiB |

Three samples, alternating before/after order. Matching repeats30kernels;
sparse174 repeats5, sparse522 repeats3. All complete canonical results agree.
Wide fixtures are synthetic; the representation heuristic is not a universal
performance guarantee. No regression beyond measurement noise on the original
large/matching/Basic kernels or sparse cases; substantial improvements on dense
wide cases. Four-word direct operations are included in the final adapter.
large: final serial2.358s/2.360CPU; extra parallel CPU24.06% (3serial samples).
basic: final serial33.920s/33.912CPU; extra parallel CPU21.78% (1serial sample).
