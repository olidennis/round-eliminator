# One-time default-diagram seeding

Native GUI Loop now runs the ordinary default diagram's active saturation once
in its guided worker and reuses every bounded, replayable tracked derivation.
This includes intermediate configurations removed from the final constraint.
The independent diagram and unrestricted certificate workers are unchanged;
guided jobs now run in a [configurable shared worker pool](fixpoint-sat-parallel.md).
Default seeding still happens only once. No WASM or GUI edit is required.

The default lattice is constructed directly as the right-closed subsets of the
original diagram, using the same order and original-label mapping as the GUI's
ordinary default. The native constructor enumerates distinct subsets under a
node budget; it does not first build an unbounded exponential completion.
Tests compare it to the existing GUI construction, including equivalent labels.

Whole original configurations, occurrence permutations, and join pivots are
retained in an explicit acyclic derivation DAG. All imports are independently
replayed. Ordinary diagram-specific equality is never used as a symbolic
identity, and a certificate must still pass the unchanged `is_pred` oracle.

The default seed has additional bridge-archive capacity rather than competing
for the existing 256 ordinary game/feedback slots. Its full DAG and replayed
roots are also preserved outside the feedback working pool. That pool visits
four seed roots per turn in cyclic order, under its existing size/profile
limits. General proof hints remain optional and bounded. See the exact limits
in [the search documentation](fixpoint-sat.md#one-time-default-diagram-seed).

## Prior diagnostic on the supplied hard problem

`examples/fixpoint_sat/hard_nonexistence.txt` has a 20-node default diagram.
Full saturation took 16–18 ms in optimized, single-saturation-thread runs and
recorded 503 derived configurations. Nine match derived configurations in the
supplied 21-combination certificate (up to commutativity, idempotence, and
coordinate order). The game export matched only three. The final default
triviality witness itself was not one of these matched configurations.

These observations motivated the integration; they do **not** establish that
the new Loop automatically finds that certificate. Earlier one-minute runs
which merely sent the default DAG through the old bounded import policy did
not find it. Those diagnostics are preserved at
`/workspace/fixpoint-default-diagnostic.w8Etsz/`.

## Validation before guided-job parallelization

- 58 native SAT/game/proof/seeding/coordinator tests passed, including default
  lattice equivalence, merged-label occurrence preservation, retained dominated
  intermediates, full seed archive coverage, cyclic provenance rejection,
  bounded/partial seeding, rotation beyond the old 24-root sample, cancellable
  batch scheduling, and one-time GUI Loop startup with cancellation/joining.
- Seven GUI and DIMACS-validation tests passed.
- Optimized native Loop still found the two-color fixed point and returned a
  verified maximal-matching nonexistence certificate.
- A blind 60-second native Loop run on the hard input started default seeding
  exactly once, built 20 nodes, and retained all 503 extracted derived
  configurations. No known certificate or subderivation was supplied.
- That run did **not** find a certificate. Its best feedback compatibility
  score reached 8/10, compared with 7/10 in the preceding diagnostic runs.
  This is a heuristic score and a single run, not evidence of convergence or
  a demonstrated certificate-search speedup.
- Final bridge archive: 553 derived configurations; six repair jobs and 14
  cached feedback retries. Wall time 60.01 s; aggregate CPU time 180.07 s;
  peak RSS 916,964 KiB. The three search workers ran concurrently, with
  `RE_NUM_THREADS=1` for internal saturation.

Logs and machine-readable results: `/workspace/fixpoint-default-seeding.lQrvYK/`.
The release example executable SHA-256 was
`883f0698338e5216ad8fcd50dbe3c7eebd88da99f3f65361e8f0ddb296ce4699`.
The native server was not rebuilt or restarted; rebuild/restart it to use the
new library path from the GUI. No search processes were left running.
