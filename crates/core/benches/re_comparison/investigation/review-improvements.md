# Maximization review experiments

This report records the earlier review stage. The later
[final-four report](final-four.md) supersedes its future-work and server-allocator
status: wider density-dependent masks and dynamic-mask history compression were
integrated, the server switched to mimalloc v2, and flow reuse and phase overlap
were tested without selecting them. The measurements below retain their original
meaning and were not rerun for this status update.

2026-10-13. This follows the static review in the root `REVIEW.md`. Only
maximization improvements were attempted. Other findings, especially incidence
edge coloring after one RE step, remain deferred.

## Method

Linux/aarch64, ten logical CPUs, Rust 1.89 release; mimalloc **v2**, option 26 = 0.
One measured child at a time, without builds, tests, servers doing work, or other
heavy jobs. Matching repeats 30 kernels per process; large and Basic use one.
Elapsed values exclude parsing/serialization; total CPU is child user + system
CPU, including every worker. Peak RSS is for the whole process, including fixture
and custom-algebra preparation. CPU is a work proxy, not measured energy.

All variants must agree on complete normalized maxima, not just counts. Native
production confirmations alternate before/after executable order across three
samples. The `before` executable is the previously integrated live-pruning
version; original RE1 timings are reused, not remeasured. The Basic driver runs
BOTH old-reference custom saturations, not UI lattice construction/cleanup.
The large fixture is its fixed second maximum; matching is the fixed step-ten
maximum. These are kernel timings, not full RE sequences or UI clicks.

`investigate_review.py` restores frozen pre-experiment kernels, keeping every
profile flag outside production. Screening records `review-first-*`,
`review-alignment-*`, and `review-combined-large` used per-query string flag
lookups; those costs varied with flag count. They informed screening but are
NOT used for final quantitative claims. Later `review-confirm-*`,
`review-prepare-large`, and `review-wide-large` use flags parsed once to bits.
The prototype baseline also moves useful-join discovery without its early return;
production confirmation compares genuinely unchanged before/after executables.

## Selected changes

- Find admissible incomparable joins before intersection/exchange preparation;
  return immediately when there are none. Preserve original trace order.
- Prepare conflict indices once per parent pair and reuse alignment margins,
  count buffers and heap frames across join choices.
- Before exchange preparation, filter join choices whose remaining row/column
  demand cannot balance in every connected component of admissible intersections.
  Uses petgraph UnionFind; passing this necessary condition still requires exact
  count-matrix enumeration.
- Cache up to 128 flow compatibility cells in a stack bitmask during the necessary
  capacity checks. Larger matrices retain ordinary graph construction and exact
  petgraph Dinic. No expanded occurrences or allocated compatibility table.
- Select exact shared generated-key history when the meet of all seed values is
  admissible, certifying dense future intersections. Each key is recorded once
  before local domination; local rejects are covered by parents/survivors and
  retained after survivor admission. Sparse workloads keep frozen batch history;
  traced runs retain their deterministic history and local traversal. No new
  Hash bound or hash-only rejection. More retained history uses more memory.
- Keep u64 for up to 64 labels, use u128 for 65–128, then fall back to vector sets.
  The public model and JSON remain unchanged. Add vector subset length rejection.

Broad shared history, including a per-pair dense gate, made the large fixture
slower and used more CPU in screening. It was rejected. Common-admissible-meet
selection preserves the Basic benefit while leaving the sparse large fixture's
history policy alone. The packed matching fixture also has a common label.

## Production results

Final integrated default targets **12 ms per pruning worker**, compared with
8 ms before these changes. The preceding 8 ms confirmation reduced absolute
CPU but left the large fixture at 32.9% extra work over its faster serial run.
Controlled 8/10/12 ms trials kept the elapsed gain; 12 ms reduced that overhead.

| Kernel | Before elapsed | Final elapsed | Before CPU | Final CPU | Final serial CPU | Extra parallel CPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Basic, both saturations | 7.690 s | 6.169 s | 54.919 s | 41.462 s | 33.521 s | 23.7% |
| Large, fixed second maximum | 1.266 s | 1.124 s | 4.360 s | 3.060 s | 2.386 s | 28.3% |
| Matching, fixed step ten | 17.160 ms | 15.376 ms | 55.100 ms | 51.011 ms | 31.139 ms | 63.8% |

Three parallel samples; matching amortizes 30 kernels each. Serial large/matching
use three samples; serial Basic uses **one** expensive sample. Serial records
precede the final parallel-only pruning adjustment; serial code is unaffected.
Whole canonical results also agree across these serial/parallel files.

The tiny matching kernel retains high relative parallel overhead (~64%), about
20 CPU ms extra; total CPU still decreased. This remains a scheduling opportunity.
The large fixture meets <=2 s and <=30% extra CPU; Basic is below 30% as well.
These workload measurements are not a universal bound on the scheduler.

Raw files: `review-production-{basic,large,matching}.jsonl` contain paired before
/8ms production runs; `review-final-*` contain the final 12ms policy. The old
RE1 reference timings are reused: Basic 27.496 s /272.056 CPU s, large 19.636 s
/191.012 CPU s, matching 103.219 ms /439.834 CPU ms. No old kernels rerun.

Peak RSS for the first production confirmation: Basic 32.26→36.16 MiB, large
67.96→72.13 MiB, matching 35.49→35.75 MiB (medians). Shared history trades memory
for matching work; do not assume memory growth stays this small on other algebras.
The production server still uses its system allocator; these measurements
use the configured mimalloc-v2 comparison executable.

## Wider alphabets

The synthetic wide large fixture duplicates each original label into two aliases
that always occur together (116 labels). This measures representation cost on a
related closure shape, not an independent 116-label application workload.
Bit-flag prototype medians: vector baseline 2.676 s / 10.440 CPU s; u128 alone
1.380 s / 4.813 CPU s; combined alignment + u128 1.197 s / 3.405 CPU s. Complete
maxima agree. Final production confirmation (three alternating samples each):

| Synthetic kernel | Before elapsed | Final elapsed | Before CPU | Final CPU |
| --- | ---: | ---: | ---: | ---: |
| large-wide | 2.424 s | 1.188 s | 9.284 s | 3.247 s |
| matching-wide | 33.948 ms | 15.791 ms | 156.527 ms | 51.660 ms |
The analogous matching fixture has 94 labels. Stored old problem sections are
unchanged driver scaffolding, NOT equivalent old-library inputs: use resource-new
only for these synthetic files. No claim about every density or >128 alphabets.

## Verification and organization

147 core + 5 server + 2 wasm adapter tests pass. Added prepared-search reuse after
an early visitor exit, component rejection over all 2x3 allowed-cell graphs,
bit 127/fallback/Hall-obstruction matching checks, 65/128-bit real closure vs vector
sets, a uniform-domination degree-three regression, and dense shared history vs
ordered traced closure. Existing exhaustive universal/custom-lattice tests remain.

20 maximization (including 128/129-label cases), 18 RE and 6 Basic FP fixtures
agree through both native server and wasm. Native server/wasm packages rebuilt;
frontend typecheck/build pass. Reusable alignment/component logic stays in
`algorithms/configurations/alignments.rs`, matching compatibility in `matching.rs`,
set subset logic in `model/labels.rs`; maximization policy/history stays in its
existing engine/combination modules. No new production layer or dependency.

Historical investigation builders also restore their old configuration exports,
maximization wrapper/signatures and label operations, so these changes do not
break or silently alter earlier prototype measurements.
The restored packed prototype passes `cargo check --release`; original and
intermediate builders also complete their profiling-anchor preparation.

## Reproduce

```sh
python3 crates/core/benches/re_comparison/investigate_review.py /tmp/re3-review --build
python3 crates/core/benches/re_comparison/compare_review.py --binary /tmp/re3-target-linux/release/re-review --case large --samples 3 --variants baseline early+reuse+components --output /tmp/review-large.jsonl
python3 crates/core/benches/re_comparison/investigate_review_policy.py /tmp/re3-review-policy --build
python3 crates/core/benches/re_comparison/compare_review.py --binary /tmp/re3-target-linux/release/re-review-policy --case large --samples 3 --variants baseline target10 target12 --output /tmp/review-policy.jsonl
```

Production: retain a copy of the before executable, build the normal comparison
with `run.py --allocator mimalloc-v2 --build-only`, then set
`RE_REVIEW_BEFORE_BINARY` and select `--variants before baseline`. To measure new
serial work, select `--workers 1 --variants baseline`. Each raw JSONL has adjacent
`.results` files with canonical/full maxima and per-trial peak RSS/binary hashes.

Still future work: reuse of flow networks, interning and wider/density-dependent
sets beyond u128, generation/pruning overlap, history compression when justified,
and other pruning-heavy workloads. No unsolicited solvability, diagram, lattice,
allocator, protocol or UI feature changes were made.
