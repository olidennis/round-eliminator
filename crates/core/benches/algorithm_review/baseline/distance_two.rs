//! LLM: Distance-two coloring on ordinary graphs, with neighboring colors visible.
//! LLM: Memberships are indexed by (own color, neighbor color, original input).
//! LLM: Active matching still uses the complete local input/color configuration.
//! LLM: Across an edge the two color indices reverse. On trees, endpoint contexts
//! LLM: with the same edge colors can be combined: other neighbors on opposite sides
//! LLM: are distance three apart. Thus collecting possible outputs by this key is
//! LLM: exact for the local/tree semantics, without constructing a colored problem.

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

// LLM: An incidence knows both endpoint colors as well as its original input label.
/// LLM: An original input descriptor augmented with the auxiliary color information needed by
/// LLM: this coloring encoding. The tuple shape distinguishes local-color from
/// LLM: own/neighbor-color incidences.
type ColoredInput = (Option<LabelId>, u32, u32);

/// LLM: The incremental distance-two SAT state over `(own color, neighbor color, original
/// LLM: input)` memberships. It retains activation gates, encoded neighborhoods, condensed
/// LLM: passive inputs, and learned clauses across palette queries. Diagonal color entries are
/// LLM: unused and passive endpoints reverse color indices.
pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // LLM: All reverse-endpoint color placements through each cached palette have
    // LLM: eager clauses. Rows that exceed the encoding budget use exact refinement.

    eager_palettes: Vec<u32>,
    formula: Formula<S>,
    // LLM: own color -> neighbor color -> original input -> output choices.

    memberships: Vec<Vec<Vec<Vec<Choice>>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, u32, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    /// LLM: Regression test: verify that new. Used by the test suite to protect this behavior
    /// LLM: while implementations change.
    #[cfg(test)]
    pub(super) fn new(
        problem: &'a Problem,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Self::new_with_maximum(problem, solver, None, computation)
    }

    /// LLM: Prepare original input contexts, membership state, and an optional compatible
    /// LLM: passive maximum. Used once per coloring request before palette queries share
    /// LLM: clauses.
    pub(super) fn new_with_maximum(
        problem: &'a Problem,
        solver: S,
        maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Enumerating, 0)?;
        let passive_inputs = prepare_passive_inputs(problem, &computation.cancellation)?;
        // LLM: Prepare one canonical rule obligation for each promised active input context.
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
        })
    }

    /// LLM: Allocate missing color memberships and their activation gates, retaining all
    /// LLM: previous variables and clauses. Used before querying a larger palette.
    fn extend(&mut self, colors: u32) -> anyhow::Result<()> {
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        while self.enabled.len() < colors as usize {
            self.formula.check()?;
            self.enabled.push(self.formula.new_lit());
        }
        let labels: BTreeSet<_> = self.problem.output().active().labels().collect();
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        for own in 0..colors as usize {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if own == self.memberships.len() {
                self.memberships.push(Vec::new());
            }
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            for neighbor in self.memberships[own].len()..colors as usize {
                // LLM: Prepare `by_input` for the following computation; it stores the by
                // LLM: input for this stage.
                let mut by_input = Vec::new();
                for &input in &self.inputs {
                    // LLM: Use the stored state for this operation without exposing mutable
                    // LLM: representation details.
                    self.formula.check()?;
                    let mut choices = Vec::new();
                    // LLM: Add the necessary SAT restriction; a refinement clause forbids the
                    // LLM: selected violating combination.
                    if own != neighbor {
                        // LLM: Add the necessary SAT restriction; a refinement clause forbids
                        // LLM: the selected violating combination.
                        for &label in &labels {
                            // LLM: Use the stored state for this operation without exposing
                            // LLM: mutable representation details.
                            self.formula.check()?;
                            if self.problem.output_allowed(input, label) {
                                // LLM: Prepare `literal` for the following computation; it
                                // LLM: stores the literal for this stage.
                                let literal = self.formula.new_lit();
                                // LLM: A smaller query disables every membership
                                // LLM: mentioning a color outside its palette.

                                self.formula.add_clause(&[!literal, self.enabled[own]])?;
                                self.formula
                                    .add_clause(&[!literal, self.enabled[neighbor]])?;
                                choices.push(Choice { label, literal });
                            }
                        }
                    }
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    by_input.push(choices);
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                self.memberships[own].push(by_input);
            }
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(())
    }

    /// LLM: Encode affordable low-degree passive rows eagerly and return rows still needing
    /// LLM: exact refinement. Used for each palette; previous eager coverage avoids encoding
    /// LLM: the same color placements twice.
    fn encode_passive(
        &mut self,
        colors: u32,
        cancellation: &Cancellation,
    ) -> anyhow::Result<Vec<CondensedConfiguration<Option<LabelId>>>> {
        // LLM: Prepare `budget` for the following computation; it stores the budget for this
        // LLM: stage.
        let mut budget = EAGER_ENCODING_BUDGET;
        let mut eager = Vec::new();
        let mut lazy = Vec::new();
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for (index, input) in self.passive_inputs.iter().enumerate() {
            // LLM: Use the stored state for this operation without exposing mutable
            // LLM: representation details.
            self.formula.check()?;
            if self.eager_palettes[index] >= colors {
                continue;
            }
            // LLM: Estimate computation cost to choose scheduling, without restricting the
            // LLM: exact search.
            let work = eager_encoding_work(
                input,
                (colors as usize).saturating_mul(colors.saturating_sub(1) as usize),
                // LLM: Every off-diagonal color pair has the same allowed domain.

                |input| self.memberships[0][1][input_index(input)].len(),
                cancellation,
            )?;
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if work <= budget {
                budget -= work;
                eager.push(index);
            } else {
                lazy.push(input.clone());
            }
        }
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if eager.is_empty() {
            return Ok(lazy);
        }
        let mut emissions = Vec::new();
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for (own, neighbors) in self.memberships[..colors as usize].iter().enumerate() {
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for (neighbor, inputs) in neighbors[..colors as usize].iter().enumerate() {
                // LLM: Process the relevant label choices while preserving their table
                // LLM: identities.
                for (&input, choices) in self.inputs.iter().zip(inputs) {
                    // LLM: Process the relevant label choices while preserving their table
                    // LLM: identities.
                    for choice in choices {
                        // LLM: Use the stored state for this operation without exposing
                        // LLM: mutable representation details.
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
        // LLM: Encode every forbidden low-degree source combination needed by future SAT
        // LLM: candidates.
        for index in eager {
            // LLM: Record prior work so the next pass need only visit newly relevant parent
            // LLM: pairs.
            let previous = self.eager_palettes[index];
            passive_families(
                std::slice::from_ref(&self.passive_inputs[index]),
                colors,
                cancellation,
                &mut |family| {
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if !family.parts().iter().any(|part| {
                        part.labels()
                            .iter()
                            .any(|(_, own, neighbor)| own >= previous || neighbor >= previous)
                    }) {
                        return Ok(true);
                    }
                    // LLM: Encode every forbidden low-degree source combination needed by
                    // LLM: future SAT candidates.
                    visit_forbidden(
                        self.problem.output().passive(),
                        family,
                        &emissions,
                        cancellation,
                        |sources| {
                            // LLM: Add the necessary SAT restriction; a refinement clause
                            // LLM: forbids the selected violating combination.
                            self.formula.add_clause(
                                &sources
                                    .into_iter()
                                    .map(|source| !source)
                                    .collect::<Vec<_>>(),
                            )
                        },
                    )?;
                    // LLM: Return the result of this completed search or conversion to its
                    // LLM: caller.
                    Ok(true)
                },
            )?;
            // LLM: Use the stored state for this operation without exposing mutable
            // LLM: representation details.
            self.eager_palettes[index] = colors;
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(lazy)
    }

    /// LLM: Read true output memberships into the appropriate color/input behavior tables.
    /// LLM: Used after SAT to verify or return a strategy certificate.
    fn witness(&self, colors: u32) -> anyhow::Result<ColoringWitness> {
        // LLM: Prepare `behaviors` for the following computation; it stores the behaviors for
        // LLM: this stage.
        let mut behaviors = Vec::new();
        for neighbors in &self.memberships[..colors as usize] {
            // LLM: Prepare `outputs_by_neighbor` for the following computation; it stores the
            // LLM: outputs by neighbor for this stage.
            let mut outputs_by_neighbor = Vec::new();
            for inputs in &neighbors[..colors as usize] {
                // LLM: Prepare `outputs_by_input` for the following computation; it stores the
                // LLM: outputs by input for this stage.
                let mut outputs_by_input = Vec::new();
                for choices in inputs {
                    // LLM: Prepare `outputs` for the following computation; it stores the
                    // LLM: outputs for this stage.
                    let mut outputs = Vec::new();
                    for choice in choices {
                        self.formula.check()?;
                        if self.formula.solver.lit_val(choice.literal)? == TernaryVal::True {
                            outputs.push(choice.label);
                        }
                    }
                    outputs_by_input.push(LabelSet::new(outputs));
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                outputs_by_neighbor.push(ColorBehavior { outputs_by_input });
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            behaviors.push(NeighborColorBehavior {
                outputs_by_neighbor,
            });
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(ColoringWitness::NeighborColors { behaviors })
    }

    /// LLM: Query one palette, checking candidate active/passive validity and refining
    /// LLM: violated obligations until a witness or UNSAT. Used by fixed decisions and
    /// LLM: consecutive maximum search; interruption remains a separate error.
    pub(super) fn solve(
        &mut self,
        colors: u32,
        mode: PassiveMode,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Option<ColoringWitness>> {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Encoding, self.iterations)?;
        self.extend(colors)?;
        let passive_inputs = self.encode_passive(colors, &computation.cancellation)?;
        // LLM: Enable exactly the requested palette and disable memberships belonging to
        // LLM: retired colors.
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
            let ColoringWitness::NeighborColors { behaviors } = &witness else {
                unreachable!("neighbor-color search produces neighbor-color witnesses");
            };
            let mut failed = None;
            // LLM: Check whether these local output domains can jointly realize some valid
            // LLM: active configuration.
            for (context, input) in self.active_inputs.iter().enumerate() {
                // LLM: Prepare `inputs` for the following computation; it stores the inputs
                // LLM: for this stage.
                let inputs = port_inputs(input);
                for own in 0..colors {
                    // LLM: Check whether these local output domains can jointly realize some
                    // LLM: valid active configuration.
                    let valid = prepare::distinct_colors_except(
                        &inputs,
                        colors,
                        Some(own),
                        &computation.cancellation,
                        &mut |neighbors| {
                            // LLM: Associate each occurrence with its available outputs before
                            // LLM: checking a whole configuration.
                            let positions: Vec<_> = inputs
                                .iter()
                                .zip(neighbors)
                                .map(|(&input, &neighbor)| {
                                    behaviors[own as usize].outputs_by_neighbor[neighbor as usize]
                                        .outputs_by_input[input_index(input)]
                                    .clone()
                                })
                                .collect();
                            // LLM: Check whether these local output domains can jointly
                            // LLM: realize some valid active configuration.
                            if !allows_positions_with(
                                self.problem.output().active(),
                                &positions,
                                &computation.cancellation,
                            )? {
                                failed = Some((context, own, neighbors.to_vec()));
                                return Ok(false);
                            }
                            // LLM: Return the result of this completed search or conversion to
                            // LLM: its caller.
                            Ok(true)
                        },
                    )?;
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if !valid {
                        break;
                    }
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if failed.is_some() {
                    break;
                }
            }
            // LLM: Require a whole valid active output by matching available per-port choices
            // LLM: to condensed part capacities.
            if let Some((context, own, neighbors)) = failed {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if !self.encoded.insert((context, own, neighbors.clone())) {
                    anyhow::bail!(
                        "Active matching encoding disagrees with distance-two verification."
                    );
                }
                let input = &self.active_inputs[context];
                let inputs = port_inputs(input);
                // LLM: Associate each occurrence with its available outputs before checking a
                // LLM: whole configuration.
                let positions: Vec<_> = inputs
                    .iter()
                    .zip(&neighbors)
                    .map(|(&input, &neighbor)| {
                        self.memberships[own as usize][neighbor as usize][input_index(input)]
                            .clone()
                    })
                    .collect();
                // LLM: Update the current working state before continuing this operation.
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
                // LLM: Stop this branch or move to its next alternative; the enclosing search
                // LLM: retains the remaining choices.
                continue;
            }
            // LLM: Collect available outputs with their input/color descriptors and source
            // LLM: provenance.
            let mut emissions = Vec::new();
            for (own, behavior) in behaviors.iter().enumerate() {
                // LLM: Process the relevant label choices while preserving their table
                // LLM: identities.
                for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
                    // LLM: Process the relevant label choices while preserving their table
                    // LLM: identities.
                    for (&input, labels) in self.inputs.iter().zip(&outputs.outputs_by_input) {
                        // LLM: Process the relevant label choices while preserving their table
                        // LLM: identities.
                        for choice in &self.memberships[own][neighbor][input_index(input)] {
                            // LLM: Use the stored state for this operation without exposing
                            // LLM: mutable representation details.
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
            // LLM: Preserve the violation or conflict needed to reject an invalid search
            // LLM: branch.
            let mut forbidden = None;
            passive_families(
                &passive_inputs,
                colors,
                &computation.cancellation,
                &mut |family| {
                    // LLM: Verify the candidate's universal passive outputs, reusing only a
                    // LLM: compatible maximum certificate.
                    forbidden = forbidden_with_maximum(
                        self.problem.output().passive(),
                        std::slice::from_ref(family),
                        &emissions,
                        mode,
                        self.maximum,
                        |input| input.0,
                        &computation.cancellation,
                    )?;
                    // LLM: Return the result of this completed search or conversion to its
                    // LLM: caller.
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

/// LLM: Visit condensed passive input families with mutually consistent reversed endpoint
/// LLM: color indices. Used by distance-two refinement and independent witness checks;
/// LLM: original input alternatives remain condensed.
fn passive_families(
    inputs: &[CondensedConfiguration<Option<LabelId>>],
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&CondensedConfiguration<ColoredInput>) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for input in inputs {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let slots: Vec<_> = input
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        debug_assert_eq!(slots.len(), 2);
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if !prepare::distinct_colors(&slots, colors, cancellation, &mut |palette| {
            // LLM: Prepare `family` for the following computation; it stores the family for
            // LLM: this stage.
            let family = CondensedConfiguration::new(
                slots
                    .iter()
                    .enumerate()
                    .map(|(port, labels)| {
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
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
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            visitor(&family)
        })? {
            return Ok(false);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(true)
}

/// LLM: Independently check numerical own/neighbor-color tables, original input permissions,
/// LLM: active matchings, and reversed passive endpoints. Used by witness validation without
/// LLM: consulting encoded SAT obligations.
pub(super) fn verify_with(
    problem: &Problem,
    witness: &ColoringWitness,
    cancellation: &Cancellation,
) -> anyhow::Result<bool> {
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::NeighborColors { behaviors } = witness else {
        return Ok(false);
    };
    let colors = u32::try_from(behaviors.len())?;
    if colors < prepare::minimum(problem, ColoringKind::DistanceTwo)? {
        return Ok(false);
    }
    let inputs = prepare::input_labels(problem);
    // LLM: Process the relevant label choices while preserving their table identities.
    for (own, behavior) in behaviors.iter().enumerate() {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if behavior.outputs_by_neighbor.len() != behaviors.len() {
            return Ok(false);
        }
        // LLM: Process the relevant label choices while preserving their table identities.
        for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if outputs.outputs_by_input.len() != inputs.len() {
                return Ok(false);
            }
            // LLM: Process the relevant label choices while preserving their table identities.
            for (&input, labels) in inputs.iter().zip(&outputs.outputs_by_input) {
                // LLM: Handle the empty case explicitly before the general traversal or
                // LLM: allocation path.
                if own == neighbor && !labels.is_empty() {
                    return Ok(false);
                }
                // LLM: Process the relevant label choices while preserving their table
                // LLM: identities.
                for output in labels.iter() {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
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
    // LLM: Check whether these local output domains can jointly realize some valid active
    // LLM: configuration.
    for input in active_inputs(problem, cancellation)? {
        // LLM: Prepare `inputs` for the following computation; it stores the inputs for this
        // LLM: stage.
        let inputs = port_inputs(&input);
        for own in 0..colors {
            // LLM: Check whether these local output domains can jointly realize some valid
            // LLM: active configuration.
            if !prepare::distinct_colors_except(
                &inputs,
                colors,
                Some(own),
                cancellation,
                &mut |neighbors| {
                    // LLM: Associate each occurrence with its available outputs before
                    // LLM: checking a whole configuration.
                    let positions: Vec<_> = inputs
                        .iter()
                        .zip(neighbors)
                        .map(|(&input, &neighbor)| {
                            behaviors[own as usize].outputs_by_neighbor[neighbor as usize]
                                .outputs_by_input[input_index(input)]
                            .clone()
                        })
                        .collect();
                    // LLM: Check whether these local output domains can jointly realize some
                    // LLM: valid active configuration.
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
    // LLM: Collect available outputs with their input/color descriptors and source provenance.
    let mut emissions = Vec::new();
    for (own, behavior) in behaviors.iter().enumerate() {
        // LLM: Process the relevant label choices while preserving their table identities.
        for (neighbor, outputs) in behavior.outputs_by_neighbor.iter().enumerate() {
            // LLM: Process the relevant label choices while preserving their table identities.
            for (&input, labels) in inputs.iter().zip(&outputs.outputs_by_input) {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                emissions.extend(labels.iter().map(|output| Emission {
                    input: (input, own as u32, neighbor as u32),
                    output,
                    source: (),
                }));
            }
        }
    }
    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    passive_families(
        &prepare_passive_inputs(problem, cancellation)?,
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

#[cfg(test)]
mod tests {
    use super::super::{tests::plain, verify_witness};
    use super::*;

    /// LLM: Regression test: verify that large domains use lazy refinement and reuse smaller
    /// LLM: eager palettes. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn large_domains_use_lazy_refinement_and_reuse_smaller_eager_palettes() {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = plain("ABCDEFGHIJKLMNOPQRSTUVWX^2", "ABCDEFGHIJKLMNOPQRSTUVWX^2");
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        // LLM: Prepare `solver` for the following computation; it stores the solver for this
        // LLM: stage.
        let mut solver = PaletteSolver::new(
            &p,
            crate::algorithms::sat::backend(&computation.cancellation),
            &mut computation,
        )
        .unwrap();
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        for (step, colors) in [14, 3, 14, 3].into_iter().enumerate() {
            // LLM: Retain the newly constructed choices in the result for this stage.
            solver.extend(colors).unwrap();
            let lazy = solver
                .encode_passive(colors, &computation.cancellation)
                .unwrap();
            assert_eq!(lazy.len(), usize::from(step % 2 == 0));
            if step % 2 != 0 {
                assert_eq!(solver.eager_palettes, [colors]);
            }
            // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
            let witness = solver
                .solve(colors, PassiveMode::General, &mut computation)
                .unwrap()
                .expect("all outputs are allowed");
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
}
