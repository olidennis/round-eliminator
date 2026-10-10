//! Prepare active rules and passive input multisets for zero-round computation.

use super::LocalRule;
use crate::algorithms::configurations::concrete_multisets_with;
use crate::computation::{Cancellation, Cancelled};
use crate::labels::LabelId;
use crate::problem::{Configuration, Problem};

/// Prepare one rule per active input multiset, or per degree for plain problems.
pub(super) fn contexts(
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
        Some(input) => concrete_multisets_with(input.active(), graph.active(), cancellation)?
            .into_iter()
            .map(|input| LocalRule {
                degree: input.degree(),
                input: Some(input.occurrences().collect()),
                output: Vec::new(),
            })
            .collect(),
        None => graph
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

/// Prepare the passive input multisets. `None` marks occurrences with no input
/// label for the zero-round checker on plain problems.
pub(super) fn passive_inputs(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<Configuration<Option<LabelId>>>, Cancelled> {
    cancellation.check()?;
    let graph = problem.graph_class();
    Ok(match problem.input() {
        Some(input) => concrete_multisets_with(input.passive(), graph.passive(), cancellation)?
            .into_iter()
            .map(|input| {
                Configuration::new(
                    input
                        .multiplicities()
                        .iter()
                        .map(|&(label, count)| (Some(label), count))
                        .collect(),
                )
                .expect("mapping preserves degree")
            })
            .collect(),
        None => graph
            .passive()
            .iter()
            .map(|&degree| Configuration::new(vec![(None, degree)]).expect("single count fits u32"))
            .collect(),
    })
}
