//! LLM: Order certificates for unbounded distance-two coloring solvability.
//! LLM: Replace each original input with two labels: neighbor smaller / larger than
//! LLM: self. Active nodes must handle every assignment of these comparisons; passive
//! LLM: endpoints have opposite comparisons. These constraints are small and remain
//! LLM: condensed. The ordinary zero-round solver supplies configuration-based rules,
//! LLM: whose emissions give the two output-set tables.
//! LLM: Completeness under our local/tree semantics follows from finite Ramsey:
//! LLM: classify each unordered pair of numeric colors by the two directional output
//! LLM: tables (over all original inputs). There are finitely many classifications.
//! LLM: A sufficiently large palette has a homogeneous subset of size max_degree + 1
//! LLM: (or 2 when needed for an edge). Choosing each possible center rank in this
//! LLM: subset realizes every smaller/larger neighborhood pattern. Thus solvability
//! LLM: for arbitrarily large palettes implies this certificate; failure means the
//! LLM: numeric-palette maximum is finite. No Ramsey bound needs to be computed.

use super::{ColorBehavior, ColoringKind, ColoringWitness, prepare};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        inputs::{active_inputs, prepare_passive_inputs},
        solvability::{
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

// LLM: Temporary label 2*i denotes a smaller neighbor, 2*i+1 a larger neighbor.
/// LLM: Encode an original input and smaller/larger comparison as one temporary input ID. Used
/// LLM: by order-based distance-two certificates.
fn tag(input: Option<LabelId>, larger: u32) -> LabelId {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    LabelId(2 * prepare::input_index(input) as u32 + larger)
}

/// LLM: Construct the auxiliary promised input problem carrying opposite endpoint color-order
/// LLM: comparisons. Used by the unbounded distance-two preflight and its independent
/// LLM: verifier.
fn input_problem(problem: &Problem, cancellation: &Cancellation) -> anyhow::Result<Problem> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    prepare::minimum(problem, ColoringKind::DistanceTwo)?;
    cancellation.check()?;
    let inputs = prepare::input_labels(problem);
    let count = inputs
        .len()
        .checked_mul(2)
        .ok_or_else(|| anyhow::anyhow!("Order input label count overflows usize."))?;
    u32::try_from(count).map_err(|_| anyhow::anyhow!("Order input label count exceeds u32."))?;
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let mut names = Vec::new();
    let mut mapping = Vec::new();
    // LLM: Process the relevant label choices while preserving their table identities.
    for (index, &input) in inputs.iter().enumerate() {
        // LLM: Prepare `outputs` for the following computation; it stores the outputs for this
        // LLM: stage.
        let mut outputs = Vec::new();
        for output in 0..problem.output().labels().len() {
            cancellation.check()?;
            let output = LabelId(output as u32);
            if problem.output_allowed(input, output) {
                outputs.push(output);
            }
        }
        // LLM: Create both smaller and larger tagged inputs for each original input, copying
        // LLM: its old output permissions.
        for relation in ["smaller", "larger"] {
            names.push(format!("{index}:{relation}"));
            mapping.push(LabelSet::new(outputs.clone()));
        }
    }
    // LLM: Allow every comparison pattern on the promised active input contexts.
    let mut active = Vec::new();
    match problem.input() {
        Input::Trivial => {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for &degree in problem.graph_class().active() {
                cancellation.check()?;
                active.push(CondensedConfiguration::new(vec![Part::new(
                    LabelSet::new(vec![tag(None, 0), tag(None, 1)]),
                    degree,
                )?])?);
            }
        }
        Input::Explicit(input) => {
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for configuration in input.active().configurations() {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                let mut parts = Vec::new();
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for part in configuration.parts() {
                    // LLM: Prepare `labels` for the following computation; it stores the
                    // LLM: labels for this stage.
                    let mut labels = Vec::new();
                    for input in part.labels().iter() {
                        cancellation.check()?;
                        labels.extend([tag(Some(input), 0), tag(Some(input), 1)]);
                    }
                    parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                active.push(CondensedConfiguration::new(parts)?);
            }
        }
    }
    // LLM: Require opposite smaller/larger tags at the two endpoints while retaining the
    // LLM: original passive input choices.
    let mut passive = Vec::new();
    for input in prepare_passive_inputs(problem, cancellation)? {
        // LLM: Prepare `slots` for the following computation; it stores the slots for this
        // LLM: stage.
        let slots: Vec<_> = input
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        debug_assert_eq!(slots.len(), 2);
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for first in 0..2 {
            // LLM: Prepare `parts` for the following computation; it stores the parts for this
            // LLM: stage.
            let mut parts = Vec::new();
            for (port, labels) in slots.iter().enumerate() {
                // LLM: Prepare `tagged` for the following computation; it stores the tagged
                // LLM: for this stage.
                let mut tagged = Vec::new();
                for input in labels.iter() {
                    cancellation.check()?;
                    tagged.push(tag(input, if port == 0 { first } else { 1 - first }));
                }
                parts.push(Part::new(LabelSet::new(tagged), 1)?);
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            passive.push(CondensedConfiguration::new(parts)?);
        }
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
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

/// LLM: Search for a smaller/larger neighbor-order strategy by reducing it to zero-round
/// LLM: solvability. Used as the distance-two unbounded-palette preflight; original inputs and
/// LLM: permissions remain available.
pub(super) fn check_with(
    problem: &Problem,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<ColoringWitness>> {
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Enumerating, 0)?;
    let input = input_problem(problem, &computation.cancellation)?;
    let ZeroRoundResult::Solvable { strategy } = zero_round::check_with(&input, mode, computation)?
    else {
        return Ok(None);
    };
    let mut outputs = vec![Vec::new(); 2 * prepare::input_labels(problem).len()];
    // LLM: Process the relevant label choices while preserving their table identities.
    for rule in strategy {
        let labels = rule.input.expect("order input is explicit");
        for (input, output) in labels.into_iter().zip(rule.output) {
            computation.cancellation.check()?;
            outputs[input.0 as usize].push(output);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
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

/// LLM: Independently check smaller/larger output tables against every valid comparison/input
/// LLM: context and reversed passive endpoint. Used to validate an unbounded distance-two
/// LLM: certificate without SAT.
pub(super) fn verify_with(
    problem: &Problem,
    witness: &ColoringWitness,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::ColorOrder { smaller, larger } = witness else {
        return Ok(false);
    };
    let inputs = prepare::input_labels(problem);
    // LLM: Process the relevant label choices while preserving their table identities.
    for behavior in [smaller, larger] {
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
    // LLM: Retain or prepare the original promised input semantics for this stage.
    let input = input_problem(problem, cancellation)?;
    let outputs: Vec<_> = smaller
        .outputs_by_input
        .iter()
        .zip(&larger.outputs_by_input)
        .flat_map(|(smaller, larger)| [smaller, larger])
        .collect();
    // LLM: Check whether these local output domains can jointly realize some valid active
    // LLM: configuration.
    for local in active_inputs(&input, cancellation)? {
        // LLM: Associate each occurrence with its available outputs before checking a whole
        // LLM: configuration.
        let positions = local
            .labels
            .expect("order input is explicit")
            .iter()
            .map(|label| outputs[label.0 as usize].clone())
            .collect::<Vec<_>>();
        // LLM: Check whether these local output domains can jointly realize some valid active
        // LLM: configuration.
        if !allows_positions_with(problem.output().active(), &positions, cancellation)? {
            return Ok(false);
        }
    }
    // LLM: Collect available outputs with their input/color descriptors and source provenance.
    let mut emissions = Vec::new();
    for (id, outputs) in outputs.into_iter().enumerate() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        emissions.extend(outputs.iter().map(|output| Emission {
            input: Some(LabelId(id as u32)),
            output,
            source: (),
        }));
    }
    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    Ok(forbidden_combination(
        problem.output().passive(),
        &prepare_passive_inputs(&input, cancellation)?,
        &emissions,
        PassiveMode::General,
        cancellation,
    )?
    .is_none())
}
