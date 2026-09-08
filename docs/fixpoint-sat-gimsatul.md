# Gimsatul, depth bounds, and symmetry breaking

Follow-up to the [single-threaded benchmark](fixpoint-sat-benchmark.md),
2026-09-08. All instances search for a nonexistence certificate using 21
combination steps and only the five original active configurations as leaves.
This does not change the GUI search, native solver dependency, or `is_pred`.

## Controlled changes

1. The exact original CNF, unchanged.
2. Original CNF plus maximum derivation depth 8.
3. Depth 8 plus symmetry breaking.

Depth is parent-chain length; original configurations have depth zero. The
encoding assigns bounded monotone unary ranks to intermediate steps, and every
selected parent edge forces the child's rank strictly below the output's.
Every actual depth-bounded DAG satisfies these constraints by using its actual
depths as ranks. Conversely a path longer than the bound contradicts the rank
constraints. The decoded parent-chain lengths are also checked independently.
The bound applies to all encoded steps, not only the final step's ancestors.
Unused steps can be chosen shallowly. No particular layer widths or wiring
from the known proof are imposed.

The symmetry pass adds two kinds of constraints:

- Order the left parent's three nonpivot source positions increasingly. Both
  inputs can be jointly reordered because these three output coordinates all
  use meet. Every subsequent use has its own free occurrence permutation.
  This chooses one of the six possible meet-coordinate orders.
- For identical label occurrences in an original parent configuration, assign
  their source positions in increasing output-row order. These constraints
  are conditioned on selecting that whole original configuration, on both
  sides of each combination.

These cuts supplement the existing fixed join coordinate and ordered parent
IDs. They do not fix original-label choices, intermediate connections, or
compatibility values, and introduce no additional mirror nodes. This is not
an exhaustive implementation of all possible symmetries.

The restriction clauses are appended to the same base encoding. This makes
the formula slightly larger, while reducing admissible assignments; this
experiment does not rebuild a smaller circuit around the restrictions.

## Satisfiability checks

Each restricted formula has its own verified satisfying model constructed
from the known certificate. For the symmetry variant the proof's coordinates
and all later uses are consistently reindexed. Its 21-step, depth-8 derivation
then satisfies the symmetry clauses and the unchanged nonexistence oracle.
Only the validation solve receives these choices as assumptions. No known
model, structure, intermediate expression, or learned clause is passed to
Gimsatul. The JavaScript checker additionally verifies every clause in the
actual exported file against the known model before launching each benchmark.

| Variant | Variables | Clauses | File bytes |
| --- | ---: | ---: | ---: |
| Original | 751,178 | 2,940,191 | 58,195,334 |
| Depth ≤ 8 | 751,297 | 2,942,642 | 58,250,890 |
| Depth ≤ 8 + symmetry | 751,297 | 2,946,338 | 58,335,145 |

SHA-256:

- Original: `c1b684d602aeaab2867cfea07f7142942e4ea8ec104d816ce4c7505c328a0440`
- Depth 8: `4cfc676f503e5fb1f99d8e6aa51fc0b81258f2876a942dad05c708c0c5f24a2c`
- Depth 8 + symmetry: `f080b5a95f6773cb803a496785ac8736a9fc740f6dcb8405c6fe2540bb636aba`

## Solver and timing

Gimsatul 1.1.3, official repository commit
`4664fd74c97f87e30e7f907181707679b6fa49f2`, gcc 13.3.0, `-Wall -O3 -DNDEBUG`.
Its upstream `make test` suite passed, including multi-threaded cases.

Each run uses `--threads=4`, CPU affinity `0-3`, and a 180-second wall-clock
budget. The runs are sequential on the same Linux aarch64 environment as the
earlier comparison (10 logical CPUs, about 7.75 GiB RAM). Parsing,
preprocessing, solving, and model output are inside the time budget; encoding,
known-witness checks, and subsequent model validation are outside. Compilation
and Rust regression tests do not overlap the timed solver runs.

This is genuine shared-clause parallel search: four worker threads run on
the same formula and exchange learned information. GNU `time` records summed
process CPU time and peak process RSS, not per-worker peak memory. Parallel
search scheduling can affect results; one run per variant is diagnostic, not
a statistical speedup measurement.

## Results

| Variant | Result | Wall time | CPU time (all workers) | Peak RSS |
| --- | --- | ---: | ---: | ---: |
| Original | Timeout; no solution | 180.04 s | 657.19 s | 1,506.3 MiB |
| Depth ≤ 8 | Timeout; no solution | 180.04 s | 665.69 s | 1,504.4 MiB |
| Depth ≤ 8 + symmetry | Timeout; no solution | 180.04 s | 662.75 s | 1,234.6 MiB |

All three discovery runs timed out, despite each formula having a separately
verified satisfying assignment. Depth 8 alone did not change the outcome or
peak memory materially. Adding symmetry reduced peak memory by about 18% in
this comparison and allowed more variable elimination, but did not yield a
certificate within the time budget. These observations do not establish
solve-time speedups or failure given a longer budget.

The four-worker total conflict counts were 455,611 (original), 461,237
(depth 8), and 418,400 (depth 8 + symmetry). These are diagnostic statistics,
not percentages of the search space explored or proof of being closer to a
solution. Clauses are shared between workers, so conflicts are not independent
units of progress either.

No single-threaded Gimsatul control was run, so this does not isolate the
speedup from its parallelism versus its own sequential mode. It does test
whether switching to a genuine four-thread solver and then adding these two
restrictions is sufficient to solve the known-SAT instance in three minutes:
it was not in these runs.

## Reproduce

Artifacts, binaries, CNFs, known models, and raw per-run JSON/time/stdout/stderr
are in `/workspace/fixpoint-sat-benchmark.ffW6pG`.

To build the same Gimsatul commit in a fresh benchmark directory:

```sh
git clone https://github.com/arminbiere/gimsatul.git "$BENCH/gimsatul"
git -C "$BENCH/gimsatul" switch --detach 4664fd74c97f87e30e7f907181707679b6fa49f2
(cd "$BENCH/gimsatul" && ./configure && make -j3 && make test)
```

From `round-eliminator-lib`, build `fixpoint_certificate_cnf` as documented
in the [export instructions](fixpoint-sat.md#exporting-a-known-satisfiable-benchmark).
Generate the original `proof-21.cnf`/`known.model` using those instructions.
Generate the two variants (output files must not exist):

```sh
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf export \
  examples/fixpoint_sat/hard_nonexistence.txt 21 "$BENCH/proof-21-depth8.cnf" \
  src/algorithms/fixpoint_sat/proof/known_certificate.txt "$BENCH/known-depth8.model" \
  --depth=8
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf export \
  examples/fixpoint_sat/hard_nonexistence.txt 21 "$BENCH/proof-21-depth8-symmetry.cnf" \
  src/algorithms/fixpoint_sat/proof/known_certificate.txt "$BENCH/known-depth8-symmetry.model" \
  --depth=8 --symmetry
```

Run the matched comparisons:

```sh
node examples/fixpoint_sat/benchmark.cjs "$BENCH" \
  target/debug/examples/fixpoint_certificate_cnf 180 gimsatul --threads=4
node examples/fixpoint_sat/benchmark.cjs "$BENCH" \
  target/debug/examples/fixpoint_certificate_cnf 180 gimsatul --threads=4 --depth=8
node examples/fixpoint_sat/benchmark.cjs "$BENCH" \
  target/debug/examples/fixpoint_certificate_cnf 180 gimsatul --threads=4 --depth=8 --symmetry
```

The same flags work with `cadical`, `kissat`, or `minisat` instead of `gimsatul`
(those solvers still use one thread). Pass matching `--depth=8 --symmetry`
flags to the Rust `verify` command when checking a restricted model.

Validation includes 31 passing SAT/game/search regression tests. New exhaustive
small cases compare the depth constraints against all three-step parent DAGs
and check that every four-coordinate combination in the test problem has an
equivalent symmetry-normalized SAT representation. The independent DIMACS
model-checker JavaScript test also passed.
