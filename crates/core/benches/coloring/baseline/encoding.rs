//! LLM: Incremental SAT over color/input output-set memberships.

use rustsat::{
    solvers::{SolveIncremental, SolverResult},
    types::{Lit, TernaryVal},
};
use std::collections::BTreeSet;

use super::{
    ColorBehavior, ColoringKind, ColoringWitness,
    prepare::{self, input_index, port_inputs},
};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        inputs::{LocalInput, active_inputs, prepare_passive_inputs},
        sat::Formula,
        solvability::{
            constraint_encoding::{Choice, encode_constraint},
            passive::{Emission, PassiveMode, forbidden_combination},
        },
    },
    computation::{Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Problem},
};

/// LLM: The incremental membership SAT state for one problem and coloring kind. It retains
/// LLM: output-membership variables, activation gates, encoded active obligations, passive
/// LLM: input rows, and refinement clauses across palette queries.
pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    kind: ColoringKind,
    formula: Formula<S>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // LLM: color -> original input -> possible output memberships.

    memberships: Vec<Vec<Vec<Choice>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    /// LLM: Prepare the original input contexts and an empty incremental coloring formula.
    /// LLM: Used by this frozen comparison kernel before any palette is solved.
    pub fn new(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Enumerating, 0)?;
        Ok(Self {
            problem,
            kind,
            formula: Formula::new(solver, computation.cancellation.clone()),
            inputs: prepare::input_labels(problem),
            active_inputs: active_inputs(problem, &computation.cancellation)?,
            passive_inputs: prepare_passive_inputs(problem, &computation.cancellation)?,
            memberships: Vec::new(),
            enabled: Vec::new(),
            encoded: BTreeSet::new(),
            iterations: 0,
        })
    }

    /// LLM: Allocate missing color memberships and their activation gates, retaining all
    /// LLM: previous variables and clauses. Used before querying a larger palette.
    fn extend(&mut self, colors: u32) -> anyhow::Result<()> {
        // LLM: Prepare `labels` for the following computation; it stores the labels for this
        // LLM: stage.
        let labels: BTreeSet<_> = self.problem.output().active().labels().collect();
        while self.memberships.len() < colors as usize {
            // LLM: Use the stored state for this operation without exposing mutable
            // LLM: representation details.
            self.formula.check()?;
            let enabled = self.formula.new_lit();
            self.enabled.push(enabled);
            let mut behavior = Vec::new();
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            for &input in &self.inputs {
                // LLM: Prepare `outputs` for the following computation; it stores the outputs
                // LLM: for this stage.
                let mut outputs = Vec::new();
                for &label in &labels {
                    // LLM: Use the stored state for this operation without exposing mutable
                    // LLM: representation details.
                    self.formula.check()?;
                    if self.problem.output_allowed(input, label) {
                        let literal = self.formula.new_lit();
                        self.formula.add_clause(&[!literal, enabled])?;
                        outputs.push(Choice { label, literal });
                    }
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                behavior.push(outputs);
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            self.memberships.push(behavior);
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(())
    }

    /// LLM: Read true output memberships into the appropriate color/input behavior tables.
    /// LLM: Used after SAT to verify or return a strategy certificate.
    fn witness(&self, colors: u32) -> anyhow::Result<ColoringWitness> {
        // LLM: Prepare `behaviors` for the following computation; it stores the behaviors for
        // LLM: this stage.
        let mut behaviors = Vec::new();
        for behavior in &self.memberships[..colors as usize] {
            // LLM: Prepare `outputs_by_input` for the following computation; it stores the
            // LLM: outputs by input for this stage.
            let mut outputs_by_input = Vec::new();
            for choices in behavior {
                // LLM: Use the stored state for this operation without exposing mutable
                // LLM: representation details.
                self.formula.check()?;
                let mut outputs = Vec::new();
                for choice in choices {
                    if self.formula.solver.lit_val(choice.literal)? == TernaryVal::True {
                        outputs.push(choice.label);
                    }
                }
                outputs_by_input.push(LabelSet::new(outputs));
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            behaviors.push(ColorBehavior { outputs_by_input });
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(ColoringWitness::LocalColors { behaviors })
    }

    /// LLM: Query one palette, checking candidate active/passive validity and refining
    /// LLM: violated obligations until a witness or UNSAT. Used by fixed decisions and
    /// LLM: consecutive maximum search; interruption remains a separate error.
    pub fn solve(
        &mut self,
        colors: u32,
        mode: PassiveMode,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Option<ColoringWitness>> {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Encoding, self.iterations)?;
        self.extend(colors)?;
        // LLM: Smaller queries disable extra colors, including their memberships.
        // LLM: Learned constraints remain valid for every subsequent palette size.

        let assumptions: Vec<_> = self
            .enabled
            .iter()
            .enumerate()
            .map(|(index, &enabled)| {
                if index < colors as usize {
                    enabled
                } else {
                    !enabled
                }
            })
            .collect();
        // LLM: Ask the incremental solver under the current activation gates; interruption is
        // LLM: not a negative proof.
        loop {
            // LLM: Update the current working state before continuing this operation.
            computation.report(Phase::Solving, self.iterations)?;
            match self.formula.solver.solve_assumps(&assumptions)? {
                SolverResult::Unsat => return Ok(None),
                SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
                SolverResult::Sat => {}
            }
            self.iterations = self.iterations.saturating_add(1);
            computation.report(Phase::Verifying, self.iterations)?;
            // LLM: Construct a checkable strategy from the current output choices.
            let witness = self.witness(colors)?;
            let ColoringWitness::LocalColors { behaviors } = &witness else {
                unreachable!("local-color search produces local-color witnesses");
            };
            let mut failed = None;
            // LLM: Check whether these local output domains can jointly realize some valid
            // LLM: active configuration.
            for (context, input) in self.active_inputs.iter().enumerate() {
                // LLM: Prepare `inputs` for the following computation; it stores the inputs
                // LLM: for this stage.
                let inputs = port_inputs(input);
                if !prepare::active_colors(
                    &inputs,
                    self.kind,
                    colors,
                    &computation.cancellation,
                    &mut |palette| {
                        // LLM: Associate each occurrence with its available outputs before
                        // LLM: checking a whole configuration.
                        let positions: Vec<_> = inputs
                            .iter()
                            .zip(palette)
                            .map(|(&input, &color)| {
                                behaviors[color as usize].outputs_by_input[input_index(input)]
                                    .clone()
                            })
                            .collect();
                        // LLM: Check whether these local output domains can jointly realize
                        // LLM: some valid active configuration.
                        if !allows_positions_with(
                            self.problem.output().active(),
                            &positions,
                            &computation.cancellation,
                        )? {
                            failed = Some((context, palette.to_vec()));
                            return Ok(false);
                        }
                        // LLM: Return the result of this completed search or conversion to its
                        // LLM: caller.
                        Ok(true)
                    },
                )? {
                    break;
                }
            }
            // LLM: Require a whole valid active output by matching available per-port choices
            // LLM: to condensed part capacities.
            if let Some((context, palette)) = failed {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if !self.encoded.insert((context, palette.clone())) {
                    anyhow::bail!("Active matching encoding disagrees with coloring verification.");
                }
                let input = &self.active_inputs[context];
                let inputs = port_inputs(input);
                // LLM: Associate each occurrence with its available outputs before checking a
                // LLM: whole configuration.
                let positions: Vec<_> = inputs
                    .iter()
                    .zip(&palette)
                    .map(|(&input, &color)| {
                        self.memberships[color as usize][input_index(input)].clone()
                    })
                    .collect();
                // LLM: Prepare activation gates so cached clauses remain usable when the
                // LLM: palette changes.
                let enabled: BTreeSet<_> = palette
                    .iter()
                    .map(|&color| self.enabled[color as usize])
                    .collect();
                computation.report(Phase::Encoding, self.iterations)?;
                // LLM: Require a whole valid active output by matching available per-port
                // LLM: choices to condensed part capacities.
                encode_constraint(
                    &mut self.formula,
                    self.problem.output().active(),
                    input.degree,
                    &positions,
                    &enabled.into_iter().collect::<Vec<_>>(),
                )?;
                continue;
            }
            // LLM: Collect available outputs with their input/color descriptors and source
            // LLM: provenance.
            let mut emissions = Vec::new();
            for (color, behavior) in self.memberships[..colors as usize].iter().enumerate() {
                // LLM: Visit the color placements required by the auxiliary promise while
                // LLM: retaining original input context.
                for (&input, choices) in self.inputs.iter().zip(behavior) {
                    // LLM: Visit the color placements required by the auxiliary promise while
                    // LLM: retaining original input context.
                    for choice in choices {
                        // LLM: Use the stored state for this operation without exposing
                        // LLM: mutable representation details.
                        self.formula.check()?;
                        if self.formula.solver.lit_val(choice.literal)? == TernaryVal::True {
                            emissions.push(Emission {
                                input: (input, color as u32),
                                output: choice.label,
                                source: choice.literal,
                            });
                        }
                    }
                }
            }
            // LLM: Preserve the violation or conflict needed to reject an invalid search
            // LLM: branch.
            let mut forbidden = None;
            prepare::passive_families(
                &self.passive_inputs,
                self.kind,
                colors,
                &computation.cancellation,
                &mut |family| {
                    // LLM: Find the emission origins of a passive violation so refinement
                    // LLM: blocks that simultaneous choice.
                    forbidden = forbidden_combination(
                        self.problem.output().passive(),
                        std::slice::from_ref(family),
                        &emissions,
                        mode,
                        &computation.cancellation,
                    )?;
                    Ok(forbidden.is_none())
                },
            )?;
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            if let Some(sources) = forbidden {
                // LLM: Add the necessary SAT restriction; a refinement clause forbids the
                // LLM: selected violating combination.
                self.formula.add_clause(
                    &sources
                        .into_iter()
                        .map(|source| !source)
                        .collect::<Vec<_>>(),
                )?;
                continue;
            }
            // LLM: Return the complete strategy certificate after all required contexts have
            // LLM: been covered.
            return Ok(Some(witness));
        }
    }
}
