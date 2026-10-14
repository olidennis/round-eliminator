//! LLM: Prepare the initially empty local rules and output domains for zero-round search.

use super::LocalRule;
pub(super) use crate::algorithms::inputs::prepare_passive_inputs;
use crate::computation::{Cancellation, Cancelled};
use crate::model::{LabelId, Problem};

/// LLM: Create one rule for every concrete active input context, with output vectors initially
/// LLM: empty. Used by the zero-round port encoding before SAT chooses occurrence outputs.
pub(super) fn prepare_local_rules(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<LocalRule>, Cancelled> {
    // LLM: Prepare one canonical rule obligation for each promised active input context.
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

/// LLM: Collect active output labels at the requested degree and filter by the incidence
/// LLM: mapping. Used to allocate zero-round per-port choices.
pub(super) fn output_domain(
    problem: &Problem,
    degree: u32,
    input: Option<LabelId>,
) -> Vec<LabelId> {
    // LLM: Prepare `labels` for the following computation; it stores the labels for this
    // LLM: stage.
    let labels: std::collections::BTreeSet<_> = problem
        .output()
        .active()
        .configurations_at_degree(degree)
        .iter()
        .flat_map(|configuration| configuration.parts())
        .flat_map(|part| part.labels().iter())
        .filter(|&output| problem.output_allowed(input, output))
        .collect();
    // LLM: Update the current working state before continuing this operation.
    labels.into_iter().collect()
}
