//! Small-alphabet set operations packed at the algorithm boundary.
//! Public constraints retain their usual label sets; larger alphabets use them
//! directly. The 64-bit representation imposes no mathematical search limit.

use super::engine::{Operations, maximize_with_minimum};
use crate::{
    computation::Computation,
    model::{Configuration, Constraint, LabelSet},
};
use std::collections::BTreeSet;

struct PackedOperations;
impl Operations<u64> for PackedOperations {
    fn union(&self, a: &u64, b: &u64) -> u64 {
        a | b
    }
    fn intersection(&self, a: &u64, b: &u64) -> u64 {
        a & b
    }
    fn is_superset(&self, a: &u64, b: &u64) -> bool {
        b & !a == 0
    }
    fn is_admissible(&self, value: &u64) -> bool {
        *value != 0
    }
}

pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() {
        computation.cancellation.check()?;
        alphabet.insert(label);
        if alphabet.len() > 64 {
            return Ok(None);
        }
    }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        computation.cancellation.check()?;
        let mut parts = Vec::new();
        for part in row.parts() {
            let mut bits = 0u64;
            for label in part.labels().iter() {
                bits |= 1u64 << alphabet.binary_search(&label).unwrap();
            }
            parts.push((bits, part.multiplicity()));
        }
        initial.push(Configuration::new(parts)?);
    }
    let maximum = maximize_with_minimum(&initial, &PackedOperations, computation, None, 256)?;
    let mut decoded = Vec::new();
    for row in maximum {
        computation.cancellation.check()?;
        let parts = row
            .multiplicities()
            .iter()
            .map(|&(bits, count)| {
                let labels = alphabet
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &label)| (bits & (1u64 << i) != 0).then_some(label))
                    .collect();
                (LabelSet::new(labels), count)
            })
            .collect();
        decoded.push(Configuration::new(parts)?);
    }
    decoded.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(Some(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        algorithms::maximize::{SetOperations, maximize_constraint_with, maximize_custom_with},
        computation::Cancellation,
        model::{CondensedConfiguration, Part},
    };

    fn family(labels: Vec<u16>, count: u32) -> CondensedConfiguration<u16> {
        CondensedConfiguration::new(vec![Part::new(LabelSet::new(labels), count).unwrap()]).unwrap()
    }

    #[test]
    fn all_64_bits_noncontiguous_ids_counts_and_zero_degree_match_vector_sets() {
        let labels: Vec<_> = (0..64).map(|i| i * 2 + 7).collect();
        let constraint = Constraint::new(vec![
            family(labels, u32::MAX),
            family(vec![7, 133], 1),
            CondensedConfiguration::new(vec![]).unwrap(),
        ]);
        let initial: Vec<_> = constraint
            .configurations()
            .map(|row| {
                Configuration::new(
                    row.parts()
                        .iter()
                        .map(|part| (part.labels().clone(), part.multiplicity()))
                        .collect(),
                )
                .unwrap()
            })
            .collect();
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        let expected = maximize_custom_with(
            &initial,
            &SetOperations { allow_empty: false },
            &mut computation,
            None,
        )
        .unwrap();
        let actual = maximum_with(&constraint, &mut computation)
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn larger_alphabets_fall_back_without_a_search_limit() {
        let constraint = Constraint::new(vec![family((0..65).collect(), 2)]);
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        assert!(
            maximum_with(&constraint, &mut computation)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            maximize_constraint_with(&constraint, &mut computation).unwrap(),
            constraint
        );
    }
}
