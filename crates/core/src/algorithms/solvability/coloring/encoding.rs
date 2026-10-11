//! Incremental SAT over color/input output-set memberships.

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
        sat::Formula,
        solvability::{
            constraint_encoding::{Choice, encode_constraint},
            inputs::{LocalInput, active_inputs, prepare_passive_inputs},
            passive::{Emission, PassiveMode, forbidden_combination},
        },
    },
    computation::{Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Problem},
};

pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    kind: ColoringKind,
    formula: Formula<S>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // color -> original input -> possible output memberships.
    memberships: Vec<Vec<Vec<Choice>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    pub fn new(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
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

    fn extend(&mut self, colors: u32) -> anyhow::Result<()> {
        let labels: BTreeSet<_> = self.problem.output().active().labels().collect();
        while self.memberships.len() < colors as usize {
            self.formula.check()?;
            let enabled = self.formula.new_lit();
            self.enabled.push(enabled);
            let mut behavior = Vec::new();
            for &input in &self.inputs {
                let mut outputs = Vec::new();
                for &label in &labels {
                    self.formula.check()?;
                    if self.problem.output_allowed(input, label) {
                        let literal = self.formula.new_lit();
                        self.formula.add_clause(&[!literal, enabled])?;
                        outputs.push(Choice { label, literal });
                    }
                }
                behavior.push(outputs);
            }
            self.memberships.push(behavior);
        }
        Ok(())
    }

    fn witness(&self, colors: u32) -> anyhow::Result<ColoringWitness> {
        let mut behaviors = Vec::new();
        for behavior in &self.memberships[..colors as usize] {
            let mut outputs_by_input = Vec::new();
            for choices in behavior {
                self.formula.check()?;
                let mut outputs = Vec::new();
                for choice in choices {
                    if self.formula.solver.lit_val(choice.literal)? == TernaryVal::True {
                        outputs.push(choice.label);
                    }
                }
                outputs_by_input.push(LabelSet::new(outputs));
            }
            behaviors.push(ColorBehavior { outputs_by_input });
        }
        Ok(ColoringWitness { behaviors })
    }

    pub fn solve(
        &mut self,
        colors: u32,
        mode: PassiveMode,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Option<ColoringWitness>> {
        computation.report(Phase::Encoding, self.iterations)?;
        self.extend(colors)?;
        // Smaller queries disable extra colors, including their memberships.
        // Learned constraints remain valid for every subsequent palette size.
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
        loop {
            computation.report(Phase::Solving, self.iterations)?;
            match self.formula.solver.solve_assumps(&assumptions)? {
                SolverResult::Unsat => return Ok(None),
                SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
                SolverResult::Sat => {}
            }
            self.iterations = self.iterations.saturating_add(1);
            computation.report(Phase::Verifying, self.iterations)?;
            let witness = self.witness(colors)?;
            let mut failed = None;
            for (context, input) in self.active_inputs.iter().enumerate() {
                let inputs = port_inputs(input);
                if !prepare::active_colors(
                    &inputs,
                    self.kind,
                    colors,
                    &computation.cancellation,
                    &mut |palette| {
                        let positions: Vec<_> = inputs
                            .iter()
                            .zip(palette)
                            .map(|(&input, &color)| {
                                witness.behaviors[color as usize].outputs_by_input
                                    [input_index(input)]
                                .clone()
                            })
                            .collect();
                        if !allows_positions_with(
                            self.problem.output().active(),
                            &positions,
                            &computation.cancellation,
                        )? {
                            failed = Some((context, palette.to_vec()));
                            return Ok(false);
                        }
                        Ok(true)
                    },
                )? {
                    break;
                }
            }
            if let Some((context, palette)) = failed {
                if !self.encoded.insert((context, palette.clone())) {
                    anyhow::bail!("Active matching encoding disagrees with coloring verification.");
                }
                let input = &self.active_inputs[context];
                let inputs = port_inputs(input);
                let positions: Vec<_> = inputs
                    .iter()
                    .zip(&palette)
                    .map(|(&input, &color)| {
                        self.memberships[color as usize][input_index(input)].clone()
                    })
                    .collect();
                let enabled: BTreeSet<_> = palette
                    .iter()
                    .map(|&color| self.enabled[color as usize])
                    .collect();
                computation.report(Phase::Encoding, self.iterations)?;
                encode_constraint(
                    &mut self.formula,
                    self.problem.output().active(),
                    input.degree,
                    &positions,
                    &enabled.into_iter().collect::<Vec<_>>(),
                )?;
                continue;
            }
            let mut emissions = Vec::new();
            for (color, behavior) in self.memberships[..colors as usize].iter().enumerate() {
                for (&input, choices) in self.inputs.iter().zip(behavior) {
                    for choice in choices {
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
            let mut forbidden = None;
            prepare::passive_families(
                &self.passive_inputs,
                self.kind,
                colors,
                &computation.cancellation,
                &mut |family| {
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
            if let Some(sources) = forbidden {
                self.formula.add_clause(
                    &sources
                        .into_iter()
                        .map(|source| !source)
                        .collect::<Vec<_>>(),
                )?;
                continue;
            }
            return Ok(Some(witness));
        }
    }
}
