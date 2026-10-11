//! Verify the hand-written fixtures; this does not compute maximization.

use round_eliminator_3::model::{Configuration, LabelId};
use round_eliminator_3::{
    algorithms::configurations::accepts,
    model::{CondensedConfiguration, Problem},
};

pub fn assert_maximized(problem: &Problem) {
    let output = problem.output();
    for &degree in problem.graph_class().passive() {
        let targets = output.passive().configurations_at_degree(degree);
        if output.labels().len() > 2 {
            // These fixtures are universal: one full-alphabet part per degree.
            if !targets.iter().any(|target| {
                target.parts().len() == 1
                    && target.parts()[0].labels().len() == output.labels().len()
            }) {
                assert_eq!(output.labels().len(), 3, "unsupported fixture");
                check_three_labels(output.passive(), degree);
            }
            continue;
        }
        // With two labels every condensed configuration permits an interval of
        // counts of label 0. A maximized constraint must contain each allowed
        // interval in one of its listed targets. Test ALL possible intervals,
        // rather than only the families exercised by the solver's witness.
        let allowed: Vec<_> = (0..=degree)
            .map(|count| {
                accepts(
                    output.passive(),
                    &Configuration::new(vec![(LabelId(0), count), (LabelId(1), degree - count)])
                        .unwrap(),
                )
            })
            .collect();
        for low in 0..=degree {
            for high in low..=degree {
                let covered = allowed[low as usize..=high as usize]
                    .iter()
                    .all(|&value| value);
                let contained = targets.iter().any(|target| {
                    let (minimum, maximum) = interval(target);
                    minimum <= low && high <= maximum
                });
                assert_eq!(
                    covered, contained,
                    "not maximized: degree {degree}, [{low}, {high}]"
                );
            }
        }
    }
}

fn check_three_labels(constraint: &round_eliminator_3::model::Constraint, degree: u32) {
    use std::collections::BTreeSet;
    // Enumerate all unordered boxes over the seven nonempty label subsets.
    // Compare semantic coverage with containment in a listed maximal box.
    fn visit(
        constraint: &round_eliminator_3::model::Constraint,
        remaining: u32,
        first: u32,
        masks: &mut Vec<u32>,
    ) {
        if remaining != 0 {
            for mask in first..8 {
                masks.push(mask);
                visit(constraint, remaining - 1, mask, masks);
                masks.pop();
            }
            return;
        }
        let mut configurations = BTreeSet::from([[0u32; 3]]);
        for &mask in masks.iter() {
            configurations = configurations
                .iter()
                .flat_map(|counts| {
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
        let covered = configurations.iter().all(|counts| {
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
        let contained = constraint
            .configurations_at_degree(masks.len() as u32)
            .iter()
            .any(|target| {
                let mut capacities: Vec<_> = target
                    .parts()
                    .iter()
                    .map(|part| part.multiplicity())
                    .collect();
                fits(masks, target, &mut capacities)
            });
        assert_eq!(covered, contained, "not maximized: {masks:?}");
    }
    // Tiny exhaustive assignment oracle, deliberately separate from the flow
    // implementation under benchmark. It runs only outside timed requests.
    fn fits(masks: &[u32], target: &CondensedConfiguration, capacities: &mut [u32]) -> bool {
        let Some((&mask, rest)) = masks.split_first() else {
            return true;
        };
        for index in 0..capacities.len() {
            if capacities[index] > 0
                && (0..3).all(|label| {
                    mask & (1 << label) == 0
                        || target.parts()[index].labels().contains(LabelId(label))
                })
            {
                capacities[index] -= 1;
                let matched = fits(rest, target, capacities);
                capacities[index] += 1;
                if matched {
                    return true;
                }
            }
        }
        false
    }
    visit(constraint, degree, 1, &mut Vec::new());
}

fn interval(configuration: &CondensedConfiguration) -> (u32, u32) {
    let minimum = configuration
        .parts()
        .iter()
        .filter(|part| part.labels().len() == 1 && part.labels().contains(LabelId(0)))
        .map(|part| part.multiplicity())
        .sum();
    let maximum = configuration
        .parts()
        .iter()
        .filter(|part| part.labels().contains(LabelId(0)))
        .map(|part| part.multiplicity())
        .sum();
    (minimum, maximum)
}
