//! Complete passive maxima, kept separately from the original problem.
//! Unrestricted mappings use output sets. Restricted mappings maximize the
//! condensed joint relation, retaining the original input promise independently.

mod antichain;
mod combination;
mod engine;
mod history;
mod packed;
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

/// A complete result for the submitted problem snapshot, not a replacement
/// problem. IDs refer to that problem's name tables. Cancelled work returns no
/// result. Joint maximality must not be mistaken for output-only maximality.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "constraint", rename_all = "snake_case")]
pub enum MaximizationResult {
    OutputOnly(Constraint),
    InputOutput(Constraint<InputOutputLabel>),
}

pub fn compute(problem: &Problem) -> anyhow::Result<MaximizationResult> {
    compute_with(
        problem,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

pub fn compute_with(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<MaximizationResult> {
    computation.report(Phase::Preparing, 0)?;
    let output = Constraint::new(
        problem
            .graph_class()
            .passive()
            .iter()
            .flat_map(|&d| {
                problem
                    .output()
                    .passive()
                    .configurations_at_degree(d)
                    .iter()
                    .cloned()
            })
            .collect(),
    );
    if matches!(problem.mapping(), OutputMapping::Unrestricted) {
        return Ok(MaximizationResult::OutputOnly(maximize_constraint_with(
            &output,
            computation,
        )?));
    }
    let input = Constraint::new(prepare_passive_inputs(problem, &computation.cancellation)?);
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
    Ok(MaximizationResult::InputOutput(maximize_constraint_with(
        &joint,
        computation,
    )?))
}

/// Normalize working parts without changing model-wide equality or entry order.
/// The same engine is used for ordinary outputs and temporary joint labels.
pub fn maximize_constraint_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Constraint<L>> {
    let maximum = if let Some(maximum) = packed::maximum_with(constraint, computation)? {
        maximum
    } else {
        let mut initial = Vec::new();
        for family in constraint.configurations() {
            computation.cancellation.check()?;
            initial.push(Configuration::new(
                family
                    .parts()
                    .iter()
                    .map(|part| (part.labels().clone(), part.multiplicity()))
                    .collect(),
            )?);
        }
        maximize_custom_with(
            &initial,
            &SetOperations { allow_empty: false },
            computation,
            None,
        )?
    };
    let mut result = Vec::with_capacity(maximum.len());
    for configuration in maximum {
        computation.cancellation.check()?;
        let parts = configuration
            .multiplicities()
            .iter()
            .map(|(labels, count): &(LabelSet<L>, u32)| Part::new(labels.clone(), *count))
            .collect::<Result<Vec<_>, _>>()?;
        result.push(CondensedConfiguration::new(parts)?);
    }
    Ok(Constraint::new(result))
}

#[cfg(test)]
mod tests;
