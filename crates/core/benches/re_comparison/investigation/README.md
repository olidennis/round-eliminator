# Why the new maximizer is slower

Investigation on 2026-10-12–13, native aarch64 Linux, Rust 1.89.0, release
optimization, 10 logical CPUs. The initial investigation used benchmark tooling
and temporary instrumented copies. The selected optimizations have subsequently
been integrated into production; the historical tables below describe each stage.

The [current-code follow-up](current-performance.md) investigates the remaining
Basic custom-algebra gap and excessive parallel CPU on the large fixture, using
configured mimalloc v2. It includes measured prototype improvements and workload
regressions; the selected changes are now integrated, with production verification
and timings in its production section.

## Scope and comparison

These measurements isolate **one maximization**, using frozen identical inputs:

- [matching-10.json](matching-10.json): input to the tenth raw RE step on
  degree-10 bipartite maximal matching; 47 labels, 28 passive rows, degree 10.
- [fixture-2.json](fixture-2.json): input to the second raw RE step on the old
  long-label `test_all()` fixture; 58 labels, 32 passive rows, degree 5.

Each snapshot contains both libraries' representations. Cloning, parsing,
conversion, UI, and transport are outside timing. The optional new-result JSON
dump used for equality checks is inside diagnostic timing. Operation counters
also add overhead. These timings supplement the uninstrumented sequence
[comparison](../README.md); they are not replacement end-to-end RE timings.
Matching values below are medians of three runs per variant. The expensive
fixture has one diagnostic run per variant. No measurements run concurrently.

| Isolated maximization | Old | Current new | Combined prototype |
| --- | ---: | ---: | ---: |
| Matching, step 10 | 122.2 ms | 256.0 ms | 63.1 ms |
| Long-label fixture, step 2 | 25.62 s | 39.01 s | 10.08 s |

The combined prototype uses early rejection of dominated intersection pairings,
capacity checks before graph allocation, a cache of recent dominators, and
batches of 256 pairs. **Every new variant's complete result was compared with
the baseline**, including labels and multiplicities, not just row counts. The
original shared-input comparison additionally checks old/new normalized equality.

### Threading

Both limits were set to 10: `RE_NUM_THREADS=10` and `RAYON_NUM_THREADS=10`.
The old native maximizer creates **10 generation and 10 pruning workers**, plus
coordination threads; the new one shares a **single Rayon pool of 10**, with
global pruning on the calling thread. Both run on the same 10 logical CPUs.
Thus the original `test_all()` is multithreaded through its native maximizer,
although the test driver itself executes the problem sequence consecutively.
`test_all()` does not select a worker count: the old maximizer uses the event
handler's limit, then the environment variable, then the CPU count. Its wasm
implementation is serial. A native worker limit of 1 still creates one worker
for each of the two stages.

## 1. Matching: missing pruning before constructing candidates

Both versions combine 2,738 parent pairs for this snapshot. The old code
rejects dominated two-by-two exchanges in the intersection pairing before
constructing a candidate configuration. The new code constructs those candidates
and later invokes general containment tests to reject them.

| Matching diagnostic count | Old | Current new | New with exchange pruning |
| --- | ---: | ---: | ---: |
| Constructed candidates | 10,117 | 345,625 | 10,201 |
| Pair-local containment checks | 1,000 | 151,133 | 13,026 |
| Actual flow calls, local and global | 12,571 | 89,462 | 14,656 |

The prototype precomputes dominated exchanges for each parent pair. If two
occupied cells can be replaced by the crossing cells, producing larger
intersection values with the same row/column counts, the original pairing can
be skipped. The union value is unchanged. This uses the existing partial-order,
meet/join and upward-closed admissibility contract; it is not tied to string
labels or to ordinary problems without inputs.

Exchange pruning alone reduces the median from **256.0 to 96.2 ms**. Adding
larger batches reduces it to **67.8 ms**; the combined prototype reaches 63.1 ms.
On the large fixture no exchange qualifies, so this optimization contributes
no pruning there.

## 2. Large fixture: dominance filtering, rather than extra candidates

The versions visit the same **741,382 parent pairs** and construct the same
**945,393 candidates** on the fixed large input. The new alignment enumerator
already avoids impossible intersections efficiently: it visits 945,393 feasible
alignments, while the old one considers approximately 394 million matrices
before its filters. Replacing this enumerator would miss the actual bottleneck.

The new run spends **28.32 of 39.01 seconds in serial global merging**, compared
with 10.42 seconds generating batches. It performs **127.39 million global
containment checks**, compared with **57.40 million** in the old implementation.
Only about **372,000** checks in either implementation reach a flow call.

### Reject impossible matches before allocating a graph

The matcher already checks that each offered value has enough compatible target
capacity. However, it builds the network and starts adding edges before reaching
that rejection. Most containment checks fail, so they repeatedly allocate and
discard graphs without running Dinic.

A prototype runs that necessary capacity test first. It preserves the same
containment-check counts and flow calls, while reducing the large run from
**39.01 to 19.34 s**, and serial merging from **28.32 to 8.98 s**. This is a
generic matching improvement, belonging in `algorithms/configurations/matching.rs`;
it does not require replacing petgraph or adding a maximization-specific matcher.

The prototype duplicates compatibility checks on matches that survive the
precheck. Production work should reuse useful compatibility data where justified
and preserve cancellation polling inside long loops.

### Try recently successful dominators first

The old pruning workers maintain a small cache of configurations that recently
dominated candidates. The new merger repeatedly scans its whole antichain.

A cache in the new merger reduces global checks from **127.39 to 51.31 million**,
and the large run from **39.01 to 23.60 s**, without changing the candidate
sequence. Merely reversing the scan order reduces checks only slightly and
takes 40.26 s. The benefit comes from remembering useful dominators.

Cache correctness relies on dominance transitivity: a cached configuration
removed from the antichain is still covered by a stronger surviving one.

## 3. Small batches repeatedly synchronize workers

With ten Rayon workers the current batch contains only twenty parent pairs.
The large fixture requires **37,072 batch dispatches and barriers**. Generation
and global merging happen consecutively, leaving the worker pool idle while
the calling thread filters results.

Increasing the batch size to 256 reduces dispatches to **2,900** and generation
wall time from **10.42 to 4.43 s**, without changing the candidate order or
number of containment checks. Total time becomes 32.23 s. Combined with the
capacity precheck and recent-dominator cache, generation and merging take
approximately 4.38 and 5.46 s, respectively: **10.08 s total**.

An uninstrumented scheduling diagnostic on the same large input took 31.03 s
with a one-thread Rayon pool and 37.76 s with ten threads. The latter consumed
44.70 s user CPU plus 14.90 s system CPU, versus 30.94 s plus 0.08 s with one
thread. Parallelism helps the matching snapshot, but frequent small dispatches
and serial merging limit its benefit on the larger one.

## Experiments that did not help

- Moving accepted-history lookups before local filtering slowed matching
  (293.5 ms). It removes candidates that could otherwise help reject other
  candidates locally, so more work reaches global pruning.
- A naive shared `Mutex<BTreeSet<_>>` of generated candidates slowed matching
  to 713.9 ms and did not improve the large fixture. Earlier deduplication needs
  a better design before adding shared synchronization.
- Preallocating graph storage alone improved matching only slightly (248.9 ms).
- Reversing the antichain scan was insufficient, as described above.

## Selected production changes

1. Move necessary capacity rejection before network allocation in generic
   matching, preserving cancellation and counted semantics.
2. Add exchange pruning to the generic maximizer, with its order-theoretic
   justification and tests for custom operations, empty values and derivations.
3. Add a small cache of recently useful dominators to the merger.
4. Amortize Rayon dispatch over larger batches. Choose batch policy using
   measured work and cancellation responsiveness rather than fixing 256 as a
   universal optimum.
5. Use parallel global pruning for substantial batches, with ordered insertion
   retained when collecting derivations, as described in the follow-up below.

These changes retain the current model and generic engine. Different label-set
storage can be assessed if a measured bottleneck remains. The original combined
prototype passes the existing **10 maximization tests** and **11
generic configuration tests**, including exhaustive languages, joint input-output
relations, custom lattices, empty values, cancellation, and worker-count agreement.
Wasm performance has not been measured in this investigation.

## Parallel global pruning follow-up

The user requested parallelizing the remaining global pruning. Two variants
were tried using the same bounded ten-worker Rayon pool:

- `parallel-global` rejects candidates against an immutable current antichain
  in parallel, with persistent caches of useful dominators for each task slot.
  Survivors are inserted serially in their original order, preserving the
  sequence of derivation notifications.
- `parallel-antichain` also computes removals in parallel: existing rows are
  compared with surviving new candidates, and new candidates with each other.
  The calling thread applies the resulting masks in a deterministic order.
  This computes the maximal antichain of the union in one batch. When an observer
  is present, it uses the first variant instead to retain intermediate admissions
  and their derivations.

Duplicate-history insertion and applying keep/remove masks remain serial;
expensive rejection and removal checks are parallel. There is no shared mutable
antichain or lock around all pruning. Cache locks are disjoint and acquired once
per task, then released after its chunk. Cached rows removed from the antichain
remain valid dominators by transitivity.

### Repeated comparison

The table uses **three-run medians for both snapshots**, with variant order
reversed on alternating samples. Parsing, source conversion and input cloning
remain outside timing, and complete maxima are compared after every run. Each
mode includes the previously tested capacity precheck, exchange pruning, and
dominator cache. The serial and parallel columns with 4,096 pairs use exactly
the same batch size to separate pruning benefits from batch-size benefits.

| One maximization | Serial, 256 pairs | Serial, 4,096 pairs | Parallel, 4,096 pairs |
| --- | ---: | ---: | ---: |
| Matching, step 10 | 62.54 ms | 62.07 ms | **53.18 ms** |
| Long-label fixture, step 2 | 10.161 s | 9.789 s | **6.425 s** |

At equal batch size, parallel pruning is approximately **1.17× faster** on
matching and **1.52× faster** on the large fixture. On the latter, global merge
wall time drops from **5.74 to 2.38 s**, while generation stays about 3.8 s.
The 4,096-pair batches require 186 dispatches, versus 2,900 with 256 pairs.

Independent cache streams and the frozen comparison snapshot perform more
containment checks: approximately 77 million globally versus 51 million serial.
Their parallel execution still reduces elapsed time. The same 945,393 candidates
are generated and the same complete final maximum is obtained. Bulk pruning
admits fewer temporary configurations, so `global_accepted` no longer describes
the same intermediate sequence as serial insertion.

### Why global pruning does not scale linearly with cores

The global stage includes substantial serial duplicate handling. Additional
timers on one follow-up run of the selected parallel variant give:

| Global stage component | Elapsed time |
| --- | ---: |
| Serial duplicate-history processing | 1.043 s |
| Worker dispatch, parallel checks and collecting results, both passes | 1.138 s |
| Other bookkeeping and applying masks | 0.122 s |
| Complete global stage | 2.303 s |

The total is consistent with the previous 2.377 s median. Duplicate processing
iterates 945,393 candidates, clones keys, and performs ordered-tree insertions
on the calling thread. Applying final masks itself takes only 0.008 s. Thus
about half the remaining global stage is serial history/bookkeeping, rather
than expensive dominance checking.

The parallel algorithm also performs approximately 77M containment checks versus
51M serial: each task learns a separate dominator cache, and checks a frozen
antichain rather than benefiting immediately from other candidates in the batch.
Parallel work is therefore not a ten-way division of the original instructions.
Tasks use equal candidate/row counts rather than equal predicted cost; dispatch
and waiting for the last task also limit scaling. Their individual contributions
have not been separately measured. Future optimization should address serial
history processing, cache effectiveness and task granularity before adding cores.

The follow-up record has `stage: "breakdown"`; it is a single diagnostic run,
not another three-sample comparison.

### Other trials and verification

Parallel rejection alone with 256-pair batches took about 8.9 s on the large
fixture; parallelizing removals too took about 8.4 s. Larger batches amortize
the additional dispatches: a 1,024-pair parallel trial took 6.86 s. Direct Rayon
collection was also compared with the existing progress dispatch helper, but
did not establish an additional improvement: 6.38 versus 6.28 s in single
4,096-pair trials. The repeated comparison uses the existing helper.

Complete results agree for every tested variant. **Twelve maximization tests and
eleven generic configuration tests pass** with the selected flags. The two new
prototype tests exercise parallel batch maximality, caches containing removed
dominators, duplicate history, empty values, cancellation, and equality of
ordered admissions in the derivation-compatible variant. Existing exhaustive
tests cover joint input-output relations and custom algebras, and verify
worker-count agreement.

The benchmark prototype remains in `parallel_pruning.rs`, injected by
`investigate.py`. Production now uses this approach for substantial native batches,
with bounded batches scaled to worker count and a 4,096-pair cap. No parallel
wasm performance claim is made.

After building the temporary libraries, reproduce the repeated comparison:

```sh
python3 crates/core/benches/re_comparison/compare_pruning.py \
  --samples 3 --workers 10 --output /tmp/re3-pruning-results.jsonl
```

The runner checks full equality, runs measurements sequentially, alternates
variant order, prints per-invocation medians, and appends records to its output.
Use `--case`, repeated `--variant`, `--workers`, or `--binary` for controlled
alternatives. [parallel-experiments.jsonl](parallel-experiments.jsonl) retains all
trials. Rows with `stage: "final"` are the three-sample comparison above;
other stages are exploratory runs.

## History and extra comparisons follow-up

Additional counters separate the work behind the 51M versus 77M global checks:

| Global containment checks | Serial, 4,096 pairs | Parallel, 4,096 pairs |
| --- | ---: | ---: |
| Recent-dominator cache | 5.20M | 5.38M |
| Full scan of existing rows | 36.37M | 46.32M |
| Removing old rows using new candidates | 9.75M | 20.07M |
| Comparing surviving new candidates with each other | Included in ordered insertion | 5.25M |

The serial loop immediately learns from each accepted candidate and removes
dominated old rows. Parallel workers compare a frozen snapshot, and the removal
pass tests old rows against new candidates that may themselves be dominated
later in that batch. These effects explain most additional comparisons; separate
cache streams alone do not explain the entire increase.

Three-run diagnostic medians for the additional trials were:

| Trial, on top of parallel pruning | Matching | Large fixture |
| --- | ---: | ---: |
| Baseline | 51.17 ms | 6.352 s |
| Concurrent history queries | 51.60 ms | 6.156 s |
| Promote successful cache entries | 51.64 ms | 6.313 s |
| Reduce the new batch serially before old-row removal | 54.86 ms | 6.310 s |

Reducing the new batch first cut checks to about **59.20M**, but its serial
reduction prevented a useful elapsed-time gain. Cache promotion likewise did not
establish a useful improvement. Single-run discovery trials found that checking
history before cloning saved copies but duplicated tree lookups (6.44 s), sorting
the remaining history added cost (6.33 s), and refreshing obsolete cache entries
added millions of comparisons (6.67 s). Those alternatives were left out.

Concurrent immutable history queries gave a small additional improvement on the
large fixture. Production enables them only when history has at least 4,096 rows
and the candidate batch at least 2,048 entries, avoiding the extra lookup on small
cases. Actual insertions remain ordered, preserving the first derivation for
within-batch duplicates. This retains ordered-only generic bounds: no new `Hash`
requirement or shared history mutation was introduced.

## Production integration and verification

Selected optimizations now live in the application:

- `configurations/matching.rs`: reject insufficient compatible capacity before
  graph allocation, then preallocate successful flow networks.
- `maximize/combination.rs`: prune dominated intersection exchanges before
  candidate construction; allocate optional derivations only when requested.
- `maximize/antichain.rs`: recent-dominator caches, parallel global rejection
  and removal, and ordered admissions when derivations are observed.
- `maximize/engine.rs`: bounded scheduling, closure frontiers and ordered history,
  with concurrent read-only queries for substantial histories.
- `parallel.rs`: shared worker-count and safe-dispatch helpers. Native uses the
  existing bounded pool; wasm keeps serial execution in its request worker.

There are no new mathematical search limits. The optional exchange shortcut
skips its quadratic precomputation above 256 compatible cells and falls back to
ordinary exact enumeration. Cancellation remains checked throughout matching,
generation, history and pruning. Optional derivation payloads are boxed internally
to keep untracked candidates compact; the public observer API is unchanged.

[production-results.jsonl](production-results.jsonl) records **uninstrumented,
three-sample** release runs, ten-worker limits, with measurements performed
sequentially. Shared-input comparisons alternate which implementation runs first.
Every step checks complete normalized maxima and aligned RE results after timing.

| Shared-input maximization total | Old, rerun | Production new | Speedup |
| --- | ---: | ---: | ---: |
| Ten matching steps | 302.19 ms | 131.90 ms | 2.29× |
| Two large-fixture steps | 26.387 s | 5.953 s | 4.43× |

The second large-fixture kernel alone takes 26.257 s old versus 5.882 s new.

| Complete raw RE sequence | Original new baseline | Production new |
| --- | ---: | ---: |
| Ten matching steps | 683 ms | 133.12 ms |
| Two large-fixture steps | 36.469 s | 5.613 s |

The original baseline is the earlier three-run measurement in the parent report;
the production column is the new three-run measurement. Kernel and complete-RE
experiments use differently numbered intermediate representations: their totals
must not be subtracted to estimate conversion overhead. These fixtures show an
improvement; they do not establish a universal performance ranking.

The workspace passes **121 core, 5 server and 2 wasm-host Rust tests**. New
tests cover batch maximality, removed dominator cache entries, ordered admissions,
large duplicate-history filtering and first derivations, zero counts, and
cancellation. Existing exhaustive tests retain custom algebras, empty values,
input-aware relations, and worker-count agreement.

The rebuilt wasm and release server also agree on all **18 maximization fixtures**
and **18 RE fixtures**, including cache reuse, repeated RE, zero/nonregular
degrees, impossible input mappings and condensed high counts. The four entry
variants pass wasm/server parity checks. The frontend has been rebuilt to include
the new wasm binary.

Reproduce the production comparison from the repository root:

```sh
python3 crates/core/benches/re_comparison/run.py \
  --threads 10 --samples 3 --modes maximize new-re \
  --target-dir /tmp/re3-target-linux --output /tmp/re3-production-results.jsonl
```

## Aggregate work versus fully serial execution

The user clarified that "energy" means total computational work. The new
`compare_work.py` runner measures the **uninstrumented production** maximizer
with one, four and ten Rayon workers, using the same two frozen inputs. One
worker disables both parallel generation and parallel pruning. Current scheduling
policies remain intact: serial native batches contain 512 pairs, four-worker
batches 2,048, and ten-worker batches 4,096. Thus this compares the actual
production paths, rather than forcing identical scheduling in all modes.

Runs are sequential, worker order reverses on alternate samples, and full maxima
are compared across every process. Matching repeats each kernel 30 times per
process to amortize process setup; the large fixture repeats once. Three-sample
medians of per-process averages are retained in [work-results.jsonl](work-results.jsonl).

**CPU time is user plus system time summed over all threads**, obtained by
differences in `getrusage(RUSAGE_CHILDREN)` around each synchronous child process.
It includes setup, result dumping and teardown, whereas kernel wall time excludes
those. Process wall time exceeds kernel wall time by only about 0.1–0.2% here.
CPU time measures occupied processor time, including runtime/kernel overhead;
it is not an instruction count or a count of mathematical operations. No
hardware energy or instruction counters are exposed by this VM.

| Fixed maximization input | Workers | Kernel elapsed | Aggregate CPU | CPU / serial | Elapsed speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| Matching, step 10 | 1 | 141.14 ms | 141.24 ms | 1.00× | 1.00× |
| Matching, step 10 | 4 | 64.58 ms | 170.63 ms | 1.21× | 2.19× |
| Matching, step 10 | 10 | 51.00 ms | 192.53 ms | 1.36× | 2.77× |
| Large fixture, step 2 | 1 | 8.308 s | 8.301 s | 1.00× | 1.00× |
| Large fixture, step 2 | 4 | 6.755 s | 16.544 s | 1.99× | 1.23× |
| Large fixture, step 2 | 10 | 5.808 s | 21.458 s | 2.58× | 1.43× |

The parallel path genuinely performs redundant comparisons. The earlier
operation counters on the large fixture show **77.01M global containment checks
versus 51.31M** with ordered serial insertion, approximately 50% more. Frozen
antichains, separate caches, old-row removal against later-dominated candidates,
and peer comparisons explain that redundancy, as detailed above. Those are
global-pruning counts from the earlier diagnostic, not total-operation counts
from this uninstrumented resource run.

CPU overhead exceeds that comparison-count increase. On the large fixture,
median system CPU rises from about **0.067 s to 5.512 s** between one and ten
workers, showing substantial operating-system overhead. This measurement does
not separately attribute that time to allocation, synchronization or scheduling.
Cache/memory effects can also increase processor time per comparison.

For matching, ten workers buy a substantial elapsed improvement for modest extra
work. For the large fixture, they finish about 30% sooner while using about 158%
more aggregate CPU. One worker currently minimizes total CPU on these inputs;
ten minimizes elapsed time among these measured settings. Four workers do not
establish a generally optimal compromise. Further work can target redundant
pruning and task/runtime overhead, or expose the speed-versus-work preference.
The earlier new-batch reduction trial reduced comparison counts without improving
elapsed time; its aggregate CPU effect has not been measured.

Reproduce with a freshly built release comparison binary:

```sh
python3 crates/core/benches/re_comparison/compare_work.py \
  --binary /tmp/re3-target-linux/release/re-comparison \
  --workers 1 4 10 --samples 3 --output /tmp/re3-work-results.jsonl
```

The binary is built by `run.py` using `--target-dir /tmp/re3-target-linux`.
The `resource-new` mode dumps its full result outside kernel timing; it adds no
instrumentation or dependencies to the production library.

## Reproduction and artifacts

`investigation/baseline/engine.rs` and `baseline/matching.rs` preserve the two
original kernels. The instrumentation script installs those into temporary
copies before applying trial flags, keeping the historical comparisons
reproducible after production changes. They are benchmark fixtures and are
never compiled into the application.

From the `round-eliminator-3` root, build instrumented **temporary copies**:

```sh
python3 crates/core/benches/re_comparison/investigate.py /tmp/re3-investigation --build
```

Run a fixed snapshot; the third positional argument is the number of kernel
repetitions. Use `profile-old` for the old library and `profile-new` for the new:

```sh
env RE_NUM_THREADS=10 RAYON_NUM_THREADS=10 \
  /tmp/re3-target-linux/release/re-investigation matching profile-new 3 \
  crates/core/benches/re_comparison/investigation/matching-10.json

env RE_NUM_THREADS=10 RAYON_NUM_THREADS=10 \
  RE_PROFILE_VARIANT=batch-256+capacity-first+recent+swap \
  /tmp/re3-target-linux/release/re-investigation test-all-second profile-new 1 \
  crates/core/benches/re_comparison/investigation/fixture-2.json
```

Set `RE_PROFILE_RESULT` to a file path to dump the complete new maximum for
comparison. Variant flags can be combined with `+`; see `investigate.py`.
That script fails if source instrumentation anchors no longer match. It adds no
dependency to the production workspace. Its hooks use thread-local counters
flushed once per combination, avoiding a contended atomic on each operation.

- [matching-experiments.jsonl](matching-experiments.jsonl): three runs per variant.
- [fixture-experiments.jsonl](fixture-experiments.jsonl): one run per variant.
- `generation_wall_ns` and `merge_wall_ns` measure calling-thread elapsed stages;
  `combine_sum_wall_ns` sums possibly overlapping worker durations, not CPU time.
- `parallel_eligible_batches` and `serial_eligible_batches` describe the work
  heuristic before any forced-serial override or dispatch fallback.
- `matches_local` counts containment checks while combining a parent pair;
  `matches_global` counts checks in global antichain insertion, including seeds.
  `flows` counts actual invocations after elementary rejection.
- Parallel pruning adds `prefilter_candidates`, `prefilter_rejected`,
  `prefilter_batches`, and `prefilter_chunks` for its first pass;
  `prefilter_wall_ns` measures that pass including deduplication and dispatch.
  `commit_wall_ns` measures serial insertion or parallel batch removal, depending
  on the selected variant. `pruning_sum_wall_ns` sums worker elapsed durations,
  not CPU time. Earlier records predate these counters.
- The breakdown run adds `dedup_wall_ns`, `pruning_dispatch_wall_ns` (summed
  elapsed dispatch/collection time of the two passes), and `mask_apply_wall_ns`.
- Matching CPU sample reports are in `new-matching-functions.txt` and
  `old-matching-functions.txt`, collected with GNU gprofng at 5 ms intervals.
  Sampling on the larger fixture missed substantial CPU time, so its diagnosis
  uses direct phase timers, operation counters, and process resource timings.
