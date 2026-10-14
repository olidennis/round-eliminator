//! LLM: Join two multiset constraints through an incidence compatibility relation.
//! LLM: Count matrices align parts, rather than expanding repetitions or permutations.
//! LLM: Each nonzero cell produces one condensed set of compatible label pairs.

use super::visit_alignments_with;

use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Constraint, LabelSet, Part},
};

/// LLM: Construct permitted joint choices of two same-degree constraint languages by visiting
/// LLM: counted alignments. The callback maps compatible label pairs or rejects them; used by
/// LLM: input-aware maxima and exact joint diagrams.
pub(crate) fn product_with<A: Copy + Ord, B: Copy + Ord, L: Copy + Ord>(
    left: &Constraint<A>,
    right: &Constraint<B>,
    pair: impl Fn(A, B) -> Option<L>,
    cancellation: &Cancellation,
) -> Result<Constraint<L>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let mut result = Vec::new();
    // LLM: Explore the next counted alternative while preserving the required total
    // LLM: capacities.
    for first in left.configurations() {
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        for second in right.configurations_at_degree(first.degree()) {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let mut choices = Vec::new();
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for first_part in first.parts() {
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for second_part in second.parts() {
                    // LLM: Prepare `labels` for the following computation; it stores the
                    // LLM: labels for this stage.
                    let mut labels = Vec::new();
                    for a in first_part.labels().iter() {
                        // LLM: Process each condensed part without expanding its repeated
                        // LLM: occurrences.
                        for b in second_part.labels().iter() {
                            cancellation.check()?;
                            if let Some(label) = pair(a, b) {
                                labels.push(label)
                            }
                        }
                    }
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    choices.push(LabelSet::new(labels));
                }
            }
            // LLM: Prepare counted rows or remaining row capacities for the next exact
            // LLM: comparison.
            let rows: Vec<_> = first.parts().iter().map(Part::multiplicity).collect();
            let remaining: Vec<_> = second.parts().iter().map(Part::multiplicity).collect();
            let compatible: Vec<_> = choices.iter().map(|set| !set.is_empty()).collect();
            // LLM: Retain the newly constructed choices in the result for this stage.
            visit_alignments_with(&rows, &remaining, &compatible, cancellation, |counts| {
                // LLM: Prepare `parts` for the following computation; it stores the parts for
                // LLM: this stage.
                let parts = counts
                    .iter()
                    .zip(&choices)
                    .filter(|(count, _)| **count > 0)
                    .map(|(&count, set)| Part::new(set.clone(), count).expect("compatible choices"))
                    .collect();
                result.push(CondensedConfiguration::new(parts).expect("product preserves degree"));
                Ok(())
            })?;
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::configurations::concrete_multisets;

    /// LLM: Regression test: verify that many parts use heap frames and skip forced zero
    /// LLM: cells. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn many_parts_use_heap_frames_and_skip_forced_zero_cells() {
        // LLM: Prepare `configuration` for the following computation; it stores the
        // LLM: configuration for this stage.
        let configuration = CondensedConfiguration::new(
            (0..200u16)
                .map(|label| Part::new(LabelSet::new(vec![label]), 1).unwrap())
                .collect(),
        )
        .unwrap();
        let constraint = Constraint::new(vec![configuration]);
        // LLM: Align counted input and output parts and retain only permitted joint label
        // LLM: combinations.
        let product = product_with(
            &constraint,
            &constraint,
            |a, b| (a == b).then_some((a, b)),
            &Cancellation::default(),
        )
        .unwrap();
        let configurations = product.configurations_at_degree(200);
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].parts().len(), 200);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(
            product_with(&constraint, &constraint, |a, b| Some((a, b)), &cancellation).is_err()
        );
    }
    /// LLM: Regression test: verify that product keeps counts and permits all alignments. Used
    /// LLM: by the test suite to protect this behavior while implementations change.
    #[test]
    fn product_keeps_counts_and_permits_all_alignments() {
        // LLM: Prepare `make` for the following computation; it stores the make for this
        // LLM: stage.
        let make = |groups: Vec<(Vec<u8>, u32)>| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            Constraint::new(vec![
                CondensedConfiguration::new(
                    groups
                        .into_iter()
                        .map(|(labels, count)| Part::new(LabelSet::new(labels), count).unwrap())
                        .collect(),
                )
                .unwrap(),
            ])
        };
        // LLM: Prepare `left` for the following computation; it stores the left for this
        // LLM: stage.
        let left = make(vec![(vec![0], 1), (vec![1], 1)]);
        let right = make(vec![(vec![2], 1), (vec![3], 1)]);
        let result =
            product_with(&left, &right, |a, b| Some((a, b)), &Cancellation::default()).unwrap();
        assert_eq!(concrete_multisets(&result, &[2]).len(), 2);
        let large = make(vec![(vec![0], u32::MAX)]);
        // LLM: Align counted input and output parts and retain only permitted joint label
        // LLM: combinations.
        let result = product_with(
            &large,
            &large,
            |a, b| Some((a, b)),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(result.configurations().count(), 1);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            result.configurations().next().unwrap().parts()[0].multiplicity(),
            u32::MAX
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            product_with(&left, &right, |_, _| None::<u8>, &Cancellation::default())
                .unwrap()
                .configurations()
                .next()
                .is_none()
        );
    }

    /// LLM: Regression test: verify that condensed product agrees with independent
    /// LLM: projections. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn condensed_product_agrees_with_independent_projections() {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use crate::algorithms::configurations::accepts;
        use crate::model::Configuration;
        // LLM: Align counted input and output parts and retain only permitted joint label
        // LLM: combinations.
        for a in 1..4 {
            // LLM: Align counted input and output parts and retain only permitted joint label
            // LLM: combinations.
            for b in 1..4 {
                // LLM: Prepare `labels` for the following computation; it stores the labels
                // LLM: for this stage.
                let labels = |mask: u8, offset: u8| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    LabelSet::new(
                        (0u8..2)
                            .filter(|i| mask & (1 << i) != 0)
                            .map(|i| i + offset)
                            .collect(),
                    )
                };
                // LLM: Prepare `left` for the following computation; it stores the left for
                // LLM: this stage.
                let left = Constraint::new(vec![
                    CondensedConfiguration::new(vec![
                        Part::new(labels(a, 0), 2).unwrap(),
                        Part::new(labels(b, 0), 1).unwrap(),
                    ])
                    .unwrap(),
                ]);
                // LLM: Prepare `right` for the following computation; it stores the right for
                // LLM: this stage.
                let right = Constraint::new(vec![
                    CondensedConfiguration::new(vec![
                        Part::new(labels(b, 2), 1).unwrap(),
                        Part::new(labels(a, 2), 2).unwrap(),
                    ])
                    .unwrap(),
                ]);
                // LLM: Align counted input and output parts and retain only permitted joint
                // LLM: label combinations.
                for mask in 0..16 {
                    // LLM: Prepare `permitted` for the following computation; it stores the
                    // LLM: permitted for this stage.
                    let permitted = |a: u8, b: u8| mask & (1 << (a * 2 + b - 2)) != 0;
                    let result = product_with(
                        &left,
                        &right,
                        |a, b| permitted(a, b).then_some((a, b)),
                        &Cancellation::default(),
                    )
                    .unwrap();
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for mut code in 0..64 {
                        // LLM: Queue the parent pairs for this bounded generation batch.
                        let mut pairs = Vec::new();
                        for _ in 0..3 {
                            let value = code % 4;
                            code /= 4;
                            pairs.push((value / 2, 2 + value % 2));
                        }
                        // LLM: Construct the independent reference result for this fixture.
                        let expected = pairs.iter().all(|&(a, b)| permitted(a, b))
                            && accepts(
                                &left,
                                &Configuration::from_occurrences(pairs.iter().map(|p| p.0))
                                    .unwrap(),
                            )
                            && accepts(
                                &right,
                                &Configuration::from_occurrences(pairs.iter().map(|p| p.1))
                                    .unwrap(),
                            );
                        // LLM: Compare the implementation result with the independently
                        // LLM: prepared reference for this fixture.
                        assert_eq!(
                            accepts(&result, &Configuration::from_occurrences(pairs).unwrap()),
                            expected
                        );
                    }
                }
            }
        }
    }
}
