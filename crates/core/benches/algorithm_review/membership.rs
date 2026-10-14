//! Prototype: one emission membership per original input/output, with active
//! matching obligations and the same exact passive counterexample checker.
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

pub(super) fn check(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    search(
        problem,
        mode,
        maximum,
        backend(&computation.cancellation),
        computation,
    )
}
fn search<S: SolveIncremental>(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    solver: S,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    computation.report(Phase::Enumerating, 0)?;
    let mut rules = prepare::prepare_local_rules(problem, &computation.cancellation)?;
    let mut domains = BTreeMap::<Option<LabelId>, BTreeSet<LabelId>>::new();
    let mut local_domains = BTreeSet::new();
    for rule in &rules {
        for port in 0..rule.degree as usize {
            computation.cancellation.check()?;
            local_domains.insert((rule.degree, rule.input.as_ref().map(|labels| labels[port])));
        }
    }
    for (degree, input) in local_domains {
        domains
            .entry(input)
            .or_default()
            .extend(prepare::output_domain(problem, degree, input));
    }
    let mut formula = Formula::new(solver, computation.cancellation.clone());
    computation.report(Phase::Encoding, 0)?;
    let memberships: BTreeMap<_, Vec<_>> = domains
        .into_iter()
        .map(|(input, labels)| {
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
    for rule in &rules {
        let positions: Vec<_> = (0..rule.degree as usize)
            .map(|port| memberships[&rule.input.as_ref().map(|labels| labels[port])].clone())
            .collect();
        encode_constraint(
            &mut formula,
            problem.output().active(),
            rule.degree,
            &positions,
            &[],
        )?;
    }
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
    let mut passive = prepare::prepare_passive_inputs(problem, &computation.cancellation)?;
    for input in passive.iter().filter(|input| input.degree() <= 2) {
        visit_forbidden(
            problem.output().passive(),
            input,
            &emissions,
            &computation.cancellation,
            |sources| {
                formula.add_clause(
                    &sources
                        .into_iter()
                        .map(|source| !source)
                        .collect::<Vec<_>>(),
                )
            },
        )?;
    }
    passive.retain(|input| input.degree() > 2);
    let mut iterations = 0;
    loop {
        computation.report(Phase::Solving, iterations)?;
        match formula.solver.solve()? {
            SolverResult::Unsat => return Ok(ZeroRoundResult::Unsolvable),
            SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
            SolverResult::Sat => {}
        }
        iterations += 1;
        computation.report(Phase::Verifying, iterations)?;
        let mut selected = Vec::new();
        for &emission in &emissions {
            computation.cancellation.check()?;
            if formula.solver.lit_val(emission.source)? == TernaryVal::True {
                selected.push(emission);
            }
        }
        if let Some(sources) = forbidden_with_maximum(
            problem.output().passive(),
            &passive,
            &selected,
            mode,
            maximum,
            |input| input,
            &computation.cancellation,
        )? {
            formula.add_clause(
                &sources
                    .into_iter()
                    .map(|source| !source)
                    .collect::<Vec<_>>(),
            )?;
            continue;
        }
        // Greedily fix ports while preserving existence of an active matching.
        // Every selected output remains within the already verified emissions.
        for rule in &mut rules {
            let mut positions: Vec<LabelSet> = (0..rule.degree as usize)
                .map(|port| {
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
            rule.output.clear();
            for port in 0..positions.len() {
                let choices = positions[port].clone();
                let mut picked = None;
                for label in choices.iter() {
                    computation.cancellation.check()?;
                    positions[port] = LabelSet::new(vec![label]);
                    if allows_positions_with(
                        problem.output().active(),
                        &positions,
                        &computation.cancellation,
                    )? {
                        picked = Some(label);
                        break;
                    }
                }
                rule.output.push(picked.ok_or_else(|| {
                    anyhow::anyhow!("Membership encoding has no active matching.")
                })?);
            }
        }
        return Ok(ZeroRoundResult::Solvable { strategy: rules });
    }
}
