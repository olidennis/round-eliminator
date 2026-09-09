# Parallel guided certificate search

Native Loop keeps its diagram/game search and its general certificate search,
and now distributes independent guided SAT jobs to a configurable worker pool.
One guided scheduler owns the fragment archive, feedback profiles, default
seed, lazy block-pair schedule, and retry queues. It sends different jobs to idle workers;
it does not run independent copies of the complete guided search.

Workers encode and solve fragment-combination, proof-growth, and internal
repair jobs. Idle solver instances can move between workers on retries, keeping
learned clauses. Partial results return to the shared archive and repair pool.
Whole-configuration replay and the final `is_pred` verifier remain unchanged.

## Configuration

- `RE_GUIDED_THREADS`: number of guided SAT workers, from 1 to 32. By default,
  the certificate CPU share minus the general proof and closure workers,
  clamped to 1–6. With the optional Gimsatul backend, Loop splits CPUs roughly
  half/half between diagram SAT and certificates. Ten CPUs give five diagram
  threads plus three guided, one general proof, and one closure worker.
  See [the current CPU allocation settings](fixpoint-sat.md#parallel-diagram-sat-and-cpu-allocation).
- `RE_GUIDED_MAX_VARIABLES`: shared guided variable-credit budget, default
  1,500,000 and minimum 152,000. Each live job reserves 152,000 credits, covering
  its encoding and feedback goals. Cached solvers count their actual variables
  against the same budget. Under memory pressure, cached jobs are resumed
  before fresh jobs are admitted; some worker slots may remain idle. Cached
  credits are removed before reserving the resumed job. Solvers are not evicted
  and there are no cold retries.
- `RE_NUM_THREADS` controls saturation in other native operations. Loop caps
  each certificate worker's nested saturation at one worker. Default-diagram
  guided seeding still runs once before the guided pool starts.

For example, from `round-eliminator-server`, rebuild/start the native server:

```sh
RE_GUIDED_THREADS=6 cargo run --release
```

No GUI or WASM changes are needed. One guided worker uses the same scheduler
and verifier as larger pools; diagram, general proof, and closure workers remain
independent.

The variable budget is **not an exact byte/RSS limit**: clauses, proof terms,
oracle caches, and the independent searches also use memory. The old separate
bridge/feedback cache caps (300,000/150,000 variables) have been removed.
The total working set also has a count limit of 16 jobs per configured worker.
Each pool job is limited to
152,000 variables; hitting the limit is locally inconclusive, never a global
mathematical conclusion. The unrestricted proof worker has not been bounded
by this change.

## Archive coverage and bounded hot work

Original inputs and all bounded default-diagram seeds are pinned. The other
256 archive slots rotate, so newly generated fragments remain eligible after
the archive fills. Jobs own their concrete terms; rotation cannot change a
running or cached SAT instance.

Six-fragment blocks are paired lazily using circle-method matchings: every
round touches every block in the sweep's snapshot, and a sweep covers every
pair from that snapshot (subject to the existing fixed-subterm cap). Newly
added blocks join the next sweep; a separate changed-block lane gives imports
priority without restarting the breadth sweep. Pair-version deduplication and
at most one pending priority entry per block replace the growing FIFO queue.

Fresh bridge jobs start at one new derivation step, then deepen to at most
eight. Every retry keeps learned clauses and solved-bound flags. A job retires
locally after at most 16 slices, with one bounded SAT call per slice (grow the
bound first, then cycle over unresolved bounds); feedback retries retain their existing
32,000-conflict cap. These finite accelerator lifetimes free space for fresh
work instead of monopolizing a memory-bounded working set. Retirement is
inconclusive, never a global nonexistence or existence claim. The independent
unrestricted certificate grammar and final verifier are unchanged.

Progress now reports fresh/hot bridge counts, default-seed coverage, newly
selected fragments, archive rotations, pending changed blocks, and shared
cached jobs/variables. These distinguish useful coverage from CPU utilization.
The new-fragment counter counts selected admission versions, not globally
distinct expressions: an evicted expression can be admitted again.

## Earlier scheduling-fix validation (before expression closure)

72 native SAT/game/proof/coordinator tests and seven GUI/validation tests pass.
New regressions cover first-round archive coverage, all cross-block pairs,
continuous imports without resetting breadth, priority scheduling, bounded
rotation with pinned seeds, cancellation before lazy scheduling, shared cache
credits beyond the old per-family cap, retained UNSAT bounds, one SAT call per
bridge slice, local work retirement, and cached-to-live transfers under a
one-slot memory budget with four configured workers. All certificate results
still go through independent replay and the unchanged final verifier.

An isolated 180-second release run on `hard_nonexistence.txt`, without the
known certificate or its subderivations as input, gave:

- No certificate within the time limit; the diagram worker reached 13 nodes.
- All 503 default fragments were retained once; the bridge search actually
  selected 455 of them, plus 218 new-fragment admission versions.
- At least 78 fresh bridge jobs and 268 hot retries were dispatched. There is
  no cold-retry path. Already-proved UNSAT bounds remain cached and skipped.
- 237 archive replacements occurred after filling the ordinary slots. The
  pending changed-block lane held 44 entries at the last progress report,
  instead of an eager FIFO of thousands of concrete combination jobs.
- At least 592 total guided slices completed. The last cache report held 47
  idle jobs using 587,380 variables under the unchanged shared credit budget.
- CPU utilization averaged 797.8%; peak RSS was 2,403,904 KiB (2.293 GiB).
  The best partial compatibility score was 9/10, not a certificate or a
  distance-to-solution guarantee.

The earlier diagnostic's bridge search selected only the first 108 archived
tuples in its two-minute instrumented run and had 20,891 fresh jobs queued.
These different-duration, single runs demonstrate improved coverage and
elimination of cold retries, not a measured certificate-discovery speedup.
An intermediate implementation with multiple SAT calls per work slice reached
only 317 default seeds in three minutes; the final implementation uses one
bounded SAT call per bridge slice.

The final release also returned a checked four-node two-color fixed point and
verified maximal-matching nonexistence certificates with both one and six
guided workers. Tests/build logs: `/workspace/fixpoint-scheduler-fix.Jus4X2/`.
Final benchmark and smoke logs: `/workspace/fixpoint-scheduler-final.Q5c0wf/`.
Executable SHA-256:
`b574e0f3da11c26d2f6474aa795bdf20162a6a314567751cf644a5c2f807fa1e`.
The native server was not rebuilt/restarted; rebuild/restart it to use these
changes from the GUI. No search/test processes were left running.

## Cancellation and progress

Every live solver has a distinct, dynamically allocated interruption slot.
A registration is cleared before the solver can move or be destroyed. Pool
shutdown interrupts and joins all its jobs. Pool-local cancellation does not
cancel the independent workers before the outer coordinator receives the
winning result. Global GUI STOP or a winning independent worker cancels the
pool as well. Scoped guards cover errors and callback panics.

Startup prints `Proof: guided workers` and `Proof: guided shared variable
budget`. Progress includes peak busy workers, completed-job counts, and worker
numbers on SAT messages. CPU utilization and completion speed need not scale
linearly: job sizes differ, the archive has a shared scheduler, and the memory
budget can limit concurrency.

## Historical parallel-pool validation (before the scheduling fix)

- 64 native SAT/game/proof/pool/coordinator tests and seven GUI/validation
  tests passed. Tests exercise simultaneous live native solvers, duplicate-slot
  rejection, pool-only versus global cancellation, callback-panic joining,
  one/four-worker certificate discovery, bounded exhaustion, and a shared
  credit budget that permits only one of four workers to run at a time.
- Optimized Loop returned the checked two-color fixed point and a verified
  maximal-matching certificate with both `RE_GUIDED_THREADS=1` and `6`.
- A 60-second hard-case run with automatic configuration selected six guided
  workers, reached six simultaneously busy guided slots, seeded all 503
  default-diagram fragments once, and averaged **791.5% CPU** (475.08 CPU
  seconds / 60.02 wall seconds). Peak RSS was 1,542,992 KiB, about 1.47 GiB.
- It completed at least 272 guided job slices, filled the 759-fragment bridge
  archive, and reached a best compatibility score of 7/10. It **did not find a
  certificate**. A preceding sequential run reached 8/10; these scores are
  heuristics, not distances to a proof. Parallel result arrival changes search
  order, so greater throughput does not imply better scores or faster discovery.

The release run overlapped part of the final validation run, so it is a
concurrency/CPU-utilization smoke test, not an isolated solver-speed benchmark.
Artifacts: `/workspace/fixpoint-parallel.63ldpG/`. Executable SHA-256:
`18441d0bed30a777df309f8db941932dc70cfd5a0b4e1837180253ad3c4f1e21`.
The native server was not rebuilt/restarted; rebuild and restart it to use the
updated library from the GUI. No test/search processes were left running.
