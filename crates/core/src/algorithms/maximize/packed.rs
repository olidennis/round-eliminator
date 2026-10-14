//! Packed set operations with density-dependent storage for wide alphabets.
//! Public constraints retain vector label sets. Working masks can have any width;
//! sparse wide alphabets use vectors. Representation never limits the search.

use super::engine::{Operations, maximize_with_storage};
use crate::{
    computation::Computation,
    model::{Configuration, Constraint, LabelSet},
};
use std::{
    collections::BTreeSet,
    ops::{BitAnd, BitOr, Not},
    sync::Arc,
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

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct FourWords([u64; 4]);
// Match the numeric ordering of u64/u128 masks: most significant word first.
// This keeps a useful traversal order; inclusion still uses set operations.
impl Ord for FourWords {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}
impl PartialOrd for FourWords {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
struct FourOperations;
impl Operations<FourWords> for FourOperations {
    fn union(&self, a: &FourWords, b: &FourWords) -> FourWords {
        FourWords(std::array::from_fn(|i| a.0[i] | b.0[i]))
    }
    fn intersection(&self, a: &FourWords, b: &FourWords) -> FourWords {
        FourWords(std::array::from_fn(|i| a.0[i] & b.0[i]))
    }
    fn is_superset(&self, a: &FourWords, b: &FourWords) -> bool {
        a.0.iter().zip(b.0).all(|(&a, b)| b & !a == 0)
    }
    fn is_admissible(&self, a: &FourWords) -> bool {
        a.0.iter().any(|&word| word != 0)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct Words(Arc<[u64]>);
// Every mask in one computation has the same width. Clones share immutable
// payloads; newly computed joins/meets own their word arrays.
impl Ord for Words {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}
impl PartialOrd for Words {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
struct WordOperations;
impl Operations<Words> for WordOperations {
    fn union(&self, a: &Words, b: &Words) -> Words {
        Words(a.0.iter().zip(b.0.iter()).map(|(&a, &b)| a | b).collect())
    }
    fn intersection(&self, a: &Words, b: &Words) -> Words {
        Words(a.0.iter().zip(b.0.iter()).map(|(&a, &b)| a & b).collect())
    }
    fn is_superset(&self, a: &Words, b: &Words) -> bool {
        a.0.iter().zip(b.0.iter()).all(|(&a, &b)| b & !a == 0)
    }
    fn is_admissible(&self, a: &Words) -> bool {
        a.0.iter().any(|&w| w != 0)
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
    }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    // Sparse wide sets cost less to compare as short vectors. Density chooses
    // working storage only, never which configurations the closure visits.
    if alphabet.len() > 128 {
        let (labels, parts) = constraint
            .configurations()
            .flat_map(|row| row.parts())
            .fold((0u128, 0u128), |(labels, parts), part| {
                (labels + part.labels().len() as u128, parts + 1)
            });
        if labels < parts * alphabet.len().div_ceil(64) as u128 {
            return Ok(None);
        }
    }
    let decoded = if alphabet.len() <= 64 {
        maximum_encoded_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |labels| {
                labels.iter().fold(0u64, |bits, label| {
                    bits | (1 << alphabet.binary_search(&label).unwrap())
                })
            },
            |bits, i| bits & (1u64 << i) != 0,
            computation,
        )?
    } else if alphabet.len() <= 128 {
        maximum_encoded_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |labels| {
                labels.iter().fold(0u128, |bits, label| {
                    bits | (1 << alphabet.binary_search(&label).unwrap())
                })
            },
            |bits, i| bits & (1u128 << i) != 0,
            computation,
        )?
    } else if alphabet.len() <= 256 {
        maximum_encoded_with(
            constraint,
            &alphabet,
            &FourOperations,
            |labels| {
                let mut words = [0; 4];
                for label in labels.iter() {
                    let i = alphabet.binary_search(&label).unwrap();
                    words[i / 64] |= 1 << (i % 64);
                }
                FourWords(words)
            },
            |bits, i| bits.0[i / 64] & (1 << (i % 64)) != 0,
            computation,
        )?
    } else {
        maximum_encoded_with(
            constraint,
            &alphabet,
            &WordOperations,
            |labels| {
                let mut words = vec![0; alphabet.len().div_ceil(64)];
                for label in labels.iter() {
                    let i = alphabet.binary_search(&label).unwrap();
                    words[i / 64] |= 1 << (i % 64);
                }
                Words(words.into())
            },
            |bits, i| bits.0[i / 64] & (1 << (i % 64)) != 0,
            computation,
        )?
    };
    Ok(Some(decoded))
}

/// Encode a complete part once. In particular, dynamic masks avoid allocating
/// and joining a singleton mask for every label. Payload sharing stays private
/// to this algorithm; the model and resulting label sets retain vector storage.
fn maximum_encoded_with<L, M>(
    constraint: &Constraint<L>,
    alphabet: &[L],
    operations: &impl Operations<M>,
    encode: impl Fn(&LabelSet<L>) -> M,
    contains: impl Fn(&M, usize) -> bool,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelSet<L>>>>
where
    L: Copy + Ord,
    M: Clone + Ord + Send + Sync,
{
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        computation.cancellation.check()?;
        initial.push(Configuration::new(
            row.parts()
                .iter()
                .map(|part| (encode(part.labels()), part.multiplicity()))
                .collect(),
        )?);
    }
    let maximum = maximize_with_storage(
        &initial,
        operations,
        computation,
        None,
        256,
        alphabet.len() > 256,
    )?;
    let mut decoded = Vec::new();
    for row in maximum {
        computation.cancellation.check()?;
        decoded.push(Configuration::new(
            row.multiplicities()
                .iter()
                .map(|(bits, count)| {
                    (
                        LabelSet::new(
                            alphabet
                                .iter()
                                .enumerate()
                                .filter_map(|(i, &label)| contains(bits, i).then_some(label))
                                .collect(),
                        ),
                        *count,
                    )
                })
                .collect(),
        )?);
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
    fn sparse_wide_alphabets_fall_back_without_a_search_limit() {
        let constraint = Constraint::new((0..129).map(|label| family(vec![label], 2)).collect());
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

    #[test]
    fn wider_dense_masks_and_compact_history_match_vector_closure() {
        // Exercise the fixed/dynamic boundary, noncontiguous IDs, real joins
        // across word boundaries, zero degree and non-expanded large counts.
        for count in [129u16, 256, 257, 522] {
            let labels: Vec<_> = (0..count).map(|i| 2 * i + 7).collect();
            let (first, last) = (labels[0], labels[labels.len() - 1]);
            let constraint = Constraint::new(vec![
                family(labels.clone(), 1),
                family(labels, u32::MAX),
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
