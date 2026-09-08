# Extracting a priority algorithm from a supplied certificate

The native runner takes an existing nonexistence certificate; it does not
search for a diagram or rediscover the certificate. Use the unshortened
`Original expressions`, since their synchronized derivation columns are
needed by the extractor. The automatic extraction call in GUI Loop remains
disabled, and WASM is unchanged.

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

Five native tests cover the old helper's second-root bug, all four-event
tournaments and lazy cycle exclusions, SAT/concrete-game agreement for all
six-event schedules of a small example, independent schedule verification,
and parsing/validation of the exact supplied certificate.
All five pass, as do all 37 existing native fixed-point SAT/game/proof tests.
