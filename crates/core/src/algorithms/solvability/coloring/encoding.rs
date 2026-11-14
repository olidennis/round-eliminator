//! LLM: Incremental SAT over color/input output-set memberships.

use rustsat::{
    solvers::{SolveIncremental, SolverResult},
    types::{Lit, TernaryVal},
};
use std::collections::{BTreeMap, BTreeSet};

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
            constraint_encoding::{Choice, encode_constraint, encode_constraint_supports},
            passive::{
                EAGER_ENCODING_BUDGET, Emission, PassiveMode, eager_encoding_work,
                forbidden_with_maximum, visit_forbidden,
            },
        },
    },
    computation::{Cancellation, Computation, Phase},
    model::{CondensedConfiguration, LabelId, LabelSet, Problem},
};

/// LLM: The incremental membership SAT state for one problem and coloring kind. It retains
/// LLM: output-membership variables, activation gates, encoded active obligations, passive
/// LLM: input rows, and refinement clauses across palette queries.
pub(super) struct PaletteSolver<'a, S> {
    problem: &'a Problem,
    maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
    kind: ColoringKind,
    formula: Formula<S>,
    inputs: Vec<Option<LabelId>>,
    active_inputs: Vec<LocalInput>,
    // LLM: Safe per-input support bounds for vertex coloring. A color needs only
    // LLM: the outputs used by one realization of every active input context.
    membership_bounds: Option<Vec<usize>>,
    passive_inputs: Vec<CondensedConfiguration<Option<LabelId>>>,
    // LLM: Per input row: every passive color placement through this palette has
    // LLM: been encoded. Larger rows/palettes continue to use exact refinement.
    eager_palettes: Vec<u32>,
    // LLM: color -> original input -> possible output memberships.
    memberships: Vec<Vec<Vec<Choice>>>,
    enabled: Vec<Lit>,
    encoded: BTreeSet<(usize, Vec<u32>)>,
    iterations: u32,
}

impl<'a, S: SolveIncremental> PaletteSolver<'a, S> {
    /// LLM: Regression test: verify that new. Used by the test suite to protect this behavior
    /// LLM: while implementations change.
    #[cfg(test)]
    pub fn new(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Self::new_with_maximum(problem, kind, solver, None, computation)
    }

    /// LLM: Prepare original input contexts, membership state, and an optional compatible
    /// LLM: passive maximum. Used once per coloring request before palette queries share
    /// LLM: clauses.
    pub(super) fn new_with_maximum(
        problem: &'a Problem,
        kind: ColoringKind,
        solver: S,
        maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
        computation: &mut Computation<'_>,
    ) -> anyhow::Result<Self> {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Enumerating, 0)?;
        let passive_inputs = prepare_passive_inputs(problem, &computation.cancellation)?;
        let inputs = prepare::input_labels(problem);
        let active_inputs = active_inputs(problem, &computation.cancellation)?;
        let membership_bounds = (kind == ColoringKind::Vertex)
            .then(|| {
                prepare::vertex_membership_bounds(
                    &active_inputs,
                    inputs.len(),
                    &computation.cancellation,
                )
            })
            .transpose()?;
        // LLM: Prepare one canonical rule obligation for each promised active input context.
        Ok(Self {
            problem,
            maximum,
            kind,
            formula: Formula::new(solver, computation.cancellation.clone()),
            inputs,
            active_inputs,
            membership_bounds,
            eager_palettes: vec![0; passive_inputs.len()],
            passive_inputs,
            memberships: Vec::new(),
            enabled: Vec::new(),
            encoded: BTreeSet::new(),
            iterations: 0,
        })
    }

    /// LLM: Allocate missing color memberships and their activation gates, retaining all
    /// LLM: previous variables and clauses. Used before querying a larger palette.
    fn extend(&mut self, colors: u32) -> anyhow::Result<()> {
        // LLM: Use active output labels to allocate each newly added color's per-input
        // LLM: membership variables.
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
                // LLM: Restrict vertex strategies to supports sufficient for all original
                // LLM: active input contexts. Disabled colors already have empty supports.
                if let Some(bounds) = &self.membership_bounds {
                    self.formula.at_most(
                        &outputs
                            .iter()
                            .map(|choice| choice.literal)
                            .collect::<Vec<_>>(),
                        bounds[input_index(input)],
                    )?;
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                behavior.push(outputs);
            }
            // LLM: Vertex colors share a single color across every active port.
            // LLM: Tie memberships to chosen active realizations before ordering behaviors.
            if self.kind == ColoringKind::Vertex {
                self.encode_vertex_supports(&behavior, enabled)?;
            }
            // LLM: Order interchangeable complete color behaviors without excluding equal
            // LLM: behavior tables.
            if let Some(previous) = self.memberships.last() {
                // LLM: Auxiliary color names carry no information. Every permutation
                // LLM: orbit has a representative with its complete behaviors sorted.
                // LLM: Include ALL original inputs; allow equal behaviors. Gate by
                // LLM: the successor so shrinking a palette leaves no extra ordering.

                let left: Vec<_> = previous
                    .iter()
                    .flatten()
                    .map(|choice| choice.literal)
                    .collect();
                // LLM: Prepare `right` for the following computation; it stores the right for
                // LLM: this stage.
                let right: Vec<_> = behavior
                    .iter()
                    .flatten()
                    .map(|choice| choice.literal)
                    .collect();
                self.formula.lexicographic_order(&left, &right, enabled)?;
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            self.memberships.push(behavior);
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(())
    }

    /// LLM: Select active realizations for one vertex color and exclude unrelated
    /// LLM: memberships. Called during palette extension; all original input contexts
    /// LLM: contribute to the union, allowing context-dependent outputs for one input.
    fn encode_vertex_supports(
        &mut self,
        behavior: &[Vec<Choice>],
        enabled: Lit,
    ) -> anyhow::Result<()> {
        // LLM: Match each original input context using this color's per-input tables.
        // LLM: Support must be accumulated across every context before restricting a table.
        let mut supports = BTreeMap::new();
        let color = self.enabled.len() as u32 - 1;
        for (context, input) in self.active_inputs.iter().enumerate() {
            self.formula.check()?;
            let inputs = port_inputs(input);
            let positions: Vec<_> = inputs
                .iter()
                .map(|&input| behavior[input_index(input)].clone())
                .collect();
            encode_constraint_supports(
                &mut self.formula,
                self.problem.output().active(),
                input.degree,
                &positions,
                &[enabled],
                &mut supports,
            )?;
            self.encoded.insert((context, vec![color; inputs.len()]));
        }
        // LLM: Each retained membership must belong to at least one selected active
        // LLM: realization for its input. Shrinking a valid strategy always permits this.
        for choice in behavior.iter().flatten() {
            let mut clause = supports.remove(&choice.literal).unwrap_or_default();
            clause.sort_unstable();
            clause.dedup();
            clause.push(!choice.literal);
            self.formula.add_clause(&clause)?;
        }
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
        // LLM: Spend a per-query clause-generation budget on affordable passive rows;
        // LLM: expensive rows retain exact lazy checks.
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
            // LLM: Each input's output domain is identical for every color. Count
            // LLM: all origin tuples over an upper bound on color placements.

            let placements = if self.kind == ColoringKind::GraphEdges {
                colors as usize
            } else {
                (colors as usize).saturating_pow(input.degree().min(2))
            };
            // LLM: Estimate computation cost to choose scheduling, without restricting the
            // LLM: exact search.
            let work = eager_encoding_work(
                input,
                placements,
                |input| self.memberships[0][input_index(input)].len(),
                cancellation,
            )?;
            // LLM: Choose between full eager origin enumeration and candidate-based refinement
            // LLM: using the work estimate.
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
        for (color, behavior) in self.memberships[..colors as usize].iter().enumerate() {
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for (&input, choices) in self.inputs.iter().zip(behavior) {
                // LLM: Visit the color placements required by the auxiliary promise while
                // LLM: retaining original input context.
                for choice in choices {
                    // LLM: Use the stored state for this operation without exposing mutable
                    // LLM: representation details.
                    self.formula.check()?;
                    emissions.push(Emission {
                        input: (input, color as u32),
                        output: choice.label,
                        source: choice.literal,
                    });
                }
            }
        }
        // LLM: Encode every forbidden low-degree source combination needed by future SAT
        // LLM: candidates.
        for index in eager {
            // LLM: Record prior work so the next pass need only visit newly relevant parent
            // LLM: pairs.
            let previous = self.eager_palettes[index];
            prepare::passive_families(
                std::slice::from_ref(&self.passive_inputs[index]),
                self.kind,
                colors,
                cancellation,
                &mut |family| {
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if previous != 0
                        && !family
                            .parts()
                            .iter()
                            .any(|part| part.labels().iter().any(|(_, color)| color >= previous))
                    {
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
        // LLM: Build one output-set table for each enabled color, indexed by every original
        // LLM: input.
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
        let passive_inputs = self.encode_passive(colors, &computation.cancellation)?;
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
                // LLM: A previously encoded active obligation must not fail verification;
                // LLM: report disagreement instead of looping indefinitely.
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
            // LLM: Learn one concrete passive counterexample before querying SAT again.
            let mut forbidden = None;
            prepare::passive_families(
                &passive_inputs,
                self.kind,
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
                    Ok(forbidden.is_none())
                },
            )?;
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            if let Some(sources) = forbidden {
                // LLM: Exclude only the simultaneous emissions that justify this failure.
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

#[cfg(test)]
mod tests {
    use super::super::{tests::plain, verify_witness};
    use super::*;

    /// LLM: Check that vertex support bounds respect original input occurrences and
    /// LLM: that edge-coloring strategies retain their broader context-dependent domains.
    #[test]
    fn vertex_support_bounds_use_all_original_input_contexts() {
        // LLM: A B has one occurrence of each input. Repeated a and multiple contexts
        // LLM: require larger supports; input names unused actively have zero support.
        let cases = [
            (
                super::super::tests::problem(
                    serde_json::json!({"kind":"independent", "input":{"active":"A B","passive":"A B"}, "output":{"active":"X X\nY Y", "passive":"X Y"}}),
                ),
                vec![1, 1],
            ),
            (
                super::super::tests::problem(
                    serde_json::json!({"kind":"independent", "input":{"active":"a a\na b","passive":"a b\nc c"}, "output":{"active":"X Y", "passive":"XY^2"}}),
                ),
                vec![3, 1, 0],
            ),
            (plain("X\nX^3", "X^2"), vec![4]),
        ];
        for (p, expected) in cases {
            for kind in [ColoringKind::Vertex, ColoringKind::IncidenceEdges] {
                let mut report = |_| {};
                let mut computation = Computation::new(Cancellation::default(), &mut report);
                let solver = PaletteSolver::new(
                    &p,
                    kind,
                    crate::algorithms::sat::backend(&computation.cancellation),
                    &mut computation,
                )
                .unwrap();
                assert_eq!(
                    solver.membership_bounds,
                    (kind == ColoringKind::Vertex).then_some(expected.clone())
                );
            }
        }
    }

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
            ColoringKind::Vertex,
            crate::algorithms::sat::backend(&computation.cancellation),
            &mut computation,
        )
        .unwrap();
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        for (step, colors) in [24, 2, 24, 2].into_iter().enumerate() {
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
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}
