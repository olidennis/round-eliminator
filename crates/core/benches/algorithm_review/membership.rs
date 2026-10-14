//! LLM: Prototype: one emission membership per original input/output, with active
//! LLM: matching obligations and the same exact passive counterexample checker.

use super::{PassiveMode, ZeroRoundResult, prepare};
use crate::{
    algorithms::{
        configurations::allows_positions_with,
        maximize::MaximizationResult,
        sat::{Formula, backend},
        solvability::{
            constraint_encoding::{Choice, encode_constraint},
            passive::{Emission, forbidden_with_maximum, visit_forbidden},
        },
    },
    computation::{Computation, Phase},
    model::{LabelId, LabelSet, Problem},
};
use rustsat::{
    solvers::{SolveIncremental, SolverResult},
    types::TernaryVal,
};
use std::collections::{BTreeMap, BTreeSet};

/// LLM: Decide deterministic active-side zero-round solvability, with the selected passive
/// LLM: validity guarantee. Used by the API and coloring preflight; cancellation and backend
/// LLM: errors are distinct from a negative decision.
pub(super) fn check(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    search(
        problem,
        mode,
        maximum,
        backend(&computation.cancellation),
        computation,
    )
}
/// LLM: Solve the experimental per-input output-membership encoding, lazily adding violated
/// LLM: active and passive obligations. Used only to benchmark an alternative zero-round
/// LLM: formulation.
fn search<S: SolveIncremental>(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    solver: S,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Enumerating, 0)?;
    let mut rules = prepare::prepare_local_rules(problem, &computation.cancellation)?;
    let mut domains = BTreeMap::<Option<LabelId>, BTreeSet<LabelId>>::new();
    let mut local_domains = BTreeSet::new();
    // LLM: Process the relevant label choices while preserving their table identities.
    for rule in &rules {
        for port in 0..rule.degree as usize {
            computation.cancellation.check()?;
            local_domains.insert((rule.degree, rule.input.as_ref().map(|labels| labels[port])));
        }
    }
    // LLM: Restrict candidate outputs to this degree and the actual incidence input.
    for (degree, input) in local_domains {
        domains
            .entry(input)
            .or_default()
            .extend(prepare::output_domain(problem, degree, input));
    }
    let mut formula = Formula::new(solver, computation.cancellation.clone());
    computation.report(Phase::Encoding, 0)?;
    // LLM: Allocate output availability variables indexed by the coloring information and
    // LLM: original inputs.
    let memberships: BTreeMap<_, Vec<_>> = domains
        .into_iter()
        .map(|(input, labels)| {
            // LLM: Update the current working state before continuing this operation.
            (
                input,
                labels
                    .into_iter()
                    .map(|label| Choice {
                        label,
                        literal: formula.new_lit(),
                    })
                    .collect(),
            )
        })
        .collect();
    // LLM: Require a whole valid active output by matching available per-port choices to
    // LLM: condensed part capacities.
    for rule in &rules {
        // LLM: Associate each occurrence with its available outputs before checking a whole
        // LLM: configuration.
        let positions: Vec<_> = (0..rule.degree as usize)
            .map(|port| memberships[&rule.input.as_ref().map(|labels| labels[port])].clone())
            .collect();
        // LLM: Require a whole valid active output by matching available per-port choices to
        // LLM: condensed part capacities.
        encode_constraint(
            &mut formula,
            problem.output().active(),
            rule.degree,
            &positions,
            &[],
        )?;
    }
    // LLM: Collect available outputs with their input/color descriptors and source provenance.
    let emissions: Vec<_> = memberships
        .iter()
        .flat_map(|(&input, choices)| {
            choices.iter().map(move |choice| Emission {
                input,
                output: choice.label,
                source: choice.literal,
            })
        })
        .collect();
    // LLM: Retain the original passive promise as condensed input families.
    let mut passive = prepare::prepare_passive_inputs(problem, &computation.cancellation)?;
    for input in passive.iter().filter(|input| input.degree() <= 2) {
        // LLM: Encode every forbidden low-degree source combination needed by future SAT
        // LLM: candidates.
        visit_forbidden(
            problem.output().passive(),
            input,
            &emissions,
            &computation.cancellation,
            |sources| {
                // LLM: Add the necessary SAT restriction; a refinement clause forbids the
                // LLM: selected violating combination.
                formula.add_clause(
                    &sources
                        .into_iter()
                        .map(|source| !source)
                        .collect::<Vec<_>>(),
                )
            },
        )?;
    }
    // LLM: Update the current working state before continuing this operation.
    passive.retain(|input| input.degree() > 2);
    let mut iterations = 0;
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    loop {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Solving, iterations)?;
        match formula.solver.solve()? {
            SolverResult::Unsat => return Ok(ZeroRoundResult::Unsolvable),
            SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
            SolverResult::Sat => {}
        }
        iterations += 1;
        computation.report(Phase::Verifying, iterations)?;
        // LLM: Prepare `selected` for the following computation; it stores the selected for
        // LLM: this stage.
        let mut selected = Vec::new();
        for &emission in &emissions {
            computation.cancellation.check()?;
            if formula.solver.lit_val(emission.source)? == TernaryVal::True {
                selected.push(emission);
            }
        }
        // LLM: Verify the candidate's universal passive outputs, reusing only a compatible
        // LLM: maximum certificate.
        if let Some(sources) = forbidden_with_maximum(
            problem.output().passive(),
            &passive,
            &selected,
            mode,
            maximum,
            |input| input,
            &computation.cancellation,
        )? {
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            formula.add_clause(
                &sources
                    .into_iter()
                    .map(|source| !source)
                    .collect::<Vec<_>>(),
            )?;
            continue;
        }
        // LLM: Greedily fix ports while preserving existence of an active matching.
        // LLM: Every selected output remains within the already verified emissions.

        for rule in &mut rules {
            // LLM: Associate each occurrence with its available outputs before checking a
            // LLM: whole configuration.
            let mut positions: Vec<LabelSet> = (0..rule.degree as usize)
                .map(|port| {
                    // LLM: Retain or prepare the original promised input semantics for this
                    // LLM: stage.
                    let input = rule.input.as_ref().map(|labels| labels[port]);
                    LabelSet::new(
                        selected
                            .iter()
                            .filter(|emission| emission.input == input)
                            .map(|emission| emission.output)
                            .collect(),
                    )
                })
                .collect();
            // LLM: Update the current working state before continuing this operation.
            rule.output.clear();
            for port in 0..positions.len() {
                // LLM: Prepare `choices` for the following computation; it stores the choices
                // LLM: for this stage.
                let choices = positions[port].clone();
                let mut picked = None;
                // LLM: Check whether these local output domains can jointly realize some valid
                // LLM: active configuration.
                for label in choices.iter() {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    computation.cancellation.check()?;
                    positions[port] = LabelSet::new(vec![label]);
                    // LLM: Check whether these local output domains can jointly realize some
                    // LLM: valid active configuration.
                    if allows_positions_with(
                        problem.output().active(),
                        &positions,
                        &computation.cancellation,
                    )? {
                        picked = Some(label);
                        break;
                    }
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                rule.output.push(picked.ok_or_else(|| {
                    anyhow::anyhow!("Membership encoding has no active matching.")
                })?);
            }
        }
        // LLM: Return the complete strategy certificate after all required contexts have been
        // LLM: covered.
        return Ok(ZeroRoundResult::Solvable { strategy: rules });
    }
}
