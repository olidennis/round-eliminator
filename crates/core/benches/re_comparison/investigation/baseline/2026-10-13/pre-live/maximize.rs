//! LLM: Complete passive maxima, kept separately from the original problem.
//! LLM: Unrestricted mappings use output sets. Restricted mappings maximize the
//! LLM: condensed joint relation, retaining the original input promise independently.

mod antichain;
mod combination;
mod engine;
pub use engine::{
    Combination, DerivationObserver, Operations, SetOperations, maximize_custom_with,
};

use crate::{
    algorithms::{configurations::product_with, inputs::prepare_passive_inputs},
    computation::{Cancellation, Computation, Phase},
    model::{
        CondensedConfiguration, Configuration, Constraint, InputOutputLabel, LabelSet,
        OutputMapping, Part, Problem,
    },
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// LLM: A completed passive maximum of the submitted snapshot, separate from the original
/// LLM: problem. Output-only maxima certify ordinary output families; joint maxima certify
/// LLM: permitted input/output rectangles and cannot certify output-only maximality.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "constraint", rename_all = "snake_case")]
pub enum MaximizationResult {
    OutputOnly(Constraint),
    InputOutput(Constraint<InputOutputLabel>),
}

/// LLM: Compute the passive maximum appropriate to this problem's permissions: ordinary
/// LLM: outputs for unrestricted mappings, joint input/output rectangles otherwise. Used by
/// LLM: the API and RE preparation. This wrapper uses a silent fresh context.
pub fn compute(problem: &Problem) -> anyhow::Result<MaximizationResult> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    compute_with(
        problem,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

/// LLM: Compute the passive maximum appropriate to this problem's permissions: ordinary
/// LLM: outputs for unrestricted mappings, joint input/output rectangles otherwise. Used by
/// LLM: the API and RE preparation. Progress and cancellation use the supplied context.
pub fn compute_with(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<MaximizationResult> {
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Preparing, 0)?;
    let output = Constraint::new(
        problem
            .graph_class()
            .passive()
            .iter()
            .flat_map(|&d| {
                // LLM: Update the current working state before continuing this operation.
                problem
                    .output()
                    .passive()
                    .configurations_at_degree(d)
                    .iter()
                    .cloned()
            })
            .collect(),
    );
    // LLM: Select the representation path that preserves the supplied input promise and
    // LLM: incidence permissions.
    if matches!(problem.mapping(), OutputMapping::Unrestricted) {
        return Ok(MaximizationResult::OutputOnly(maximize_constraint_with(
            &output,
            computation,
        )?));
    }
    let input = Constraint::new(prepare_passive_inputs(problem, &computation.cancellation)?);
    // LLM: Align counted input and output parts and retain only permitted joint label
    // LLM: combinations.
    let joint = product_with(
        &input,
        &output,
        |input, output| {
            problem
                .output_allowed(input, output)
                .then_some(InputOutputLabel { input, output })
        },
        &computation.cancellation,
    )?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(MaximizationResult::InputOutput(maximize_constraint_with(
        &joint,
        computation,
    )?))
}

/// LLM: Convert condensed rows to canonical counted algebra values, compute closure, and
/// LLM: return maximal condensed rows. Shared by ordinary output and joint input/output
/// LLM: maximization.
pub fn maximize_constraint_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Constraint<L>> {
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let mut initial = Vec::new();
    for family in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        initial.push(Configuration::new(
            family
                .parts()
                .iter()
                .map(|part| (part.labels().clone(), part.multiplicity()))
                .collect(),
        )?);
    }
    // LLM: Compute exact closure under the selected standard or dual algebra.
    let maximum = maximize_custom_with(
        &initial,
        &SetOperations { allow_empty: false },
        computation,
        None,
    )?;
    let mut result = Vec::with_capacity(maximum.len());
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for configuration in maximum {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let parts = configuration
            .multiplicities()
            .iter()
            .map(|(labels, count): &(LabelSet<L>, u32)| Part::new(labels.clone(), *count))
            .collect::<Result<Vec<_>, _>>()?;
        result.push(CondensedConfiguration::new(parts)?);
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(result))
}

#[cfg(test)]
mod tests;
