# Old/new RE and maximization comparison

The latest maximization implementation and measurements are in the
[live-pruning report](investigation/live-pruning.md). Tables below retain the
historical comparison stages.

Measured on 2026-10-12 in the same aarch64 Linux environment, with 10 logical CPUs,
Rust 1.89.0, release optimization, `RE_NUM_THREADS=10`, and
`RAYON_NUM_THREADS=10`. The main comparisons use medians of three independent
sequence runs. No other benchmark runs concurrently.
The old maximizer creates ten generation plus ten pruning workers; the original
new maximizer used one shared pool of ten with serial global pruning. Both use the
same machine. These settings bound each implementation's stages, rather than
creating identical worker topologies.

## Fixtures

- Degree-10 bipartite maximal matching: active `M U^9 / P^10`, passive
  `M UP^9 / U^10`; ten consecutive RE steps.
- The exact second problem from the old `test_all()`, starting
  `(0a) (00b) (00c) (00d) (00e)`: 32 active degree-5 rows, 61 passive degree-3
  rows, 18 initial labels. The text is copied verbatim to
  [test-all-second.txt](test-all-second.txt). Measure two actual RE calls.
  The old `re(..., 2, ...)` test helper performs one Speedup plus maximization
  of its result, so calling that helper would measure a different scope.

## What is timed

| Mode | Scope |
| --- | --- |
| `old-re` | Independent old sequence using `Problem::speedup`, including maximization and set-label conversion. |
| `new-re` | Independent new sequence using `round_elimination::apply`, including maximization and set-label conversion; every new snapshot starts uncached. |
| `old-button` | Old button workflow: missing source partial diagram, Speedup, and `fix_problem(..., true, true)`. Initial Start preparation is outside timing. |
| `maximize` | Both maximizers on precisely the same passive family at each step of an old raw RE sequence. Input conversion and caller-side cloning are excluded. |
| `maximize-cleaned` | Optional maximizer comparison on common inputs from an old button sequence, with its cleanup outside timing. |

All measurements exclude parsing, JSON, HTTP, browser rendering, initial Rayon
pool construction, and result checks. They are native computation times for the
actions, rather than elapsed browser-click timings. Both libraries run with a
no-op progress consumer. The old maximizer's per-frontier thread construction
remains inside its timing; the new maximizer normally shares a pre-existing pool.

The old maximizer mutates an owned constraint; the new function returns a new
constraint. Each function's internal normalization, search, pruning, allocation,
and result construction are timed. The old `is_maximized` shortcut is disabled
for these kernel measurements, and no new cached maximum is supplied.
Implementation order is reversed on alternate samples, including inside each
maximizer comparison. Inputs have the same IDs, label choices, and counts within
each comparison; the new model deduplicates structurally identical rows.

After every paired maximization, normalize both results and assert equality.
Also build both complete RE results, align new labels by their source sets, and
assert equality of normalized constraints. Checks happen outside timing. The
independent sequences can choose different numeric label orders; this changes
search order and helps explain why their totals differ from the shared-input
kernel totals. Stored row counts also differ where deduplication or old GUI
cleanup removes redundancy. These are not differences in the paired RE result's
normalized mathematical constraints.

## Reproduce

From the RE3 root, with the old worktree in the sibling `round-eliminator` folder:

```sh
python3 crates/core/benches/re_comparison/run.py --threads 10 --steps 10 --samples 3
python3 crates/core/benches/re_comparison/summarize.py
```

The runner builds a separate temporary Cargo package linking both libraries;
the old library is not added to the production workspace or dependencies. Its
benchmark dependency lock is retained in [Cargo.lock](Cargo.lock). The old crate
uses `onlyrust` to avoid compiling MiniSat; SAT backends are unused by RE and by
the old preparation on these fixtures (none has passive degree 2).
No library source is modified. `--target-dir` and `--toolchain` select the build;
`--binary` reuses an already built comparison executable. `--case` and `--modes`
restrict the measurements, and `--steps` changes the matching sequence length.
The fixture always uses two steps. An external watchdog reports unfinished
benchmark steps separately; it never changes either algorithm's search.

[results.jsonl](results.jsonl) contains individual steps, sequence cumulative
times, input/output sizes, sample numbers, and environment metadata.
[summary.csv](summary.csv) contains medians and minimum/maximum times per step
and for completed sequences. Sequence totals are medians of per-run sums, not
sums of individual step medians. Optional `maximize-cleaned` discovery data has
one sample, explicitly distinguished from the three-sample main comparisons.

## Results

All values below are three-sample medians. Sequence totals are in seconds.

| Fixture | Timed work | Old (s) | New (s) | New / old |
| --- | --- | ---: | ---: | ---: |
| Matching, 10 steps | RE only | 0.296 | 0.683 | 2.31× |
| Matching, 10 steps | Button computation | 0.379 | 0.683 | 1.80× |
| Matching, 10 steps | Maximization only, shared inputs | 0.297 | 0.550 | 1.85× |
| Long-label fixture, 2 steps | RE only | 24.422 | 36.469 | 1.49× |
| Long-label fixture, 2 steps | Button computation | 24.840 | 36.469 | 1.47× |
| Long-label fixture, 2 steps | Maximization only, shared inputs | 24.314 | 40.107 | 1.65× |

### Matching per step

Values are milliseconds; no cleanup in the independent RE columns.

| Step | Old RE | New RE | Old max, shared input | New max, shared input |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2.038 | 0.037 | 1.841 | 0.026 |
| 2 | 7.827 | 1.226 | 7.735 | 1.153 |
| 3 | 8.020 | 2.155 | 8.523 | 2.052 |
| 4 | 8.533 | 2.423 | 8.709 | 2.939 |
| 5 | 8.521 | 6.691 | 8.670 | 6.868 |
| 6 | 9.636 | 9.681 | 9.066 | 9.044 |
| 7 | 21.633 | 35.797 | 19.217 | 27.764 |
| 8 | 24.518 | 39.382 | 24.344 | 40.395 |
| 9 | 92.741 | 306.480 | 93.101 | 205.171 |
| 10 | 115.976 | 280.327 | 116.605 | 253.940 |

### Long-label fixture per step

| Step | Old RE (s) | New RE (s) | Old max, shared input (s) | New max, shared input (s) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.119 | 0.028 | 0.109 | 0.029 |
| 2 | 24.306 | 36.441 | 24.182 | 40.078 |

The new implementation is faster on early/simple maxima but slower on the
later/harder ones. On shared inputs, the ten matching maximizations are 1.85×
slower in total, and the hard second fixture maximization is 1.66× slower.
The first fixture maximization is about 3.8× faster. These measurements identify
a real maximizer regression rather than just extra button work. Matching labels
grow from 3 initially to 59 after ten steps; the final raw active/passive row
counts are 52/41 in both versions. The long-label fixture ends with 48 labels
and 153 active rows in both versions; passive stored counts differ through
deduplication and cleanup, with paired normalized outputs verified equal.

The pure new RE total and its shared-input maximization total come from separate
runs on differently numbered intermediate representations. Do not subtract
these totals to estimate conversion overhead. No universal performance ranking
is claimed. The follow-up [investigation](investigation/README.md) identifies
missing early pairing pruning, costly failed containment checks, repeated
antichain scans, and small Rayon batches. It records controlled prototypes on
frozen inputs; the baseline sequence measurements above remain unchanged.

## After production optimizations

The selected improvements are now integrated, including parallel native global
pruning. A new three-run comparison is recorded in
[the production section](investigation/README.md#production-integration-and-verification).
Complete new raw RE takes **133 ms for ten matching steps** and **5.613 s for
two large-fixture steps**. On identical inputs, maximization totals are old/new
**302/132 ms** and **26.387/5.953 s**, respectively. Complete normalized maxima
and aligned RE outputs agree at every paired step. The original tables above
remain historical baselines; the new records are in
[production-results.jsonl](investigation/production-results.jsonl).

## Parallelism and total work

[The aggregate-work comparison](investigation/README.md#aggregate-work-versus-fully-serial-execution)
measures production maximization at 1, 4 and 10 workers on frozen identical inputs.
Ten workers take 36% more aggregate CPU on matching for a 2.77× elapsed speedup;
the large fixture uses 158% more CPU for a 1.43× speedup. Operation counters also
show redundant parallel pruning. CPU time is a work proxy, not a hardware energy
measurement. `compare_work.py` reproduces the measurements, and
[work-results.jsonl](investigation/work-results.jsonl) retains each sample.

## Basic fixed point and allocators

[The custom-algebra and allocator report](investigation/allocator-comparison.md)
compares the requested degree-seven Basic fixed-point saturation, including its
default lattice and empty values, and measures the effect of system, jemalloc
and configured mimalloc allocation on the ordinary matching and large inputs.
Allocator features affect only the comparison executable, equally for both
libraries. The old server's allocator was not selected by the earlier
system-allocator benchmark binary.
