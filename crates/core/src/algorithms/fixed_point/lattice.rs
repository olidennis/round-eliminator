//! Successor-closed subsets of the default output diagram, including the empty
//! set, with precomputed joins/meets for the two dual closure algebras.

use crate::{
    algorithms::{graphs::reachability_with, maximize::Operations},
    computation::{Computation, Phase},
    model::{LabelId, LabelSet},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Lattice {
    pub sets: Vec<LabelSet>,
    pub principal: Vec<LabelId>,
    union: Vec<Vec<LabelId>>,
    intersection: Vec<Vec<LabelId>>,
    subset: Vec<Vec<bool>>,
}

impl Lattice {
    pub fn new(successors: &[Vec<u32>], computation: &mut Computation<'_>) -> anyhow::Result<Self> {
        let rows: Vec<_> = successors
            .iter()
            .map(|row| row.iter().map(|&i| i as usize).collect())
            .collect();
        let principal_sets: Vec<_> = reachability_with(&rows, &computation.cancellation)?
            .into_iter()
            .map(|row| LabelSet::new(row.into_iter().map(|i| LabelId(i as u32)).collect()))
            .collect();
        // Every successor-closed set is a union of principal closures. Build
        // those unions incrementally instead of testing every arbitrary subset.
        let mut closed = BTreeSet::from([LabelSet::default()]);
        for (index, principal) in principal_sets.iter().enumerate() {
            let mut additions = Vec::new();
            for set in &closed {
                computation.cancellation.check()?;
                additions.push(set.union(principal));
            }
            closed.extend(additions);
            computation.report(Phase::Preparing, index as u32 + 1)?;
        }
        let sets: Vec<_> = closed.into_iter().collect();
        let ids: BTreeMap<_, _> = sets
            .iter()
            .enumerate()
            .map(|(i, set)| Ok((set.clone(), LabelId(u32::try_from(i)?))))
            .collect::<Result<_, std::num::TryFromIntError>>()?;
        let principal = principal_sets.iter().map(|set| ids[set]).collect();
        let mut union = Vec::new();
        let mut intersection = Vec::new();
        let mut subset = Vec::new();
        for a in &sets {
            let (mut joins, mut meets, mut order) = (Vec::new(), Vec::new(), Vec::new());
            for b in &sets {
                computation.cancellation.check()?;
                joins.push(ids[&a.union(b)]);
                meets.push(ids[&a.intersection(b)]);
                order.push(a.is_subset_of(b));
            }
            union.push(joins);
            intersection.push(meets);
            subset.push(order);
        }
        Ok(Self {
            sets,
            principal,
            union,
            intersection,
            subset,
        })
    }

    pub fn subsets(&self, label: LabelId) -> LabelSet {
        LabelSet::new(
            (0..self.sets.len())
                .filter(|&i| self.subset[i][label.0 as usize])
                .map(|i| LabelId(i as u32))
                .collect(),
        )
    }
}

pub(super) struct LatticeOperations<'a> {
    pub lattice: &'a Lattice,
    pub dual: bool,
}
impl Operations<LabelId> for LatticeOperations<'_> {
    fn union(&self, a: &LabelId, b: &LabelId) -> LabelId {
        let table = if self.dual {
            &self.lattice.intersection
        } else {
            &self.lattice.union
        };
        table[a.0 as usize][b.0 as usize]
    }
    fn intersection(&self, a: &LabelId, b: &LabelId) -> LabelId {
        let table = if self.dual {
            &self.lattice.union
        } else {
            &self.lattice.intersection
        };
        table[a.0 as usize][b.0 as usize]
    }
    fn is_superset(&self, a: &LabelId, b: &LabelId) -> bool {
        if self.dual {
            self.lattice.subset[a.0 as usize][b.0 as usize]
        } else {
            self.lattice.subset[b.0 as usize][a.0 as usize]
        }
    }
    // The default admissibility keeps every value, including the empty set.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computation::Cancellation;
    #[test]
    fn cycles_transitive_arrows_and_isolated_labels_generate_closed_sets() {
        let lattice = Lattice::new(
            &[vec![1], vec![0, 2], vec![], vec![]],
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        let expected = [
            vec![],
            vec![2],
            vec![3],
            vec![2, 3],
            vec![0, 1, 2],
            vec![0, 1, 2, 3],
        ];
        let expected: BTreeSet<_> = expected
            .into_iter()
            .map(|ids| LabelSet::new(ids.into_iter().map(LabelId).collect()))
            .collect();
        assert_eq!(
            lattice.sets.iter().cloned().collect::<BTreeSet<_>>(),
            expected
        );
        for a in 0..lattice.sets.len() {
            for b in 0..lattice.sets.len() {
                let a = LabelId(a as u32);
                let b = LabelId(b as u32);
                let ordinary = LatticeOperations {
                    lattice: &lattice,
                    dual: false,
                };
                let dual = LatticeOperations {
                    lattice: &lattice,
                    dual: true,
                };
                assert_eq!(dual.union(&a, &b), ordinary.intersection(&a, &b));
                assert_eq!(dual.intersection(&a, &b), ordinary.union(&a, &b));
                assert_eq!(
                    lattice.sets[ordinary.union(&a, &b).0 as usize],
                    lattice.sets[a.0 as usize].union(&lattice.sets[b.0 as usize])
                );
            }
        }
    }
}
