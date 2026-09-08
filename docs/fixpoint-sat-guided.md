# Witness-guided certificate search: implementation and benchmark

Native GUI **Loop** now runs a third, witness-guided worker alongside the
diagram/game worker and the general certificate worker. No button, solver
dependency, extra mirror relation, or WASM change was added. The final
certificate checker is still the existing `is_pred`-based oracle.

The game exports explicit, finite derivation DAGs, including intermediate
winning positions and already-explored alternative winning splits. The new
worker independently replays these from whole original configurations. It
then searches small batches of concrete fragments, initially adding one to
three combination steps and gradually widening to eight. Original-only
bootstrap batches stop at three; deeper original-only synthesis remains the
general worker's job. Interrupted neighborhoods are retried, with a bounded
cache for solvers and learned clauses. See [the encoding and resource
limits](fixpoint-sat.md#witness-guided-small-sat-instances).

This implements short recombinations above fixed fragments. It does **not**
yet implement replacing an internal branch in a fixed game-proof topology,
or feeding partially successful SAT derivations back into fragment generation.

## Hard example: not solved yet

The optimized native Loop was run on `hard_nonexistence.txt` for 180 seconds,
without supplying the known certificate or any of its subderivations.

| Measurement | Final development run |
| --- | --- |
| Result | Timeout; no certificate found |
| Wall time | 180.01 seconds |
| User + system CPU time | 540.06 seconds |
| Peak RSS, entire process | 938.1 MiB |
| Imported distinct derived configurations | 21 |
| Largest local fragment batch | 12 |
| Largest additional-step bound reached | 8 |
| Local SAT variable counts observed | 638–115,545 |
| Largest diagram size being searched | 13 |

The small one-to-three-step instances are substantially smaller than the old
751,178-variable, 21-step standalone benchmark. However, these are different
search spaces: the old benchmark is known satisfiable, while a local fragment
batch is **not** guaranteed to contain a certificate. Smaller formulas alone
therefore do not establish faster certificate discovery. Several local bounds
were proved UNSAT; other bounds reached their conflict budgets and remained
inconclusive. Neither result is treated as a global answer.

Artifacts for this run are in
`/workspace/fixpoint-guided-benchmark.Kby1UD/`: `loop.stdout`, `loop.stderr`,
`loop.time`, and `loop.json`. The executable SHA-256 was
`d2c865cc3099621b7306ef3509bf3c355bdcbe23a64cc96e12716c9765dcf706`.
The machine is the same Linux/aarch64, 10-logical-core, roughly 7.75-GiB
environment as the preceding solver benchmarks. The three workers ran
concurrently, without CPU pinning; `RE_NUM_THREADS=1` limited internal
saturation parallelism. The elapsed time includes all three searches, not
only SAT solving. This is one development run, not a statistical comparison.

Reproduce from the repository root, using a new output directory:

```sh
cargo build --release --manifest-path round-eliminator-lib/Cargo.toml --example fixpoint_sat
mkdir guided-results
node round-eliminator-lib/examples/fixpoint_sat/guided-benchmark.cjs \
  round-eliminator-lib/target/release/examples/fixpoint_sat guided-results 180
```

The script refuses to overwrite existing results, records the executable
hash, and never reads the known-certificate fixture. The external benchmark
timeout terminates the process; GUI STOP is tested separately through the
coordinator's cooperative cancellation and native solver interrupters.

## Validation

- All 37 native SAT/game/proof/coordinator tests passed.
- New tests cover whole-configuration replay, invalid/cyclic parents,
  occurrence permutations, cross-batch coverage, local inconclusiveness,
  actual game-DAG export, and restricted certificate synthesis.
- The native interruption test now exercises all three solver slots.
- The known-certificate recombination unit test still finds a certificate
  with one new step from two proper subderivations. Those fragments are
  supplied deliberately in that unit test, **not** in the benchmark above.
- Optimized native Loop returned a verified nonexistence certificate for
  maximal matching and a checked, automatically renamed fixed point for the
  two-color example, both in milliseconds.
- All three GUI label-mapping tests and the standalone DIMACS-checker test
  passed. No new GUI control is required.

The implementation preserves both kinds of conclusive result, but the hard
case remains an unresolved performance problem. It would be incorrect to
describe this change as successful automatic rediscovery of its certificate.
