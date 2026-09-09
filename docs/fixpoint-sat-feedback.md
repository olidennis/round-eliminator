# Partial-proof feedback and internal certificate repair

Native **Loop** includes these mechanisms inside its guided search. Guided jobs
now run in a [configurable shared worker pool](fixpoint-sat-parallel.md); the
measurements below predate that parallelization. No new button, solver
dependency, mirror relation, or WASM change was added. The independent diagram
and unrestricted certificate searches remain available and retain their
conclusive outcomes. Restart/rebuild the native server to use the new code in
the GUI.

## Implementation

- Every fragment retains an explicit, shared derivation DAG back to whole
  original node configurations. Replaying it checks parents, pivots, and
  bijective occurrence permutations. Oversized fragments are skipped rather
  than treated as valid independent labels.
- Small SAT jobs first request a full certificate, then try partial
  compatibility goals. Valid incomplete derivations and their intermediate
  configurations feed back into future synthesis. Scheduling includes both
  improvement-oriented and weaker exploratory goals; improvement is not a
  prerequisite for keeping a useful subproof.
- Repair jobs replace the parent/permutation choices at one to three internal
  combination steps. All other steps remain wired, with their original pivot
  coordinates, and affected ancestors are rebuilt symbolically. This is not
  limited to appending steps above an immutable expression.
- A rotating 96-fragment pool retains multiple distinct proofs per
  compatibility/depth profile. Profiles use original-label probes as well as
  within-configuration compatibility. They are heuristics, never equivalence
  proofs. The separate bridge archive now pins default seeds and rotates its
  ordinary game/feedback slots; see [pool scheduling](fixpoint-sat-parallel.md).
- Unfinished repair/feedback solvers retain learned clauses in a bounded cache
  and receive larger conflict slices, interleaved with fresh jobs. Older bridge
  slices are capped so they cannot monopolize the guided thread indefinitely.
- Game witness export additionally samples previously omitted decompositions
  using already winning positions or immediate original-input leaves. It does
  not change the fast game's selected strategy, reachability search, or blocker.

Resource limits are documented in [the search description](fixpoint-sat.md).
All partial and complete SAT models are decoded into derivations and replayed;
a certificate is returned only after the unchanged `NonexistenceOracle`
(`is_pred`) accepts the result. Local UNSAT, time slices, pool eviction, and
resource exhaustion never imply a global answer.

## Controlled reconstruction of the supplied certificate

The supplied hard-case certificate has a 21-combination-step DAG, with five
original configurations and maximum depth eight. The test leaves its
surrounding DAG fixed and hides the indicated internal steps' parent and
permutation choices. Only original configurations are SAT leaves: neither the
completed certificate nor its derived expressions are supplied as extra seeds.

| Hidden internal steps (DAG indices) | SAT variables | Solver time | Result |
| --- | ---: | ---: | --- |
| 24 | 5,639 | 0.001 s | Verified certificate |
| 23, 24 | 17,743 | 0.126 s | Verified certificate |
| 22, 23, 24 | 38,571 | 0.449 s | Verified certificate |
| 5 (an early step) | 2,540 | 0.001 s | Verified certificate |

These are single diagnostic runs, using the existing native MiniSat backend
with a 100,000-conflict cap. Times exclude encoding and independent replay.
The surrounding Rust tests use the debug profile. This establishes that local
repair can reconstruct missing parts in these known-good contexts, **not**
that it can independently discover those contexts.

Reproduce from the repository root:

```sh
RE_NUM_THREADS=1 cargo test --manifest-path round-eliminator-lib/Cargo.toml \
  repairs_masked_internal_steps_of_the_supplied_certificate -- --nocapture
```

## Validation and running the blind search

The 48 native SAT/game/proof tests pass, including new tests for masked
reconstruction, partial-model reuse, profile/oracle agreement, occurrence and
nonzero-pivot preservation, equal-profile diversity, omitted game splits,
invalid/cyclic provenance, and bounded/cancelled work. The seven GUI/API and
standalone DIMACS-checker tests also pass. Logs from development are in
`/workspace/re-native-deps.xPzJqP/sat-tests-verified.log`.

Build and run the actual native Loop with no known-certificate input:

```sh
cargo build --release --manifest-path round-eliminator-lib/Cargo.toml --example fixpoint_sat
round-eliminator-lib/target/release/examples/fixpoint_sat \
  round-eliminator-lib/examples/fixpoint_sat/hard_nonexistence.txt --parallel
```

This command has no overall timeout. To measure a bounded run, create a fresh
output directory and use `examples/fixpoint_sat/guided-benchmark.cjs` as described
in [the guided-search report](fixpoint-sat-guided.md). That script never reads
the known-certificate fixture, and records the executable hash, complete logs,
resource usage, fragment counts, feedback scores, and cached retries.

The displayed compatibility count is only a heuristic. In this degree-four
case, all ten independent pairs (six off-diagonal, four diagonal) must be
compatible. A score such as 7/10 does not predict how much search remains.

## Blind hard-case benchmark: still not solved

On 2026-09-09, the final optimized native Loop was run for 180 seconds with
only `hard_nonexistence.txt` as input, on the same Linux/aarch64 environment
as the earlier benchmarks. `RE_NUM_THREADS=1` limits internal saturation;
all three search workers run concurrently, without CPU pinning. Compilation
and the reconstruction tests were outside the measured interval.

| Measurement | Result |
| --- | ---: |
| Outcome | Timeout; no certificate found |
| Wall time | 180.01 s |
| Combined CPU time | 540.15 s |
| Peak process RSS | 1,071.9 MiB |
| Distinct fragments retained in the bridge archive | 220 |
| Current feedback pool size | 96 |
| Cumulative feedback admissions (including replacements/re-admissions) | 314 |
| Best observed feedback compatibility score | 7/10 |
| New internal-repair jobs | 17 |
| Cached repair/feedback retries | 44 |
| Repair/feedback SAT solves logged | 93 |
| Repair/feedback SAT variable counts observed | 638–29,110 |
| Largest diagram size reached | 13 |

Artifacts: `/workspace/fixpoint-feedback-final.7ysKeg/`, including `loop.json`,
complete stdout/stderr, timing, and executable SHA-256
`123b85c568d51cb4a413d9b8873f411e2daf0ab4b0a55ee3953bc3cca8026efc`.
The known certificate was not supplied. A preliminary 180-second run before
cached retries and the bridge-slice cap also timed out; its artifacts are in
`/workspace/fixpoint-feedback-benchmark.rpo4Yp/`.

These runs establish that feedback, repair, and retries are exercised in the
real Loop path. They do **not** establish faster independent certificate
discovery. The hard case remains unresolved in this time budget. The masked
reconstruction results above must not be described as blind rediscovery.

The final native parallel path was also checked on the two-color example
(verified, renamed fixed point) and maximal matching (verified nonexistence
certificate); both completed within the 20-second smoke-test limits.
