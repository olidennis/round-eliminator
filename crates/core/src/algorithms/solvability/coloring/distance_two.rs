//! Distance-two coloring on ordinary graphs, with neighboring colors visible.
//!
//! Memberships are indexed by (own color, neighbor color, original input).
//! Active matching still uses the complete local input/color configuration.
//! Across an edge the two color indices reverse. On trees, endpoint contexts
//! with the same edge colors can be combined: other neighbors on opposite sides
//! are distance three apart. Thus collecting possible outputs by this key is
//! exact for the local/tree semantics, without constructing a colored problem.

use std::collections::BTreeSet;

use rustsat::{
    solvers::{SolveIncremental, SolverResult},
    types::{Lit, TernaryVal},
};

use super::{
    ColorBehavior, ColoringKind, ColoringWitness, NeighborColorBehavior,
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
    computation::{Cancellation, Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Part, Problem},
};

// An incidence knows both endpoint colors as well as its original input label.
type ColoredInput = (Option<LabelId>, u32, u32);

pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    formula: Formula<S>,
    // own color -> neighbor color -> original input -> output choices.
    memberships: Vec<Vec<Vec<Vec<Choice>>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, u32, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    pub(super) fn new(
        problem: &'a Problem,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        computation.report(Phase::Enumerating, 0)?;
        Ok(Self {
            problem,
            inputs: prepare::input_labels(problem),
            active_inputs: active_inputs(problem, &computation.cancellation)?,
            passive_inputs: prepare_passive_inputs(problem, &computation.cancellation)?,
            formula: Formula::new(solver, computation.cancellation.clone()),
            memberships: Vec::new(),
            enabled: Vec::new(),
            encoded: BTreeSet::new(),
            iterations: 0,
        })
    }

    fn extend(&mut self, colors: u32) -> anyhow::Result<()> {
        while self.enabled.len() < colors as usize {
            self.formula.check()?;
            self.enabled.push(self.formula.new_lit());
        }
        let labels: BTreeSet<_> = self.problem.output().active().labels().collect();
        for own in 0..colors as usize {
            if own == self.memberships.len() {
                self.memberships.push(Vec::new());
            }
            for neighbor in self.memberships[own].len()..colors as usize {
                let mut by_input = Vec::new();
                for &input in &self.inputs {
                    self.formula.check()?;
                    let mut choices = Vec::new();
                    if own != neighbor {
                        for &label in &labels {
                            self.formula.check()?;
                            if self.problem.output_allowed(input, label) {
                                let literal = self.formula.new_lit();
                                // A smaller query disables every membership
                                // mentioning a color outside its palette.
                                self.formula.add_clause(&[!literal, self.enabled[own]])?;
                                self.formula
                                    .add_clause(&[!literal, self.enabled[neighbor]])?;
                                choices.push(Choice { label, literal });
                            }
                        }
                    }
                    by_input.push(choices);
                }
                self.memberships[own].push(by_input);
            }
        }
        Ok(())
    }

    fn witness(&self, colors: u32) -> anyhow::Result<ColoringWitness> {
        let mut behaviors = Vec::new();
        for neighbors in &self.memberships[..colors as usize] {
            let mut outputs_by_neighbor = Vec::new();
            for inputs in &neighbors[..colors as usize] {
                let mut outputs_by_input = Vec::new();
                for choices in inputs {
                    let mut outputs = Vec::new();
                    for choice in choices {
                        self.formula.check()?;
                        if self.formula.solver.lit_val(choice.literal)? == TernaryVal::True {
                            outputs.push(choice.label);
                        }
                    }
                    outputs_by_input.push(LabelSet::new(outputs));
                }
                outputs_by_neighbor.push(ColorBehavior { outputs_by_input });
            }
            behaviors.push(NeighborColorBehavior {
                outputs_by_neighbor,
            });
        }
        Ok(ColoringWitness::NeighborColors { behaviors })
    }

    pub(super) fn solve(
        &mut self,
        colors: u32,
        mode: PassiveMode,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Option<ColoringWitness>> {
        computation.report(Phase::Encoding, self.iterations)?;
        self.extend(colors)?;
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
            let ColoringWitness::NeighborColors { behaviors } = &witness else {
                unreachable!("neighbor-color search produces neighbor-color witnesses");
            };
            let mut failed = None;
            for (context, input) in self.active_inputs.iter().enumerate() {
                let inputs = port_inputs(input);
                for own in 0..colors {
                    let valid = prepare::distinct_colors_except(
                        &inputs,
                        colors,
                        Some(own),
                        &computation.cancellation,
                        &mut |neighbors| {
                            let positions: Vec<_> = inputs
                                .iter()
                                .zip(neighbors)
                                .map(|(&input, &neighbor)| {
                                    behaviors[own as usize].outputs_by_neighbor[neighbor as usize]
                                        .outputs_by_input[input_index(input)]
                                    .clone()
                                })
                                .collect();
                            if !allows_positions_with(
                                self.problem.output().active(),
                                &positions,
                                &computation.cancellation,
                            )? {
                                failed = Some((context, own, neighbors.to_vec()));
                                return Ok(false);
                            }
                            Ok(true)
                        },
                    )?;
                    if !valid {
                        break;
                    }
                }
                if failed.is_some() {
                    break;
                }
            }
            if let Some((context, own, neighbors)) = failed {
                if !self.encoded.insert((context, own, neighbors.clone())) {
                    anyhow::bail!(
                        "Active matching encoding disagrees with distance-two verification."
                    );
                }
                let input = &self.active_inputs[context];
                let inputs = port_inputs(input);
                let positions: Vec<_> = inputs
                    .iter()
                    .zip(&neighbors)
                    .map(|(&input, &neighbor)| {
                        self.memberships[own as usize][neighbor as usize][input_index(input)]
                            .clone()
                    })
                    .collect();
                computation.report(Phase::Encoding, self.iterations)?;
                encode_constraint(
                    &mut self.formula,
                    self.problem.output().active(),
                    input.degree,
                    &positions,
                    &std::iter::once(own)
                        .chain(neighbors.iter().copied())
                        .map(|color| self.enabled[color as usize])
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>(),
                )?;
                continue;
            }
            let mut emissions = Vec::new();
            for (own, behavior) in behaviors.iter().enumerate() {
                for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
                    for (&input, labels) in self.inputs.iter().zip(&outputs.outputs_by_input) {
                        for choice in &self.memberships[own][neighbor][input_index(input)] {
                            self.formula.check()?;
                            if labels.contains(choice.label) {
                                emissions.push(Emission {
                                    input: (input, own as u32, neighbor as u32),
                                    output: choice.label,
                                    source: choice.literal,
                                });
                            }
                        }
                    }
                }
            }
            let mut forbidden = None;
            passive_families(
                &self.passive_inputs,
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

/// Keep original passive input alternatives condensed. Color indices at the
/// second endpoint reverse, so only mutually consistent endpoint views combine.
fn passive_families(
    inputs: &[CondensedConfiguration<Option<LabelId>>],
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&CondensedConfiguration<ColoredInput>) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    for input in inputs {
        cancellation.check()?;
        let slots: Vec<_> = input
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        debug_assert_eq!(slots.len(), 2);
        if !prepare::distinct_colors(&slots, colors, cancellation, &mut |palette| {
            let family = CondensedConfiguration::new(
                slots
                    .iter()
                    .enumerate()
                    .map(|(port, labels)| {
                        Part::new(
                            LabelSet::new(
                                labels
                                    .iter()
                                    .map(|input| (input, palette[port], palette[1 - port]))
                                    .collect(),
                            ),
                            1,
                        )
                        .expect("nonempty inputs")
                    })
                    .collect(),
            )
            .expect("two incidences preserve degree");
            visitor(&family)
        })? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Independent witness checks use counted matching and passive verification,
/// without SAT or the search's encoded-context cache.
pub(super) fn verify_with(
    problem: &Problem,
    witness: &ColoringWitness,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    let ColoringWitness::NeighborColors { behaviors } = witness else {
        return Ok(false);
    };
    let colors = u32::try_from(behaviors.len())?;
    if colors < prepare::minimum(problem, ColoringKind::DistanceTwo)? {
        return Ok(false);
    }
    let inputs = prepare::input_labels(problem);
    for (own, behavior) in behaviors.iter().enumerate() {
        if behavior.outputs_by_neighbor.len() != behaviors.len() {
            return Ok(false);
        }
        for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
            if outputs.outputs_by_input.len() != inputs.len() {
                return Ok(false);
            }
            for (&input, labels) in inputs.iter().zip(&outputs.outputs_by_input) {
                if own == neighbor && !labels.is_empty() {
                    return Ok(false);
                }
                for output in labels.iter() {
                    cancellation.check()?;
                    if output.0 as usize >= problem.output().labels().len()
                        || !problem.output_allowed(input, output)
                    {
                        return Ok(false);
                    }
                }
            }
        }
    }
    for input in active_inputs(problem, cancellation)? {
        let inputs = port_inputs(&input);
        for own in 0..colors {
            if !prepare::distinct_colors_except(
                &inputs,
                colors,
                Some(own),
                cancellation,
                &mut |neighbors| {
                    let positions: Vec<_> = inputs
                        .iter()
                        .zip(neighbors)
                        .map(|(&input, &neighbor)| {
                            behaviors[own as usize].outputs_by_neighbor[neighbor as usize]
                                .outputs_by_input[input_index(input)]
                            .clone()
                        })
                        .collect();
                    Ok(allows_positions_with(
                        problem.output().active(),
                        &positions,
                        cancellation,
                    )?)
                },
            )? {
                return Ok(false);
            }
        }
    }
    let mut emissions = Vec::new();
    for (own, behavior) in behaviors.iter().enumerate() {
        for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
            for (&input, labels) in inputs.iter().zip(&outputs.outputs_by_input) {
                cancellation.check()?;
                emissions.extend(labels.iter().map(|output| Emission {
                    input: (input, own as u32, neighbor as u32),
                    output,
                    source: (),
                }));
            }
        }
    }
    passive_families(
        &prepare_passive_inputs(problem, cancellation)?,
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
