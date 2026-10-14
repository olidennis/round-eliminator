//! Set operations packed at the algorithm boundary for up to 128 labels.
//! Public constraints retain their usual label sets; larger alphabets use them
//! directly. The working representation imposes no mathematical search limit.

use super::engine::{Operations, maximize_with_minimum};
use crate::{
    computation::Computation,
    model::{Configuration, Constraint, LabelSet},
};
use std::{
    collections::BTreeSet,
    ops::{BitAnd, BitOr, Not},
};

struct PackedOperations;
impl<M> Operations<M> for PackedOperations
where
    M: Copy + Default + PartialEq + BitOr<Output = M> + BitAnd<Output = M> + Not<Output = M>,
{
    fn union(&self, a: &M, b: &M) -> M {
        *a | *b
    }
    fn intersection(&self, a: &M, b: &M) -> M {
        *a & *b
    }
    fn is_superset(&self, a: &M, b: &M) -> bool {
        (*b & !*a) == M::default()
    }
    fn is_admissible(&self, value: &M) -> bool {
        *value != M::default()
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
        if alphabet.len() > 128 {
            return Ok(None);
        }
    }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    // Keep compact one-word rows for small alphabets; u128 would double their
    // payload on this target. Larger alphabets use two-word integer operations.
    let decoded = if alphabet.len() <= 64 {
        maximum_packed_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |i| 1u64 << i,
            computation,
        )?
    } else {
        maximum_packed_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |i| 1u128 << i,
            computation,
        )?
    };
    Ok(Some(decoded))
}

fn maximum_packed_with<L, M>(
    constraint: &Constraint<L>,
    alphabet: &[L],
    operations: &impl Operations<M>,
    bit: impl Fn(usize) -> M,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelSet<L>>>>
where
    L: Copy + Ord,
    M: Copy + Default + Ord + Send + Sync,
{
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        computation.cancellation.check()?;
        let mut parts = Vec::new();
        for part in row.parts() {
            let mut bits = M::default();
            for label in part.labels().iter() {
                bits = operations.union(&bits, &bit(alphabet.binary_search(&label).unwrap()));
            }
            parts.push((bits, part.multiplicity()));
        }
        initial.push(Configuration::new(parts)?);
    }
    let maximum = maximize_with_minimum(&initial, operations, computation, None, 256)?;
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
                    .filter_map(|(i, &label)| {
                        operations
                            .is_admissible(&operations.intersection(&bits, &bit(i)))
                            .then_some(label)
                    })
                    .collect();
                (LabelSet::new(labels), count)
            })
            .collect();
        decoded.push(Configuration::new(parts)?);
    }
    decoded.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(decoded)
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
        let constraint = Constraint::new(vec![family((0..129).collect(), 2)]);
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

    #[test]
    fn two_word_closure_crosses_bit_64_and_keeps_bit_127() {
        for count in [65u16, 128] {
            let labels: Vec<_> = (0..count).map(|i| 2 * i + 7).collect();
            let (first, last) = (labels[0], labels[labels.len() - 1]);
            let constraint = Constraint::new(vec![
                family(labels.clone(), 1),
                family(labels.clone(), u32::MAX),
                family(vec![first], 2),
                family(vec![last], 2),
                CondensedConfiguration::new(vec![
                    Part::new(LabelSet::new(vec![first]), 1).unwrap(),
                    Part::new(LabelSet::new(vec![last]), 1).unwrap(),
                ])
                .unwrap(),
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
            assert!(actual.contains(
                &Configuration::new(vec![(LabelSet::new(vec![first, last]), 2)]).unwrap()
            ));
        }
    }
}
