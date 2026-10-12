//! Prepare the initially empty local rules and output domains for zero-round search.
use super::LocalRule;
pub(super) use crate::algorithms::inputs::prepare_passive_inputs;
use crate::computation::{Cancellation, Cancelled};
use crate::model::{LabelId, Problem};

/// One rule per concrete active input at an allowed degree; outputs start empty.
pub(super) fn prepare_local_rules(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<LocalRule>, Cancelled> {
    Ok(
        crate::algorithms::inputs::active_inputs(problem, cancellation)?
            .into_iter()
            .map(|input| LocalRule {
                degree: input.degree,
                input: input.labels,
                output: Vec::new(),
            })
            .collect(),
    )
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
