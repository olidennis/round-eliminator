//! Distance-two coloring on ordinary graphs, with neighboring colors visible.
//!
//! Memberships are indexed by (own color, neighbor color, original input).
//! Active matching still uses the complete local input/color configuration.
//! Across an edge the two color indices reverse. On trees, endpoint contexts
//! with the same edge colors can be combined: other neighbors on opposite sides
//! are distance three apart. Thus collecting possible outputs by this key is
//! exact for the local/tree semantics, without constructing a colored problem.

use std::collections::{BTreeMap, BTreeSet};

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
        inputs::{LocalInput, active_inputs, prepare_passive_inputs},
        sat::Formula,
        solvability::{
            constraint_encoding::{Choice, encode_constraint},
            passive::{
                EAGER_ENCODING_BUDGET, Emission, PassiveMode, eager_encoding_work,
                forbidden_combination, forbidden_with_maximum, visit_forbidden,
            },
        },
    },
    computation::{Cancellation, Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Part, Problem},
};

// An incidence knows both endpoint colors as well as its original input label.
type ColoredInput = (Option<LabelId>, u32, u32);

pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // All reverse-endpoint color placements through each cached palette have
    // eager clauses. Rows that exceed the encoding budget use exact refinement.
    eager_palettes: Vec<u32>,
    formula: Formula<S>,
    // own color -> neighbor color -> original input -> output choices.
    memberships: Vec<Vec<Vec<Vec<Choice>>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, u32, Vec<u32>)>,
    iterations: u32,
    symmetry: BTreeMap<u32, Lit>,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    #[cfg(test)]
    pub(super) fn new(
        problem: &'a Problem,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        Self::new_with_maximum(problem, solver, None, computation)
    }

    pub(super) fn new_with_maximum(
        problem: &'a Problem,
        solver: S,
        maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        computation.report(Phase::Enumerating, 0)?;
        let passive_inputs = prepare_passive_inputs(problem, &computation.cancellation)?;
        Ok(Self {
            problem,
            maximum,
            inputs: prepare::input_labels(problem),
            active_inputs: active_inputs(problem, &computation.cancellation)?,
            eager_palettes: vec![0; passive_inputs.len()],
            passive_inputs,
            formula: Formula::new(solver, computation.cancellation.clone()),
            memberships: Vec::new(),
            enabled: Vec::new(),
            encoded: BTreeSet::new(),
            iterations: 0,
            symmetry: BTreeMap::new(),
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

    /// A row's union over neighbor colors is invariant under column renaming.
    /// Every simultaneous row/column permutation orbit has a representative
    /// with these profiles sorted. This is cheaper than full matrix lex leaders.
    /// Per-palette gates preserve incremental growth and shrinkage.
    fn encode_symmetry(&mut self, colors: u32) -> anyhow::Result<()> {
        if self.symmetry.contains_key(&colors) {
            return Ok(());
        }
        let gate = self.formula.new_lit();
        let mut profiles = Vec::new();
        for own in 0..colors as usize {
            let mut sources = BTreeMap::<(usize, LabelId), Vec<Lit>>::new();
            for neighbor in 0..colors as usize {
                for (input, choices) in self.memberships[own][neighbor].iter().enumerate() {
                    for choice in choices {
                        sources
                            .entry((input, choice.label))
                            .or_default()
                            .push(choice.literal);
                    }
                }
            }
            let mut profile = Vec::new();
            for sources in sources.into_values() {
                let literal = self.formula.new_lit();
                for &source in &sources {
                    self.formula.add_clause(&[!source, literal])?;
                }
                let mut clause = vec![!literal];
                clause.extend(sources);
                self.formula.add_clause(&clause)?;
                profile.push(literal);
            }
            profiles.push(profile);
        }
        for pair in profiles.windows(2) {
            self.formula.lexicographic_order(&pair[0], &pair[1], gate)?;
        }
        self.symmetry.insert(colors, gate);
        Ok(())
    }

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
            let work = eager_encoding_work(
                input,
                (colors as usize).saturating_mul(colors.saturating_sub(1) as usize),
                // Every off-diagonal color pair has the same allowed domain.
                |input| self.memberships[0][1][input_index(input)].len(),
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
        for (own, neighbors) in self.memberships[..colors as usize].iter().enumerate() {
            for (neighbor, inputs) in neighbors[..colors as usize].iter().enumerate() {
                for (&input, choices) in self.inputs.iter().zip(inputs) {
                    for choice in choices {
                        self.formula.check()?;
                        emissions.push(Emission {
                            input: (input, own as u32, neighbor as u32),
                            output: choice.label,
                            source: choice.literal,
                        });
                    }
                }
            }
        }
        for index in eager {
            let previous = self.eager_palettes[index];
            passive_families(
                std::slice::from_ref(&self.passive_inputs[index]),
                colors,
                cancellation,
                &mut |family| {
                    if !family.parts().iter().any(|part| {
                        part.labels()
                            .iter()
                            .any(|(_, own, neighbor)| own >= previous || neighbor >= previous)
                    }) {
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
        self.encode_symmetry(colors)?;
        let passive_inputs = self.encode_passive(colors, &computation.cancellation)?;
        let mut assumptions: Vec<_> = self
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
        assumptions.extend(
            self.symmetry
                .iter()
                .map(|(&palette, &gate)| if palette == colors { gate } else { !gate }),
        );
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
                &passive_inputs,
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
            crate::algorithms::sat::backend(&computation.cancellation),
            &mut computation,
        )
        .unwrap();
        for (step, colors) in [14, 3, 14, 3].into_iter().enumerate() {
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
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
}

#[cfg(test)]
mod symmetry_tests {
    use super::*;
    use crate::algorithms::sat::backend;
    #[test]
    fn simultaneous_swap_encoding_matches_all_small_matrices_and_retains_every_orbit() {
        let problem = crate::parse_problem(
            serde_json::from_value(serde_json::json!({"kind":"plain",
            "constraints":{"active":"A", "passive":"A^2"}}))
            .unwrap(),
        )
        .unwrap();
        let cancellation = Cancellation::default();
        let mut report = |_| {};
        let mut computation = Computation::new(cancellation.clone(), &mut report);
        let mut solver =
            PaletteSolver::new(&problem, backend(&cancellation), &mut computation).unwrap();
        solver.extend(4).unwrap();
        solver.encode_symmetry(3).unwrap();
        solver.encode_symmetry(4).unwrap();
        let pairs: Vec<_> = (0..3)
            .flat_map(|a| (0..3).filter(move |&b| a != b).map(move |b| (a, b)))
            .collect();
        let bits = |mask: u32, order: &[usize]| {
            pairs
                .iter()
                .map(|&(a, b)| {
                    let index = pairs
                        .iter()
                        .position(|&pair| pair == (order[a], order[b]))
                        .unwrap();
                    mask & (1 << index) != 0
                })
                .collect::<Vec<_>>()
        };
        let leader = |mask| {
            let row_union: Vec<_> = (0..3)
                .map(|own| {
                    pairs
                        .iter()
                        .enumerate()
                        .any(|(i, &(a, _))| a == own && mask & (1 << i) != 0)
                })
                .collect();
            row_union.windows(2).all(|pair| pair[0] <= pair[1])
        };
        for mask in 0..64 {
            let mut assumptions: Vec<_> = solver
                .enabled
                .iter()
                .enumerate()
                .map(|(i, &literal)| if i < 3 { literal } else { !literal })
                .collect();
            assumptions.extend(
                solver
                    .symmetry
                    .iter()
                    .map(|(&n, &literal)| if n == 3 { literal } else { !literal }),
            );
            assumptions.extend(pairs.iter().enumerate().map(|(i, &(a, b))| {
                let literal = solver.memberships[a][b][0][0].literal;
                if mask & (1 << i) != 0 {
                    literal
                } else {
                    !literal
                }
            }));
            assert_eq!(
                solver.formula.solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                leader(mask)
            );
            let permutations = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ];
            assert!(permutations.iter().any(|permutation| {
                let permuted = bits(mask, permutation)
                    .iter()
                    .enumerate()
                    .fold(0u32, |mask, (i, &bit)| mask | ((bit as u32) << i));
                leader(permuted)
            }));
        }
    }
}
