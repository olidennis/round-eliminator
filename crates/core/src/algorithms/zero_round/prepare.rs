//! Prepare active rules and condensed passive input families for zero-round computation.

use super::LocalRule;
use crate::algorithms::configurations::concrete_multisets_with;
use crate::computation::{Cancellation, Cancelled};
use crate::model::LabelId;
use crate::model::{CondensedConfiguration, Input, LabelSet, Part, Problem};

/// Prepare one local rule for each concrete active input configuration at an allowed degree.
/// The output is initialized as an empty vector.
/// Note: condensed configurations get expanded into normal configurations, 
/// because the algorithm could output different configurations in each case.
pub(super) fn prepare_local_rules(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<LocalRule>, Cancelled> {
    cancellation.check()?;
    // Constraints are invariant under port permutations. Restricting a
    // strategy to one canonical ordering of each input multiset preserves
    // active validity and can only reduce the possible passive emissions.
    // Thus sorted inputs suffice for existence, even for repeated labels.
    let graph = problem.graph_class();
    Ok(match problem.input() {
        Input::Explicit(input) => {
            concrete_multisets_with(input.active(), graph.active(), cancellation)?
                .into_iter()
                .map(|input| LocalRule {
                    degree: input.degree(),
                    input: Some(input.occurrences().collect()),
                    output: Vec::new(),
                })
                .collect()
        }
        Input::Trivial => graph
            .active()
            .iter()
            .map(|&degree| LocalRule {
                degree,
                input: None,
                output: Vec::new(),
            })
            .collect(),
    })
}

/// Extract the possible input configurations from the passive constraint.
/// Configurations are kept condensed.
/// We will use these condensed input configurations to construct the possible
/// output configurations that the zero round algorithm could create on the passive side,
/// by replacing each input label with all possible output labels mapped to it.
pub(super) fn prepare_passive_inputs(
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

/// Candidate output labels at this degree, filtered by the mapping restriction.
pub(super) fn output_domain(
    problem: &Problem,
    degree: u32,
    input: Option<LabelId>,
) -> Vec<LabelId> {
    let labels: std::collections::BTreeSet<_> = problem
        .output()
        .active()
        .configurations_at_degree(degree)
        .iter()
        .flat_map(|configuration| configuration.parts())
        .flat_map(|part| part.labels().iter())
        .filter(|&output| problem.output_allowed(input, output))
        .collect();
    labels.into_iter().collect()
}
