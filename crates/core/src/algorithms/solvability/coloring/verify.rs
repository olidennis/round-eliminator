//! Witness verification using flow and exact passive checks, without SAT.

use super::{
    ColoringKind, ColoringWitness,
    prepare::{self, input_index, port_inputs},
};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        solvability::{
            inputs::{active_inputs, prepare_passive_inputs},
            passive::{Emission, PassiveMode, forbidden_combination},
        },
    },
    computation::Cancellation,
    model::Problem,
};

/// `repeat` certifies an unbounded result by using the first behavior for every
/// color. Checking enough copies to cover all degrees establishes repeatability.
pub fn verify_witness(
    problem: &Problem,
    kind: ColoringKind,
    witness: &ColoringWitness,
    repeat: bool,
) -> bool {
    verify_with(problem, kind, witness, repeat, &Cancellation::default()).unwrap_or(false)
}

fn verify_with(
    problem: &Problem,
    kind: ColoringKind,
    witness: &ColoringWitness,
    repeat: bool,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    let minimum = prepare::minimum(problem, kind)?;
    let colors = if repeat {
        if witness.behaviors.len() != 1 {
            return Ok(false);
        }
        problem
            .graph_class()
            .active()
            .iter()
            .chain(problem.graph_class().passive())
            .copied()
            .max()
            .unwrap_or(1)
            .max(1)
    } else {
        u32::try_from(witness.behaviors.len())?
    };
    if colors < minimum {
        return Ok(false);
    }
    let inputs = prepare::input_labels(problem);
    for behavior in &witness.behaviors {
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
    let behavior = |color: u32| &witness.behaviors[if repeat { 0 } else { color as usize }];
    for input in active_inputs(problem, cancellation)? {
        let inputs = port_inputs(&input);
        if !prepare::active_colors(&inputs, kind, colors, cancellation, &mut |palette| {
            let positions: Vec<_> = inputs
                .iter()
                .zip(palette)
                .map(|(&input, &color)| {
                    behavior(color).outputs_by_input[input_index(input)].clone()
                })
                .collect();
            Ok(allows_positions_with(
                problem.output().active(),
                &positions,
                cancellation,
            )?)
        })? {
            return Ok(false);
        }
    }
    let mut emissions = Vec::new();
    for color in 0..colors {
        for (&input, outputs) in inputs.iter().zip(&behavior(color).outputs_by_input) {
            emissions.extend(outputs.iter().map(|output| Emission {
                input: (input, color),
                output,
                source: (),
            }));
        }
    }
    prepare::passive_families(
        &prepare_passive_inputs(problem, cancellation)?,
        kind,
        colors,
        cancellation,
        &mut |family| {
            Ok(forbidden_combination(
                problem.output().passive(),
                std::slice::from_ref(family),
                &emissions,
                PassiveMode::General,
                cancellation,
            )?
            .is_none())
        },
    )
}
