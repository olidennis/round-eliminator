# Constructive lattice normalization (native only)

In **Fixed Point Tools**, **Make fixed point good** applies the constructive
normalization to an existing nontrivial fixed point. It processes the whole
problem and materializes the result regardless of the Partial / Only determine
triviality switches. It does not change Basic, Loop, `is_pred`, or the WASM path.

1. Expand node configurations, giving every occurrence its own identity.
2. Use the existing native Minisat dependency to map this structure into proper
   subsets of its node blocks until it is a core. Target node matches are
   permutations of *occurrences*, including repeated labels. Independently
   validate every SAT witness and the composed map.
3. Construct the lattice of intersections of core edge-neighborhoods, ordered
   by reverse inclusion. This is an explicit construction; there is **no search
   for a diagram**. Original-label twins can share one bit in this step, but not
   in the preceding occurrence-matching step.
4. Map each original label to the meet of all its occurrence outputs (and those
   of its edge-diagram successors), using double common-neighborhood closure.
5. Include all pairs of lattice elements whose extents are pairwise compatible
   in the core. This is an explicit edge relaxation, not a search.
6. Run the existing full fixed-point procedure, check nontriviality, and verify
   that the resulting edge diagram equals the construction lattice exactly.

The completeness argument assumes a finite, nonempty, live, unoriented node-edge
weak fixed point (`bar(R) R(F) ->_0 F`), ordinary zero-round triviality, and
context-dependent zero-round relaxations with label mergers allowed. The action
checks the degree, liveness, and nontriviality preconditions; it does **not** first
run RE to establish that the input is a weak fixed point. A successful output is
verified by the existing procedure and nontriviality test independently of that
assumption. A trivial construction reports an error, never a no-fixed-point
certificate. Inputs with unused or partnerless labels should be simplified first.

The result retains the full construction alphabet and the input-to-output label
mapping. Construction details include Custom-compatible diagram text: use it
on the **original input** to reuse the lattice. Custom need not reproduce the
additional edge relaxations in step 5. The displayed edge-replacement diagram is
recomputed and checked to equal the construction lattice. The usual GUI
postprocessing is deliberately bypassed because it
prunes auxiliary elements used by the lattice. Merger names, e.g. `(A=C)`, use the
same collision-safe naming as Loop.

Default limits: 64 original labels, node degree 16, 192 expanded occurrences,
100,000 Cartesian choices per constraint, 128 lattice elements, 500,000 variables
and 2,000,000 clauses per SAT query, and 50,000 conflicts per solve. Hitting a limit
is **inconclusive**, not UNSAT or a nonexistence result. The library's public
`NormalizationOptions` permits adjusting these limits (the bitset implementation
still caps the original alphabet at 64). The subsequent full closure uses the
existing cancellable procedure; the limits are not an end-to-end time bound.
Core SAT calls are conflict-bounded; STOP is observed between calls and during
encoding/closure. Progress prefixed `Normalize:` is printed in the server terminal.

For `A A B / C C B / D D E` with edges `A AC / B ED / D D`, the core has two node
configurations and six occurrences; its neighborhood lattice has seven elements.
The output is nontrivial and identifies the original labels A and C.

The accompanying proof draft is maintained outside this repository, at
`/workspace/lattice-fixed-point-proof/lattice-fixed-point-normalization.tex`, as requested. The result is a
research proof draft, not a machine-formally verified theorem.

Tests:

```
cargo test --release --lib fixpoint_normalize
node --test www/gui-fixpoint-normalize.test.cjs www/gui-label-mapping.test.cjs
```
