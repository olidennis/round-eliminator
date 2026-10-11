//! Order certificates for unbounded distance-two coloring solvability.
//!
//! Replace each original input with two labels: neighbor smaller / larger than
//! self. Active nodes must handle every assignment of these comparisons; passive
//! endpoints have opposite comparisons. These constraints are small and remain
//! condensed. The ordinary zero-round solver supplies configuration-based rules,
//! whose emissions give the two output-set tables.
//!
//! Completeness under our local/tree semantics follows from finite Ramsey:
//! classify each unordered pair of numeric colors by the two directional output
//! tables (over all original inputs). There are finitely many classifications.
//! A sufficiently large palette has a homogeneous subset of size max_degree + 1
//! (or 2 when needed for an edge). Choosing each possible center rank in this
//! subset realizes every smaller/larger neighborhood pattern. Thus solvability
//! for arbitrarily large palettes implies this certificate; failure means the
//! numeric-palette maximum is finite. No Ramsey bound needs to be computed.

use super::{ColorBehavior, ColoringKind, ColoringWitness, prepare};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        solvability::{
            inputs::{active_inputs, prepare_passive_inputs},
            passive::{Emission, PassiveMode, forbidden_combination},
            zero_round::{self, ZeroRoundResult},
        },
    },
    computation::{Cancellation, Computation, Phase},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, Input, LabelId, LabelSet,
        OutputMapping, Part, Problem,
    },
};

// Temporary label 2*i denotes a smaller neighbor, 2*i+1 a larger neighbor.
fn tag(input: Option<LabelId>, larger: u32) -> LabelId {
    LabelId(2 * prepare::input_index(input) as u32 + larger)
}

fn input_problem(problem: &Problem, cancellation: &Cancellation) -> anyhow::Result<Problem> {
    prepare::minimum(problem, ColoringKind::DistanceTwo)?;
    cancellation.check()?;
    let inputs = prepare::input_labels(problem);
    let count = inputs
        .len()
        .checked_mul(2)
        .ok_or_else(|| anyhow::anyhow!("Order input label count overflows usize."))?;
    u32::try_from(count).map_err(|_| anyhow::anyhow!("Order input label count exceeds u32."))?;
    let mut names = Vec::new();
    let mut mapping = Vec::new();
    for (index, &input) in inputs.iter().enumerate() {
        let mut outputs = Vec::new();
        for output in 0..problem.output().labels().len() {
            cancellation.check()?;
            let output = LabelId(output as u32);
            if problem.output_allowed(input, output) {
                outputs.push(output);
            }
        }
        for relation in ["smaller", "larger"] {
            names.push(format!("{index}:{relation}"));
            mapping.push(LabelSet::new(outputs.clone()));
        }
    }
    let mut active = Vec::new();
    match problem.input() {
        Input::Trivial => {
            for &degree in problem.graph_class().active() {
                cancellation.check()?;
                active.push(CondensedConfiguration::new(vec![Part::new(
                    LabelSet::new(vec![tag(None, 0), tag(None, 1)]),
                    degree,
                )?])?);
            }
        }
        Input::Explicit(input) => {
            for configuration in input.active().configurations() {
                cancellation.check()?;
                let mut parts = Vec::new();
                for part in configuration.parts() {
                    let mut labels = Vec::new();
                    for input in part.labels().iter() {
                        cancellation.check()?;
                        labels.extend([tag(Some(input), 0), tag(Some(input), 1)]);
                    }
                    parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
                }
                active.push(CondensedConfiguration::new(parts)?);
            }
        }
    }
    let mut passive = Vec::new();
    for input in prepare_passive_inputs(problem, cancellation)? {
        let slots: Vec<_> = input
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        debug_assert_eq!(slots.len(), 2);
        for first in 0..2 {
            let mut parts = Vec::new();
            for (port, labels) in slots.iter().enumerate() {
                let mut tagged = Vec::new();
                for input in labels.iter() {
                    cancellation.check()?;
                    tagged.push(tag(input, if port == 0 { first } else { 1 - first }));
                }
                parts.push(Part::new(LabelSet::new(tagged), 1)?);
            }
            passive.push(CondensedConfiguration::new(parts)?);
        }
    }
    Problem::new(
        problem.graph_class().clone(),
        Input::Explicit(ConstraintPair::new(
            names,
            Constraint::new(active),
            Constraint::new(passive),
        )?),
        problem.output().clone(),
        OutputMapping::Restricted(mapping),
    )
    .map_err(Into::into)
}

pub(super) fn check_with(
    problem: &Problem,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<ColoringWitness>> {
    computation.report(Phase::Enumerating, 0)?;
    let input = input_problem(problem, &computation.cancellation)?;
    let ZeroRoundResult::Solvable { strategy } = zero_round::check_with(&input, mode, computation)?
    else {
        return Ok(None);
    };
    let mut outputs = vec![Vec::new(); 2 * prepare::input_labels(problem).len()];
    for rule in strategy {
        let labels = rule.input.expect("order input is explicit");
        for (input, output) in labels.into_iter().zip(rule.output) {
            computation.cancellation.check()?;
            outputs[input.0 as usize].push(output);
        }
    }
    Ok(Some(ColoringWitness::ColorOrder {
        smaller: ColorBehavior {
            outputs_by_input: outputs
                .iter()
                .step_by(2)
                .cloned()
                .map(LabelSet::new)
                .collect(),
        },
        larger: ColorBehavior {
            outputs_by_input: outputs
                .into_iter()
                .skip(1)
                .step_by(2)
                .map(LabelSet::new)
                .collect(),
        },
    }))
}

/// Verify the two output tables against every comparison pattern, without SAT.
/// Numeric colors and huge palettes are never enumerated or materialized.
pub(super) fn verify_with(
    problem: &Problem,
    witness: &ColoringWitness,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    let ColoringWitness::ColorOrder { smaller, larger } = witness else {
        return Ok(false);
    };
    let inputs = prepare::input_labels(problem);
    for behavior in [smaller, larger] {
        if behavior.outputs_by_input.len() != inputs.len() {
            return Ok(false);
        }
        for (&input, outputs) in inputs.iter().zip(&behavior.outputs_by_input) {
            for output in outputs.iter() {
                cancellation.check()?;
                if output.0 as usize >= problem.output().labels().len()
                    || !problem.output_allowed(input, output)
                {
                    return Ok(false);
                }
            }
        }
    }
    let input = input_problem(problem, cancellation)?;
    let outputs: Vec<_> = smaller
        .outputs_by_input
        .iter()
        .zip(&larger.outputs_by_input)
        .flat_map(|(smaller, larger)| [smaller, larger])
        .collect();
    for local in active_inputs(&input, cancellation)? {
        let positions = local
            .labels
            .expect("order input is explicit")
            .iter()
            .map(|label| outputs[label.0 as usize].clone())
            .collect::<Vec<_>>();
        if !allows_positions_with(problem.output().active(), &positions, cancellation)? {
            return Ok(false);
        }
    }
    let mut emissions = Vec::new();
    for (id, outputs) in outputs.into_iter().enumerate() {
        cancellation.check()?;
        emissions.extend(outputs.iter().map(|output| Emission {
            input: Some(LabelId(id as u32)),
            output,
            source: (),
        }));
    }
    Ok(forbidden_combination(
        problem.output().passive(),
        &prepare_passive_inputs(&input, cancellation)?,
        &emissions,
        PassiveMode::General,
        cancellation,
    )?
    .is_none())
}
