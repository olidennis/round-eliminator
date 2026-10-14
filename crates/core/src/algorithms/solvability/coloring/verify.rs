//! LLM: Witness verification using flow and exact passive checks, without SAT.

use super::{
    ColoringKind, ColoringWitness, distance_two, order,
    prepare::{self, input_index, port_inputs},
};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        inputs::{active_inputs, prepare_passive_inputs},
        solvability::passive::{Emission, PassiveMode, forbidden_combination},
    },
    computation::Cancellation,
    model::Problem,
};

/// LLM: `repeat` checks an unbounded certificate: a repeated local behavior or the
/// LLM: distance-two smaller/larger strategy. Otherwise check a fixed numeric palette. Used by
/// LLM: coloring solvability.
pub fn verify_witness(
    problem: &Problem,
    kind: ColoringKind,
    witness: &ColoringWitness,
    repeat: bool,
) -> bool {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    verify_with(problem, kind, witness, repeat, &Cancellation::default()).unwrap_or(false)
}

/// LLM: Validate witness shape, permissions, active color/input contexts, and passive output
/// LLM: families without SAT. Used by the public coloring certificate checker.
fn verify_with(
    problem: &Problem,
    kind: ColoringKind,
    witness: &ColoringWitness,
    repeat: bool,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    // LLM: Dispatch neighbor-information and order certificates to their dedicated independent
    // LLM: checkers.
    if kind == ColoringKind::DistanceTwo {
        return if repeat {
            order::verify_with(problem, witness, cancellation)
        } else {
            distance_two::verify_with(problem, witness, cancellation)
        };
    }
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::LocalColors { behaviors } = witness else {
        return Ok(false);
    };
    let minimum = prepare::minimum(problem, kind)?;
    // LLM: Check that computed indices and counts fit the model's integer representation.
    let colors = if repeat {
        // LLM: An unbounded local-color witness must contain exactly one repeating behavior,
        // LLM: rather than a finite palette table.
        if behaviors.len() != 1 {
            return Ok(false);
        }
        // LLM: Update the current working state before continuing this operation.
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
        u32::try_from(behaviors.len())?
    };
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if colors < minimum {
        return Ok(false);
    }
    let inputs = prepare::input_labels(problem);
    // LLM: Process the relevant label choices while preserving their table identities.
    for behavior in behaviors {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if behavior.outputs_by_input.len() != inputs.len() {
            return Ok(false);
        }
        // LLM: Process the relevant label choices while preserving their table identities.
        for (&input, outputs) in inputs.iter().zip(&behavior.outputs_by_input) {
            // LLM: Process the relevant label choices while preserving their table identities.
            for output in outputs.iter() {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                if output.0 as usize >= problem.output().labels().len()
                    || !problem.output_allowed(input, output)
                {
                    return Ok(false);
                }
            }
        }
    }
    // LLM: Select the one repeated table for an unbounded witness or the actual color row for
    // LLM: a numerical witness.
    let behavior = |color: u32| &behaviors[if repeat { 0 } else { color as usize }];
    for input in active_inputs(problem, cancellation)? {
        // LLM: Prepare `inputs` for the following computation; it stores the inputs for this
        // LLM: stage.
        let inputs = port_inputs(&input);
        if !prepare::active_colors(&inputs, kind, colors, cancellation, &mut |palette| {
            // LLM: Associate each occurrence with its available outputs before checking a
            // LLM: whole configuration.
            let positions: Vec<_> = inputs
                .iter()
                .zip(palette)
                .map(|(&input, &color)| {
                    behavior(color).outputs_by_input[input_index(input)].clone()
                })
                .collect();
            // LLM: Check whether these local output domains can jointly realize some valid
            // LLM: active configuration.
            Ok(allows_positions_with(
                problem.output().active(),
                &positions,
                cancellation,
            )?)
        })? {
            return Ok(false);
        }
    }
    // LLM: Collect available outputs with their input/color descriptors and source provenance.
    let mut emissions = Vec::new();
    for color in 0..colors {
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for (&input, outputs) in inputs.iter().zip(&behavior(color).outputs_by_input) {
            emissions.extend(outputs.iter().map(|output| Emission {
                input: (input, color),
                output,
                source: (),
            }));
        }
    }
    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    prepare::passive_families(
        &prepare_passive_inputs(problem, cancellation)?,
        kind,
        colors,
        cancellation,
        &mut |family| {
            // LLM: Find the emission origins of a passive violation so refinement blocks that
            // LLM: simultaneous choice.
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
