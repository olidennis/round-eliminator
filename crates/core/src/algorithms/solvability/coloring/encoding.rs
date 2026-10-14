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
        inputs::{LocalInput, active_inputs, prepare_passive_inputs},
        sat::Formula,
        solvability::{
            constraint_encoding::{Choice, encode_constraint},
            passive::{
                EAGER_ENCODING_BUDGET, Emission, PassiveMode, eager_encoding_work,
                forbidden_with_maximum, visit_forbidden,
            },
        },
    },
    computation::{Cancellation, Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Problem},
};

pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
    kind: ColoringKind,
    formula: Formula<S>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // Per input row: every passive color placement through this palette has
    // been encoded. Larger rows/palettes continue to use exact refinement.
    eager_palettes: Vec<u32>,
    // color -> original input -> possible output memberships.
    memberships: Vec<Vec<Vec<Choice>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    #[cfg(test)]
    pub fn new(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        Self::new_with_maximum(problem, kind, solver, None, computation)
    }

    pub(super) fn new_with_maximum(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        computation.report(Phase::Enumerating, 0)?;
        let passive_inputs = prepare_passive_inputs(problem, &computation.cancellation)?;
        Ok(Self {
            problem,
            maximum,
            kind,
            formula: Formula::new(solver, computation.cancellation.clone()),
            inputs: prepare::input_labels(problem),
            active_inputs: active_inputs(problem, &computation.cancellation)?,
            eager_palettes: vec![0; passive_inputs.len()],
            passive_inputs,
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
            if let Some(previous) = self.memberships.last() {
                // Auxiliary color names carry no information. Every permutation
                // orbit has a representative with its complete behaviors sorted.
                // Include ALL original inputs; allow equal behaviors. Gate by
                // the successor so shrinking a palette leaves no extra ordering.
                let left: Vec<_> = previous
                    .iter()
                    .flatten()
                    .map(|choice| choice.literal)
                    .collect();
                let right: Vec<_> = behavior
                    .iter()
                    .flatten()
                    .map(|choice| choice.literal)
                    .collect();
                self.formula.lexicographic_order(&left, &right, enabled)?;
            }
            self.memberships.push(behavior);
        }
        Ok(())
    }

    /// Encode affordable low-degree rows, returning the rows still requiring
    /// refinement. Cached clauses remain valid when colors are disabled.
    fn encode_passive(
        &mut self,
        colors: u32,
        cancellation: &Cancellation,
    ) -> anyhow::Result<Vec<CondensedConfiguration<Option<LabelId>>>> {
        let mut budget = EAGER_ENCODING_BUDGET;
        let mut eager = Vec::new();
        let mut lazy = Vec::new();
        for (index, input) in self.passive_inputs.iter().enumerate() {
            self.formula.check()?;
            if self.eager_palettes[index] >= colors {
                continue;
            }
            // Each input's output domain is identical for every color. Count
            // all origin tuples over an upper bound on color placements.
            let placements = if self.kind == ColoringKind::GraphEdges {
                colors as usize
            } else {
                (colors as usize).saturating_pow(input.degree().min(2))
            };
            let work = eager_encoding_work(
                input,
                placements,
                |input| self.memberships[0][input_index(input)].len(),
                cancellation,
            )?;
            if work <= budget {
                budget -= work;
                eager.push(index);
            } else {
                lazy.push(input.clone());
            }
        }
        if eager.is_empty() {
            return Ok(lazy);
        }
        let mut emissions = Vec::new();
        for (color, behavior) in self.memberships[..colors as usize].iter().enumerate() {
            for (&input, choices) in self.inputs.iter().zip(behavior) {
                for choice in choices {
                    self.formula.check()?;
                    emissions.push(Emission {
                        input: (input, color as u32),
                        output: choice.label,
                        source: choice.literal,
                    });
                }
            }
        }
        for index in eager {
            let previous = self.eager_palettes[index];
            prepare::passive_families(
                std::slice::from_ref(&self.passive_inputs[index]),
                self.kind,
                colors,
                cancellation,
                &mut |family| {
                    if previous != 0
                        && !family
                            .parts()
                            .iter()
                            .any(|part| part.labels().iter().any(|(_, color)| color >= previous))
                    {
                        return Ok(true);
                    }
                    visit_forbidden(
                        self.problem.output().passive(),
                        family,
                        &emissions,
                        cancellation,
                        |sources| {
                            self.formula.add_clause(
                                &sources
                                    .into_iter()
                                    .map(|source| !source)
                                    .collect::<Vec<_>>(),
                            )
                        },
                    )?;
                    Ok(true)
                },
            )?;
            self.eager_palettes[index] = colors;
        }
        Ok(lazy)
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
        Ok(ColoringWitness::LocalColors { behaviors })
    }

    pub fn solve(
        &mut self,
        colors: u32,
        mode: PassiveMode,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Option<ColoringWitness>> {
        computation.report(Phase::Encoding, self.iterations)?;
        self.extend(colors)?;
        let passive_inputs = self.encode_passive(colors, &computation.cancellation)?;
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
            let ColoringWitness::LocalColors { behaviors } = &witness else {
                unreachable!("local-color search produces local-color witnesses");
            };
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
                                behaviors[color as usize].outputs_by_input[input_index(input)]
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
                &passive_inputs,
                self.kind,
                colors,
                &computation.cancellation,
                &mut |family| {
                    forbidden = forbidden_with_maximum(
                        self.problem.output().passive(),
                        std::slice::from_ref(family),
                        &emissions,
                        mode,
                        self.maximum,
                        |input| input.0,
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

#[cfg(test)]
mod tests {
    use super::super::{tests::plain, verify_witness};
    use super::*;

    #[test]
    fn large_domains_use_lazy_refinement_and_reuse_smaller_eager_palettes() {
        let p = plain("ABCDEFGHIJKLMNOPQRSTUVWX^2", "ABCDEFGHIJKLMNOPQRSTUVWX^2");
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        let mut solver = PaletteSolver::new(
            &p,
            ColoringKind::Vertex,
            crate::algorithms::sat::backend(&computation.cancellation),
            &mut computation,
        )
        .unwrap();
        for (step, colors) in [24, 2, 24, 2].into_iter().enumerate() {
            solver.extend(colors).unwrap();
            let lazy = solver
                .encode_passive(colors, &computation.cancellation)
                .unwrap();
            assert_eq!(lazy.len(), usize::from(step % 2 == 0));
            if step % 2 != 0 {
                assert_eq!(solver.eager_palettes, [colors]);
            }
            let witness = solver
                .solve(colors, PassiveMode::General, &mut computation)
                .unwrap()
                .expect("all outputs are allowed");
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}
