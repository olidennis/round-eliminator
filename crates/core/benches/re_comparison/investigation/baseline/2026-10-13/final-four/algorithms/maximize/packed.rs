//! LLM: Set operations packed at the algorithm boundary for up to 128 labels.
//! LLM: Public constraints retain their usual label sets; larger alphabets use them
//! LLM: directly. The working representation imposes no mathematical search limit.

use super::engine::{Operations, maximize_with_minimum};
use crate::{
    computation::Computation,
    model::{Configuration, Constraint, LabelSet},
};
use std::{
    collections::BTreeSet,
    ops::{BitAnd, BitOr, Not},
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
        if alphabet.len() > 128 {
            return Ok(None);
        }
    }
    // LLM: Use a deterministic local alphabet for compact masks and decide whether density
    // LLM: justifies packed storage.
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    // LLM: Keep compact one-word rows for small alphabets; u128 would double their
    // LLM: payload on this target. Larger alphabets use two-word integer operations.

    let decoded = if alphabet.len() <= 64 {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        maximum_packed_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |i| 1u64 << i,
            computation,
        )?
    } else {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        maximum_packed_with(
            constraint,
            &alphabet,
            &PackedOperations,
            |i| 1u128 << i,
            computation,
        )?
    };
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Some(decoded))
}

/// LLM: Compute the same constraint closure using the frozen benchmark's selected packed-mask
/// LLM: layout. Used to compare historical representation policies.
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
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let mut parts = Vec::new();
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for part in row.parts() {
            let mut bits = M::default();
            for label in part.labels().iter() {
                bits = operations.union(&bits, &bit(alphabet.binary_search(&label).unwrap()));
            }
            parts.push((bits, part.multiplicity()));
        }
        initial.push(Configuration::new(parts)?);
    }
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = maximize_with_minimum(&initial, operations, computation, None, 256)?;
    let mut decoded = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for row in maximum {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let parts = row
            .multiplicities()
            .iter()
            .map(|&(bits, count)| {
                // LLM: Prepare `labels` for the following computation; it stores the labels
                // LLM: for this stage.
                let labels = alphabet
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &label)| {
                        operations
                            .is_admissible(&operations.intersection(&bits, &bit(i)))
                            .then_some(label)
                    })
                    .collect();
                // LLM: Update the current working state before continuing this operation.
                (LabelSet::new(labels), count)
            })
            .collect();
        // LLM: Retain the newly constructed choices in the result for this stage.
        decoded.push(Configuration::new(parts)?);
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

    /// LLM: Regression test: verify that larger alphabets fall back without a search limit.
    /// LLM: Used by the test suite to protect this behavior while implementations change.
    #[test]
    fn larger_alphabets_fall_back_without_a_search_limit() {
        // LLM: Prepare `constraint` for the following computation; it stores the constraint
        // LLM: for this stage.
        let constraint = Constraint::new(vec![family((0..129).collect(), 2)]);
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
}
