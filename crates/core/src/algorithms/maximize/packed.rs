//! LLM: Packed set operations with density-dependent storage for wide alphabets.
//! LLM: Public constraints retain vector label sets. Working masks can have any width;
//! LLM: sparse wide alphabets use vectors. Representation never limits the search.

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

/// LLM: The bitwise set algebra for one machine-word mask. Nonzero masks are admissible output
/// LLM: choices; local bit positions are decoded to the original alphabet after maximization.
struct PackedOperations;
impl<M> Operations<M> for PackedOperations
where
    M: Copy + Default + PartialEq + BitOr<Output = M> + BitAnd<Output = M> + Not<Output = M>,
{
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &M, b: &M) -> M {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        *a | *b
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &M, b: &M) -> M {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        *a & *b
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &M, b: &M) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        (*b & !*a) == M::default()
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, value: &M) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        *value != M::default()
    }
}

/// LLM: A fixed four-word bit mask representing a dense alphabet in the maximization
/// LLM: experiment or implementation. Bit positions refer to its private local alphabet, not
/// LLM: necessarily the original numeric label IDs.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct FourWords([u64; 4]);
// LLM: Match the numeric ordering of u64/u128 masks: most significant word first.
// LLM: This keeps a useful traversal order; inclusion still uses set operations.

impl Ord for FourWords {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}
impl PartialOrd for FourWords {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // LLM: Return the result of this completed search or conversion to its caller.
        Some(self.cmp(other))
    }
}
/// LLM: The join, meet, containment, and nonempty checks for fixed four-word masks used by the
/// LLM: generic maximization engine.
struct FourOperations;
impl Operations<FourWords> for FourOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &FourWords, b: &FourWords) -> FourWords {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        FourWords(std::array::from_fn(|i| a.0[i] | b.0[i]))
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &FourWords, b: &FourWords) -> FourWords {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        FourWords(std::array::from_fn(|i| a.0[i] & b.0[i]))
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &FourWords, b: &FourWords) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().zip(b.0).all(|(&a, b)| b & !a == 0)
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, a: &FourWords) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().any(|&word| word != 0)
    }
}

/// LLM: An owned multiword bit mask for a dense local alphabet. Payload sharing and bitwise
/// LLM: operations are private to maximization and do not change the public label-set
/// LLM: representation.
#[derive(Clone, Eq, PartialEq)]
struct Words(Arc<[u64]>);
// LLM: Every mask in one computation has the same width. Clones share immutable
// LLM: payloads; newly computed joins/meets own their word arrays.

impl Ord for Words {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}
impl PartialOrd for Words {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // LLM: Return the result of this completed search or conversion to its caller.
        Some(self.cmp(other))
    }
}
/// LLM: The set algebra for multiword masks, including the policy that excludes empty output
/// LLM: choices during ordinary maximization.
struct WordOperations;
impl Operations<Words> for WordOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &Words, b: &Words) -> Words {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        Words(a.0.iter().zip(b.0.iter()).map(|(&a, &b)| a | b).collect())
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &Words, b: &Words) -> Words {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        Words(a.0.iter().zip(b.0.iter()).map(|(&a, &b)| a & b).collect())
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &Words, b: &Words) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().zip(b.0.iter()).all(|(&a, &b)| b & !a == 0)
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, a: &Words) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().any(|&w| w != 0)
    }
}

/// LLM: Try private packed-mask maximization for the constraint. Used by the public maximizer;
/// LLM: None selects the equivalent vector-set fallback for sparse wide alphabets, rather than
/// LLM: rejecting the problem or limiting its size.
pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    // LLM: Prepare the local alphabet shared by the representation conversion.
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() {
        computation.cancellation.check()?;
        alphabet.insert(label);
    }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    // LLM: Sparse wide sets cost less to compare as short vectors. Density chooses
    // LLM: working storage only, never which configurations the closure visits.

    if alphabet.len() > 128 {
        // LLM: Update the current working state before continuing this operation.
        let (labels, parts) = constraint
            .configurations()
            .flat_map(|row| row.parts())
            .fold((0u128, 0u128), |(labels, parts), part| {
                (labels + part.labels().len() as u128, parts + 1)
            });
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if labels < parts * alphabet.len().div_ceil(64) as u128 {
            return Ok(None);
        }
    }
    // LLM: Prepare `decoded` for the following computation; it stores the decoded for this
    // LLM: stage.
    let decoded = if alphabet.len() <= 64 {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
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
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
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
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        maximum_encoded_with(
            constraint,
            &alphabet,
            &FourOperations,
            |labels| {
                // LLM: Prepare `words` for the following computation; it stores the words for
                // LLM: this stage.
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
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        maximum_encoded_with(
            constraint,
            &alphabet,
            &WordOperations,
            |labels| {
                // LLM: Prepare `words` for the following computation; it stores the words for
                // LLM: this stage.
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
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Some(decoded))
}

/// LLM: Encode each complete part once, run the generic closure on packed values, and decode
/// LLM: surviving rows. Used by packed maximization variants to avoid repeated singleton-mask
/// LLM: allocation.
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
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        initial.push(Configuration::new(
            row.parts()
                .iter()
                .map(|part| (encode(part.labels()), part.multiplicity()))
                .collect(),
        )?);
    }
    // LLM: Run the same closure with the chosen packed-value/history representation.
    let maximum = maximize_with_storage(
        &initial,
        operations,
        computation,
        None,
        256,
        alphabet.len() > 256,
    )?;
    // LLM: Prepare `decoded` for the following computation; it stores the decoded for this
    // LLM: stage.
    let mut decoded = Vec::new();
    for row in maximum {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        decoded.push(Configuration::new(
            row.multiplicities()
                .iter()
                .map(|(bits, count)| {
                    // LLM: Update the current working state before continuing this operation.
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
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
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

    /// LLM: Construct a condensed test family from label domains and their multiplicities.
    /// LLM: Used by small coverage, maximization, or representation fixtures.
    fn family(labels: Vec<u16>, count: u32) -> CondensedConfiguration<u16> {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        CondensedConfiguration::new(vec![Part::new(LabelSet::new(labels), count).unwrap()]).unwrap()
    }

    /// LLM: Regression test: verify that all 64 bits noncontiguous ids counts and zero degree
    /// LLM: match vector sets. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn all_64_bits_noncontiguous_ids_counts_and_zero_degree_match_vector_sets() {
        // LLM: Prepare `labels` for the following computation; it stores the labels for this
        // LLM: stage.
        let labels: Vec<_> = (0..64).map(|i| i * 2 + 7).collect();
        let constraint = Constraint::new(vec![
            family(labels, u32::MAX),
            family(vec![7, 133], 1),
            CondensedConfiguration::new(vec![]).unwrap(),
        ]);
        // LLM: Prepare `initial` for the following computation; it stores the initial for this
        // LLM: stage.
        let initial: Vec<_> = constraint
            .configurations()
            .map(|row| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                Configuration::new(
                    row.parts()
                        .iter()
                        .map(|part| (part.labels().clone(), part.multiplicity()))
                        .collect(),
                )
                .unwrap()
            })
            .collect();
        // LLM: Connect phase/work updates to the caller while retaining cooperative
        // LLM: cancellation.
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        // LLM: Compute exact closure under the selected standard or dual algebra.
        let expected = maximize_custom_with(
            &initial,
            &SetOperations { allow_empty: false },
            &mut computation,
            None,
        )
        .unwrap();
        // LLM: Evaluate the implementation under test on the same fixture.
        let actual = maximum_with(&constraint, &mut computation)
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
    }

    /// LLM: Regression test: verify that sparse wide alphabets fall back without a search
    /// LLM: limit. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn sparse_wide_alphabets_fall_back_without_a_search_limit() {
        // LLM: Prepare `constraint` for the following computation; it stores the constraint
        // LLM: for this stage.
        let constraint = Constraint::new((0..129).map(|label| family(vec![label], 2)).collect());
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        assert!(
            maximum_with(&constraint, &mut computation)
                .unwrap()
                .is_none()
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            maximize_constraint_with(&constraint, &mut computation).unwrap(),
            constraint
        );
    }

    /// LLM: Regression test: verify that two word closure crosses bit 64 and keeps bit 127.
    /// LLM: Used by the test suite to protect this behavior while implementations change.
    #[test]
    fn two_word_closure_crosses_bit_64_and_keeps_bit_127() {
        // LLM: Compute exact closure under the selected standard or dual algebra.
        for count in [65u16, 128] {
            // LLM: Prepare `labels` for the following computation; it stores the labels for
            // LLM: this stage.
            let labels: Vec<_> = (0..count).map(|i| 2 * i + 7).collect();
            let (first, last) = (labels[0], labels[labels.len() - 1]);
            // LLM: Prepare `constraint` for the following computation; it stores the
            // LLM: constraint for this stage.
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
            // LLM: Prepare `initial` for the following computation; it stores the initial for
            // LLM: this stage.
            let initial: Vec<_> = constraint
                .configurations()
                .map(|row| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    Configuration::new(
                        row.parts()
                            .iter()
                            .map(|part| (part.labels().clone(), part.multiplicity()))
                            .collect(),
                    )
                    .unwrap()
                })
                .collect();
            // LLM: Connect phase/work updates to the caller while retaining cooperative
            // LLM: cancellation.
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            // LLM: Compute exact closure under the selected standard or dual algebra.
            let expected = maximize_custom_with(
                &initial,
                &SetOperations { allow_empty: false },
                &mut computation,
                None,
            )
            .unwrap();
            // LLM: Evaluate the implementation under test on the same fixture.
            let actual = maximum_with(&constraint, &mut computation)
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected);
            assert!(actual.contains(
                &Configuration::new(vec![(LabelSet::new(vec![first, last]), 2)]).unwrap()
            ));
        }
    }

    /// LLM: Regression test: verify that wider dense masks and compact history match vector
    /// LLM: closure. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn wider_dense_masks_and_compact_history_match_vector_closure() {
        // LLM: Exercise the fixed/dynamic boundary, noncontiguous IDs, real joins
        // LLM: across word boundaries, zero degree and non-expanded large counts.

        for count in [129u16, 256, 257, 522] {
            // LLM: Prepare `labels` for the following computation; it stores the labels for
            // LLM: this stage.
            let labels: Vec<_> = (0..count).map(|i| 2 * i + 7).collect();
            let (first, last) = (labels[0], labels[labels.len() - 1]);
            // LLM: Prepare `constraint` for the following computation; it stores the
            // LLM: constraint for this stage.
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
            // LLM: Prepare `initial` for the following computation; it stores the initial for
            // LLM: this stage.
            let initial: Vec<_> = constraint
                .configurations()
                .map(|row| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    Configuration::new(
                        row.parts()
                            .iter()
                            .map(|part| (part.labels().clone(), part.multiplicity()))
                            .collect(),
                    )
                    .unwrap()
                })
                .collect();
            // LLM: Connect phase/work updates to the caller while retaining cooperative
            // LLM: cancellation.
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            // LLM: Compute exact closure under the selected standard or dual algebra.
            let expected = maximize_custom_with(
                &initial,
                &SetOperations { allow_empty: false },
                &mut computation,
                None,
            )
            .unwrap();
            // LLM: Evaluate the implementation under test on the same fixture.
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
