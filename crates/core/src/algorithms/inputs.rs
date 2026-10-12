//! Original input configurations shared by diagram and solvability analyses.
use crate::algorithms::configurations::concrete_multisets_with;
use crate::computation::{Cancellation, Cancelled};
use crate::model::{CondensedConfiguration, Input, LabelId, LabelSet, Part, Problem};

pub(crate) struct LocalInput {
    pub degree: u32,
    pub labels: Option<Vec<LabelId>>,
}

/// Canonical sorted multisets suffice because constraints are invariant under
/// port permutations. A strategy may still distinguish repeated occurrences
/// using local port order, and may depend on the entire multiset.
pub(crate) fn active_inputs(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<LocalInput>, Cancelled> {
    cancellation.check()?;
    Ok(match problem.input() {
        Input::Explicit(input) => {
            concrete_multisets_with(input.active(), problem.graph_class().active(), cancellation)?
                .into_iter()
                .map(|input| LocalInput {
                    degree: input.degree(),
                    labels: Some(input.occurrences().collect()),
                })
                .collect()
        }
        Input::Trivial => problem
            .graph_class()
            .active()
            .iter()
            .map(|&degree| LocalInput {
                degree,
                labels: None,
            })
            .collect(),
    })
}

/// Extract the possible input configurations from the passive constraint.
/// Configurations are kept condensed.
/// We will use these condensed input configurations to construct the possible
/// output configurations that a candidate strategy could create on the passive side,
/// by replacing each input label with all possible output labels mapped to it.
pub(crate) fn prepare_passive_inputs(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<CondensedConfiguration<Option<LabelId>>>, Cancelled> {
    cancellation.check()?;
    let graph = problem.graph_class();
    let mut configurations = Vec::new();
    match problem.input() {
        Input::Explicit(input) => {
            for &degree in graph.passive() {
                for configuration in input.passive().configurations_at_degree(degree) {
                    cancellation.check()?;
                    configurations.push(
                        CondensedConfiguration::new(
                            configuration
                                .parts()
                                .iter()
                                .map(|part| {
                                    Part::new(
                                        LabelSet::new(part.labels().iter().map(Some).collect()),
                                        part.multiplicity(),
                                    )
                                    .expect("nonempty label set")
                                })
                                .collect(),
                        )
                        .expect("degree preserved"),
                    );
                }
            }
        }
        Input::Trivial => {
            for &degree in graph.passive() {
                cancellation.check()?;
                configurations.push(
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![None]), degree).expect("nonempty label set"),
                    ])
                    .expect("single count fits u32"),
                );
            }
        }
    }
    Ok(configurations)
}
