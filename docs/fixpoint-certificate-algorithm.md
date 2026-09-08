# Extracting a priority algorithm from a supplied certificate

The native runner takes an existing nonexistence certificate; it does not
search for a diagram or rediscover the certificate. It now accepts the
`Original expressions` printed by native Loop even when their tree shapes
have been simplified independently. Before priority search it restores a
synchronized active derivation using only commutativity and idempotency,
then checks the active derivation, projection equivalence, and universal
certificate. Already-synchronized certificates keep their exact trees.
The automatic extraction call in GUI Loop remains disabled, and WASM is
unchanged.

Two bugs in the original helper were fixed: the second tree's root now uses
the second tree's index map, and the priority comparator returns `Equal`
when comparing an item with itself.

The standalone implementation searches the same fixed-priority scheme with
the existing native MiniSat dependency. It constant-folds the game formulas,
introduces only relevant order comparisons, and excludes cyclic orderings
incrementally. This avoids eagerly generating the original cubic set of
transitivity constraints. Returned schedules are independently checked
against all edge games, the tree precedences, and the meet/join precedences.

## Run

From the repository root:

```sh
cargo build --release --manifest-path round-eliminator-lib/Cargo.toml --example fixpoint_certificate_algorithm
round-eliminator-lib/target/release/examples/fixpoint_certificate_algorithm extract \
  round-eliminator-lib/examples/fixpoint_sat/hard_nonexistence.txt \
  round-eliminator-lib/src/algorithms/fixpoint_sat/proof/known_certificate.txt \
  algorithm.json 18000
```

The last argument is a time budget in seconds (here, five hours; default:
180 seconds). Progress goes to the terminal. SAT solving uses conflict
slices, so cancellation/time-budget checks happen between slices rather
than enforcing a hard process deadline. A verified schedule is saved only
on success, and existing output files are never overwritten.

To independently verify a saved schedule:

```sh
round-eliminator-lib/target/release/examples/fixpoint_certificate_algorithm verify \
  round-eliminator-lib/examples/fixpoint_sat/hard_nonexistence.txt algorithm.json
```

The JSON records the certificate and ranks indexed by color, port/expression,
and inorder arrow position. Higher ranks act first. The extractor uses
degree-plus-one colors. `--sat-only` disables the preliminary deterministic
schedule candidates and their SAT phase hints.

Saved algorithms include the original readable text as `source_certificate`
and the full reconstructed trees as `certificate`. Verification uses the
saved full trees and does not rerun reconstruction. Existing algorithm JSON
without the optional provenance field is still accepted.

To restore and save a certificate without running priority SAT:

```sh
round-eliminator-lib/target/release/examples/fixpoint_certificate_algorithm normalize \
  problem.txt certificate.txt synchronized.txt 60
```

Reconstruction is deterministic and memoized over tuples of the supplied
subterms. It preserves repeated label occurrences and verifies original
active leaf configurations. It does not invent arbitrary new subexpressions
or relax certificate checking. The search stops inconclusively at its time
budget or 200,000 cached tuple states; expansion beyond one million tree
nodes is rejected explicitly. These limits do not imply impossibility.

Different placements of restored redundant operators can produce different
priority-search instances. Only the first recovered derivation is searched;
an UNSAT result does **not** exclude other reconstructions. The original
certificate and its nonexistence conclusion are unaffected.

The 19 previously rejected certificates (cases 1–15 and 17–20 of the
unclassified batch) were rerun with 60 seconds per extraction. All 19 now
reach priority search and return UNSAT for their chosen reconstructed
derivation. There were no input failures or timeouts; measured wall times
were 0.01–3.29 seconds, running two cases concurrently. Reconstructed trees
had 8–64 arrows per expression. No algorithm was found in this experiment.
The exact reduced inputs are retained in the regression fixture
`round-eliminator-lib/src/algorithms/nofixpoint/algorithm/fixtures/loop_certificates.json`.

## Supplied hard certificate: completed, UNSAT

The first corrected, lazy-cycle implementation completed the supplied case
in 160.04 seconds and returned **UNSAT**, not a timeout. Its measurements:

| Measurement | Result |
| --- | --- |
| Original expressions | 4, each with 61 arrows and depth 8 |
| Colors | 5 |
| Priorities | 1,220 |
| Edge games | 160 |
| SAT variables | 683,276 |
| Relevant order comparisons | 225,705 |
| Initial clauses | 1,374,986 |
| Final clauses | 1,518,346 |
| Cycle exclusions | 143,360 |
| Intermediate SAT models | 560 |
| Preliminary schedules tried | 32 |
| Wall time, including process overhead | 160.17 seconds |
| Peak resident memory | 225,728 KiB |

The current version additionally triangulates cycle exclusions with shared
order comparisons and uses the best preliminary schedule for phase hints.
These are satisfiability-preserving refinements, not a broader algorithm
scheme. The table reports the preceding run, not a new benchmark of these
refinements.

This result rules out a schedule **in this helper's fixed-priority scheme
for this certificate and color count**. It does not invalidate the
nonexistence certificate and is not a proof that the problem lacks an
O(log* n) algorithm. Letting this same search run longer cannot change an
UNSAT result; extracting an algorithm would require investigating a
different certificate or a more general extraction scheme.

## Regression checks

Tests cover the old helper's second-root bug, all four-event tournaments
and lazy cycle exclusions, SAT/concrete-game agreement for all six-event
schedules of a small example, independent schedule verification, and the
exact supplied hard certificate. Reconstruction tests cover idempotent
columns, independently swapped children, repeated occurrences, invalid
leaves, interruption, and exact normalization round trips for all 19 saved
Loop certificates that previously failed before SAT search.
All 40 native fixed-point SAT/game/proof tests and all eight algorithm
extraction tests pass, including a recovered schedule's JSON round trip and
independent verification.
