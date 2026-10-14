//! LLM: Successor-closed subsets of the default output diagram, including the empty
//! LLM: set, with precomputed joins/meets for the two dual closure algebras.

use crate::{
    algorithms::{graphs::reachability_with, maximize::Operations},
    computation::{Computation, Phase},
    model::{LabelId, LabelSet},
};
use std::collections::{BTreeMap, BTreeSet};

/// LLM: The frozen comparison kernel's diagram-generated closed sets, principal elements, and
/// LLM: full join/meet/subset tables. All operation pairs are precomputed; this historical
/// LLM: layout is retained for benchmark comparisons.
pub(super) struct Lattice {
    pub sets: Vec<LabelSet>,
    pub principal: Vec<LabelId>,
    union: Vec<Vec<LabelId>>,
    intersection: Vec<Vec<LabelId>>,
    subset: Vec<Vec<bool>>,
}

impl Lattice {
    /// LLM: Enumerate the diagram-generated closed sets and prepare their principal images and
    /// LLM: operation lookup representation. Used once during Basic FP construction.
    pub fn new(successors: &[Vec<u32>], computation: &mut Computation<'_>) -> anyhow::Result<Self> {
        // LLM: Prepare counted rows or remaining row capacities for the next exact comparison.
        let rows: Vec<_> = successors
            .iter()
            .map(|row| row.iter().map(|&i| i as usize).collect())
            .collect();
        let principal_sets: Vec<_> = reachability_with(&rows, &computation.cancellation)?
            .into_iter()
            .map(|row| LabelSet::new(row.into_iter().map(|i| LabelId(i as u32)).collect()))
            .collect();
        // LLM: Every successor-closed set is a union of principal closures. Build
        // LLM: those unions incrementally instead of testing every arbitrary subset.

        let mut closed = BTreeSet::from([LabelSet::default()]);
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (index, principal) in principal_sets.iter().enumerate() {
            // LLM: Prepare `additions` for the following computation; it stores the additions
            // LLM: for this stage.
            let mut additions = Vec::new();
            for set in &closed {
                computation.cancellation.check()?;
                additions.push(set.union(principal));
            }
            closed.extend(additions);
            computation.report(Phase::Preparing, index as u32 + 1)?;
        }
        // LLM: Collect canonical working sets before assigning deterministic local IDs.
        let sets: Vec<_> = closed.into_iter().collect();
        let ids: BTreeMap<_, _> = sets
            .iter()
            .enumerate()
            .map(|(i, set)| Ok((set.clone(), LabelId(u32::try_from(i)?))))
            .collect::<Result<_, std::num::TryFromIntError>>()?;
        let principal = principal_sets.iter().map(|set| ids[set]).collect();
        let mut union = Vec::new();
        // LLM: Prepare `intersection` for the following computation; it stores the
        // LLM: intersection for this stage.
        let mut intersection = Vec::new();
        let mut subset = Vec::new();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for a in &sets {
            // LLM: Update the current working state before continuing this operation.
            let (mut joins, mut meets, mut order) = (Vec::new(), Vec::new(), Vec::new());
            for b in &sets {
                computation.cancellation.check()?;
                joins.push(ids[&a.union(b)]);
                meets.push(ids[&a.intersection(b)]);
                order.push(a.is_subset_of(b));
            }
            union.push(joins);
            // LLM: Retain the newly constructed choices in the result for this stage.
            intersection.push(meets);
            subset.push(order);
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            sets,
            principal,
            union,
            intersection,
            subset,
        })
    }

    /// LLM: Collect every lattice ID below one value in the inclusion order. Used when
    /// LLM: translating fixed-point algebra elements to passive downward choices.
    pub fn subsets(&self, label: LabelId) -> LabelSet {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        LabelSet::new(
            (0..self.sets.len())
                .filter(|&i| self.subset[i][label.0 as usize])
                .map(|i| LabelId(i as u32))
                .collect(),
        )
    }
}

/// LLM: A borrowed lattice and duality flag for fixed-point closure. The algebra keeps
/// LLM: bottom/empty elements; dual mode swaps union/intersection and reverses the order.
pub(super) struct LatticeOperations<'a> {
    pub lattice: &'a Lattice,
    pub dual: bool,
}
impl Operations<LabelId> for LatticeOperations<'_> {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &LabelId, b: &LabelId) -> LabelId {
        // LLM: Prepare `table` for the following computation; it stores the table for this
        // LLM: stage.
        let table = if self.dual {
            &self.lattice.intersection
        } else {
            &self.lattice.union
        };
        table[a.0 as usize][b.0 as usize]
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &LabelId, b: &LabelId) -> LabelId {
        // LLM: Prepare `table` for the following computation; it stores the table for this
        // LLM: stage.
        let table = if self.dual {
            &self.lattice.union
        } else {
            &self.lattice.intersection
        };
        table[a.0 as usize][b.0 as usize]
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &LabelId, b: &LabelId) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if self.dual {
            self.lattice.subset[a.0 as usize][b.0 as usize]
        } else {
            self.lattice.subset[b.0 as usize][a.0 as usize]
        }
    }
    // LLM: The default admissibility keeps every value, including the empty set.

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computation::Cancellation;
    /// LLM: Regression test: verify that cycles transitive arrows and isolated labels generate
    /// LLM: closed sets. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn cycles_transitive_arrows_and_isolated_labels_generate_closed_sets() {
        // LLM: Prepare `lattice` for the following computation; it stores the lattice for this
        // LLM: stage.
        let lattice = Lattice::new(
            &[vec![1], vec![0, 2], vec![], vec![]],
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        // LLM: Construct the independent reference result for this fixture.
        let expected = [
            vec![],
            vec![2],
            vec![3],
            vec![2, 3],
            vec![0, 1, 2],
            vec![0, 1, 2, 3],
        ];
        // LLM: Construct the independent reference result for this fixture.
        let expected: BTreeSet<_> = expected
            .into_iter()
            .map(|ids| LabelSet::new(ids.into_iter().map(LabelId).collect()))
            .collect();
        assert_eq!(
            lattice.sets.iter().cloned().collect::<BTreeSet<_>>(),
            expected
        );
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for a in 0..lattice.sets.len() {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for b in 0..lattice.sets.len() {
                // LLM: Prepare `a` for the following computation; it stores the a for this
                // LLM: stage.
                let a = LabelId(a as u32);
                let b = LabelId(b as u32);
                let ordinary = LatticeOperations {
                    lattice: &lattice,
                    dual: false,
                };
                // LLM: Prepare `dual` for the following computation; it stores the dual for
                // LLM: this stage.
                let dual = LatticeOperations {
                    lattice: &lattice,
                    dual: true,
                };
                assert_eq!(dual.union(&a, &b), ordinary.intersection(&a, &b));
                assert_eq!(dual.intersection(&a, &b), ordinary.union(&a, &b));
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    lattice.sets[ordinary.union(&a, &b).0 as usize],
                    lattice.sets[a.0 as usize].union(&lattice.sets[b.0 as usize])
                );
            }
        }
    }
}
