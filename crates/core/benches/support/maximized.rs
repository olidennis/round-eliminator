//! LLM: Verify the hand-written fixtures; this does not compute maximization.

use round_eliminator_3::model::{Configuration, LabelId};
use round_eliminator_3::{
    algorithms::configurations::accepts,
    model::{CondensedConfiguration, Problem},
};

/// LLM: Independently certify that a hand-written passive fixture lists every universal
/// LLM: maximal family. Used by benchmarks that rely on the maximized containment shortcut.
pub fn assert_maximized(problem: &Problem) {
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = problem.output();
    for &degree in problem.graph_class().passive() {
        // LLM: Prepare `targets` for the following computation; it stores the targets for this
        // LLM: stage.
        let targets = output.passive().configurations_at_degree(degree);
        if output.labels().len() > 2 {
            // LLM: These fixtures are universal: one full-alphabet part per degree.

            if !targets.iter().any(|target| {
                target.parts().len() == 1
                    && target.parts()[0].labels().len() == output.labels().len()
            }) {
                assert_eq!(output.labels().len(), 3, "unsupported fixture");
                check_three_labels(output.passive(), degree);
            }
            continue;
        }
        // LLM: With two labels every condensed configuration permits an interval of
        // LLM: counts of label 0. A maximized constraint must contain each allowed
        // LLM: interval in one of its listed targets. Test ALL possible intervals,
        // LLM: rather than only the families exercised by the solver's witness.

        let allowed: Vec<_> = (0..=degree)
            .map(|count| {
                accepts(
                    output.passive(),
                    &Configuration::new(vec![(LabelId(0), count), (LabelId(1), degree - count)])
                        .unwrap(),
                )
            })
            .collect();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for low in 0..=degree {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for high in low..=degree {
                // LLM: Prepare `covered` for the following computation; it stores the covered
                // LLM: for this stage.
                let covered = allowed[low as usize..=high as usize]
                    .iter()
                    .all(|&value| value);
                let contained = targets.iter().any(|target| {
                    let (minimum, maximum) = interval(target);
                    minimum <= low && high <= maximum
                });
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    covered, contained,
                    "not maximized: degree {degree}, [{low}, {high}]"
                );
            }
        }
    }
}

/// LLM: Exhaustively verify maximality over a three-label alphabet. Used by the benchmark
/// LLM: fixture checker as an independent oracle.
fn check_three_labels(constraint: &round_eliminator_3::model::Constraint, degree: u32) {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use std::collections::BTreeSet;
    // LLM: Enumerate all unordered boxes over the seven nonempty label subsets.
    // LLM: Compare semantic coverage with containment in a listed maximal box.
    /// LLM: Extend the current enumeration prefix, visit complete choices, and restore it when
    /// LLM: backtracking. Used by the enclosing exact traversal or independent fixture oracle.
    fn visit(
        constraint: &round_eliminator_3::model::Constraint,
        remaining: u32,
        first: u32,
        masks: &mut Vec<u32>,
    ) {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if remaining != 0 {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for mask in first..8 {
                masks.push(mask);
                visit(constraint, remaining - 1, mask, masks);
                masks.pop();
            }
            return;
        }
        // LLM: Prepare `configurations` for the following computation; it stores the
        // LLM: configurations for this stage.
        let mut configurations = BTreeSet::from([[0u32; 3]]);
        for &mask in masks.iter() {
            // LLM: Update the current working state before continuing this operation.
            configurations = configurations
                .iter()
                .flat_map(|counts| {
                    // LLM: Update the current working state before continuing this operation.
                    (0..3)
                        .filter(move |label| mask & (1 << label) != 0)
                        .map(move |label| {
                            let mut next = *counts;
                            next[label] += 1;
                            next
                        })
                })
                .collect();
        }
        // LLM: Prepare `covered` for the following computation; it stores the covered for this
        // LLM: stage.
        let covered = configurations.iter().all(|counts| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            accepts(
                constraint,
                &Configuration::new(
                    counts
                        .iter()
                        .enumerate()
                        .map(|(label, &count)| (LabelId(label as u32), count))
                        .collect(),
                )
                .unwrap(),
            )
        });
        // LLM: Prepare `contained` for the following computation; it stores the contained for
        // LLM: this stage.
        let contained = constraint
            .configurations_at_degree(masks.len() as u32)
            .iter()
            .any(|target| {
                // LLM: Prepare `capacities` for the following computation; it stores the
                // LLM: capacities for this stage.
                let mut capacities: Vec<_> = target
                    .parts()
                    .iter()
                    .map(|part| part.multiplicity())
                    .collect();
                fits(masks, target, &mut capacities)
            });
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(covered, contained, "not maximized: {masks:?}");
    }
    // LLM: Tiny exhaustive assignment oracle, deliberately separate from the flow
    // LLM: implementation under benchmark. It runs only outside timed requests.
    /// LLM: Check a concrete occurrence assignment against one candidate family. Used by the
    /// LLM: small exhaustive maximized-fixture oracle.
    fn fits(masks: &[u32], target: &CondensedConfiguration, capacities: &mut [u32]) -> bool {
        // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
        // LLM: traversal to backtrack or continue.
        let Some((&mask, rest)) = masks.split_first() else {
            return true;
        };
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for index in 0..capacities.len() {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if capacities[index] > 0
                && (0..3).all(|label| {
                    mask & (1 << label) == 0
                        || target.parts()[index].labels().contains(LabelId(label))
                })
            {
                // LLM: Update the current working state before continuing this operation.
                capacities[index] -= 1;
                let matched = fits(rest, target, capacities);
                capacities[index] += 1;
                if matched {
                    return true;
                }
            }
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        false
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    visit(constraint, degree, 1, &mut Vec::new());
}

/// LLM: Construct an ordered label interval for compact hand-written maximized fixtures. Used
/// LLM: by the benchmark support module.
fn interval(configuration: &CondensedConfiguration) -> (u32, u32) {
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let minimum = configuration
        .parts()
        .iter()
        .filter(|part| part.labels().len() == 1 && part.labels().contains(LabelId(0)))
        .map(|part| part.multiplicity())
        .sum();
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = configuration
        .parts()
        .iter()
        .filter(|part| part.labels().contains(LabelId(0)))
        .map(|part| part.multiplicity())
        .sum();
    (minimum, maximum)
}
