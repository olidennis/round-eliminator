# Hard-case certificate SAT benchmark

Run date: 2026-09-08. This benchmarks discovering a nonexistence certificate
from the original problem, not finding a good diagram or running the whole GUI
Loop. No application solver has been replaced.

## Instance and positive witness

Input: [`hard_nonexistence.txt`](../round-eliminator-lib/examples/fixpoint_sat/hard_nonexistence.txt).
The user's supplied certificate is stored in
[`known_certificate.txt`](../round-eliminator-lib/src/algorithms/fixpoint_sat/proof/known_certificate.txt).
Sharing repeated whole-configuration derivations gives a 21-combination-step
DAG using only the five original active configurations as leaves. The bound
is sufficient, not proven minimal.

The unrestricted DIMACS file has:

- 751,178 Boolean variables;
- 2,940,191 clauses;
- 58,195,334 bytes;
- SHA-256 `c1b684d602aeaab2867cfea07f7142942e4ea8ec104d816ce4c7505c328a0440`.

The exporter first builds that unrestricted formula, then fixes the known
derivation's choices **only as assumptions in a validation solve**. The resulting
assignment satisfies every recorded clause. Independent replay also confirms
that it is an active derivation accepted by the unchanged nonexistence oracle.
A separate JavaScript checker verifies every clause in the actual DIMACS file
against that assignment. These checks establish that the exact benchmark
instance really is SAT, rather than merely assuming the bound is sufficient.
A second export without reading the known certificate was also byte-for-byte
identical (`proof-21-unhinted.cnf`).

The benchmark solvers receive only `proof-21.cnf`. They are not given the known
assignment, its parent/permutation choices, intermediate expressions, learned
clauses, or constraints on smaller bounds. Finding a different certificate is
allowed. This preserves the discovery problem.

## Measurement setup

Linux aarch64 environment, Apple CPU implementer, 10 logical CPUs, about
7.75 GiB RAM. Solver runs are sequential and pinned to CPU 0. Wall-clock limits
include parsing, preprocessing, search, and any model output; encoding and
subsequent verification are outside the limit. GNU `time` records user/system
CPU time and peak resident memory. GNU `timeout` sends SIGINT at the deadline,
with SIGKILL after a further five seconds if necessary.

- Existing native backend: MiniSat core 2.2 through the unchanged
  `rustsat-minisat` 0.3.1 dependency (`rustsat` 0.5.1). Its C++ library is a
  Release build with `-O3 -DNDEBUG`. The Rust file-loading/example wrapper uses
  the debug profile; its separate CNF loading time is reported in stderr.
- CaDiCaL 3.0.1, official tag `rel-3.0.1`, commit
  `c60730422e758ef1cebe7aeddf2dda31c996bf04`, g++ 13.3.0, `-O3 -DNDEBUG`.
- Kissat 4.0.4, official tag `rel-4.0.4`, commit
  `8af8e56f174b778aef3aa45af9f739b2a5f492c2`, gcc 13.3.0, `-O3 -DNDEBUG`.

Both additional solvers are standalone local builds from their official
repositories; neither was added to Cargo dependencies. All default runs use
the solver's default configuration and seed. A single run per configuration
is a diagnostic, not a statistical solver ranking. Batch results do not fully
predict performance in the application's incremental, cooperating searches.

## Results

Five configurations, 180-second wall-clock limit each:

| Solver | Result | Wall time | CPU time | Peak RSS |
| --- | --- | ---: | ---: | ---: |
| MiniSat core 2.2 | Timeout; no solution | 180.01 s | 179.97 s | 1,245.6 MiB |
| CaDiCaL 3.0.1 | Timeout; no solution | 180.01 s | 179.93 s | 816.8 MiB |
| Kissat 4.0.4 | Timeout; no solution | 180.00 s | 179.89 s | 526.8 MiB |
| CaDiCaL 3.0.1 `--sat` | Timeout; no solution | 180.02 s | 179.83 s | 838.9 MiB |
| Kissat 4.0.4 `--sat` | Timeout; no solution | 180.00 s | 179.91 s | 603.8 MiB |

MiniSat's debug Rust loader took 13.01 seconds, leaving about 167 seconds of
the budget for native solving. CaDiCaL parsed in 0.72 CPU seconds, Kissat in
0.39. Peak memory includes loading as well as search. These are end-to-end
fixed-file timings, not a clean comparison of solver-loop speed.

CaDiCaL reached 148,176 conflicts and Kissat 166,638. Both were actively
searching, not stuck in parsing or CNF generation. Conflict counts are not
directly comparable measures of progress between different solvers.

None of these independent searches found a certificate within its budget.
This does not show that any of the solvers would fail given more time, or
that they are equally fast. It does show that the difficulty is not solely
an insufficient proof bound: the tested formula demonstrably has a solution.
Changing the solver or selecting its SAT-focused preset did not resolve this
instance in these runs. The known witness is a validation result, not an
independent discovery success. No parallel SAT portfolio was benchmarked.

The known 21-step derivation has depth 8 (also checked in the regression test).
This makes a depth-8 restriction, or a partially fixed proof shape, useful
future experiments whose satisfiability can again be checked with the known
trace. Those restrictions were not applied in this benchmark.

A subsequent [Gimsatul/depth/symmetry comparison](fixpoint-sat-gimsatul.md)
tests these restrictions with four parallel solver workers.

## Artifacts and reproduction

Artifacts for this run are in `/workspace/fixpoint-sat-benchmark.ffW6pG`:
`proof-21.cnf`, `known.model`, solver sources/binaries, and per-run `.json`,
`.time`, `.stdout`, and `.stderr` files. Successful independent searches also
produce a checked `.certificate` file. Empty MiniSat model files after timeout
are not solutions.

Build the native exporter and create the CNF using the commands in
[the export documentation](fixpoint-sat.md#exporting-a-known-satisfiable-benchmark).
The exporter refuses to overwrite existing output files.

For a fresh artifact directory, from `round-eliminator-lib`:

```sh
BENCH="$(mktemp -d)"
RE_NUM_THREADS=1 target/debug/examples/fixpoint_certificate_cnf export \
  examples/fixpoint_sat/hard_nonexistence.txt 21 "$BENCH/proof-21.cnf" \
  src/algorithms/fixpoint_sat/proof/known_certificate.txt "$BENCH/known.model"
git clone --depth 1 --branch rel-3.0.1 https://github.com/arminbiere/cadical.git "$BENCH/cadical"
git clone --depth 1 --branch rel-4.0.4 https://github.com/arminbiere/kissat.git "$BENCH/kissat"
(cd "$BENCH/cadical" && ./configure && make -j3)
(cd "$BENCH/kissat" && ./configure && make -j3)
```

From `round-eliminator-lib`, after builds have finished:

```sh
node examples/fixpoint_sat/benchmark.cjs "$BENCH" \
  target/debug/examples/fixpoint_certificate_cnf 180
node examples/fixpoint_sat/benchmark.cjs "$BENCH" \
  target/debug/examples/fixpoint_certificate_cnf 180 cadical-sat kissat-sat
```

Optional solver names after the limit select individual configurations:
`cadical`, `kissat`, `minisat`, `cadical-sat`, `kissat-sat`. The last two pass
the solver's built-in `--sat` configuration. Existing result files are never
overwritten. The runner uses Node.js, GNU `time`/`timeout`, and `taskset`.

If a solver reports SAT, the runner checks every literal/clause in the actual
file and uses `verify` to decode and independently check the universal
certificate. An unexpected UNSAT result is treated as an error because the
known assignment has already established satisfiability.

## Regression checks

The 29 SAT/game/search Rust tests passed, including new tests for the 21-step
known derivation plan, known assumptions leaving the CNF unchanged, rejection
of invalid models, and bounded UNSAT not being called a universal certificate.
The actual-DIMACS JavaScript checker test and the three existing GUI label
mapping tests also passed. `git diff --check` passed. Solver compilations and
Rust tests ran outside the measured solver intervals.
