//! LLM: Prototype: plain problems with passive degree at most two reduce to finding
//! LLM: a compatible output support hitting every part of an active configuration.

use super::{LocalRule, ZeroRoundResult};
use crate::{
    algorithms::configurations::accepts,
    computation::{Computation, Phase},
    model::{Configuration, Input, LabelId, OutputMapping, Problem},
};

/// LLM: Recognize trivial-input, unrestricted-mapping problems whose passive degrees are at
/// LLM: most two. Used before selecting the exact compatible-support zero-round search.
pub(super) fn applicable(problem: &Problem) -> bool {
    // LLM: Update the current working state before continuing this operation.
    matches!(problem.input(), Input::Trivial)
        && matches!(problem.mapping(), OutputMapping::Unrestricted)
        && problem
            .graph_class()
            .passive()
            .iter()
            .all(|&degree| degree <= 2)
}
/// LLM: Find a compatible output support hitting one active row at every declared degree, or
/// LLM: prove none exists. Used by plain passive-degree-at-most-two zero-round decisions;
/// LLM: selection is configuration-based and interruption is cooperative.
pub(super) fn check(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    // LLM: Require the specialized search precondition and reject an impossible isolated
    // LLM: passive node before selecting a support.
    assert!(applicable(problem));
    computation.report(Phase::Preparing, 0)?;
    let passive = problem.output().passive();
    if problem.graph_class().passive().contains(&0) && !accepts(passive, &Configuration::default())
    {
        return Ok(ZeroRoundResult::Unsolvable);
    }
    // LLM: Prepare `allowed` for the following computation; it stores the allowed for this
    // LLM: stage.
    let allowed: Vec<_> = problem
        .output()
        .active()
        .labels()
        .filter(|&label| {
            problem.graph_class().passive().iter().all(|&degree| {
                degree == 0 || accepts(passive, &Configuration::new(vec![(label, degree)]).unwrap())
            })
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    // LLM: Record the admissible pairings used to restrict exact matching or support search.
    let mut compatible = vec![vec![true; allowed.len()]; allowed.len()];
    if problem.graph_class().passive().contains(&2) {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (a, &left) in allowed.iter().enumerate() {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for (b, &right) in allowed.iter().enumerate() {
                computation.cancellation.check()?;
                compatible[a][b] = accepts(
                    passive,
                    &Configuration::from_occurrences([left, right]).unwrap(),
                );
            }
        }
    }
    // LLM: Allocate or reconstruct one local rule per promised active context.
    let mut strategy = Vec::new();
    let found = choose_degree(
        problem,
        0,
        &allowed,
        &compatible,
        &(0..allowed.len()).collect::<Vec<_>>(),
        &mut strategy,
        computation,
    )?;
    // LLM: Return the negative decision only after the required validity/search condition has
    // LLM: been settled.
    Ok(if found {
        ZeroRoundResult::Solvable { strategy }
    } else {
        ZeroRoundResult::Unsolvable
    })
}
/// LLM: Select an active row and compatible output support for the next required degree. Used
/// LLM: only by the retained recursive degree-two search prototype.
fn choose_degree(
    problem: &Problem,
    degree_index: usize,
    labels: &[LabelId],
    compatible: &[Vec<bool>],
    candidates: &[usize],
    strategy: &mut Vec<LocalRule>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<bool> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    computation.cancellation.check()?;
    if degree_index == problem.graph_class().active().len() {
        return Ok(true);
    }
    let degree = problem.graph_class().active()[degree_index];
    // LLM: Explore the next counted alternative while preserving the required total
    // LLM: capacities.
    for configuration in problem.output().active().configurations_at_degree(degree) {
        // LLM: Prepare `choices` for the following computation; it stores the choices for this
        // LLM: stage.
        let choices: Vec<Vec<_>> = configuration
            .parts()
            .iter()
            .map(|part| {
                candidates
                    .iter()
                    .copied()
                    .filter(|&index| part.labels().contains(labels[index]))
                    .collect()
            })
            .collect();
        // LLM: Keep repetitions as count distributions instead of expanding permutations of
        // LLM: occurrences.
        let counts: Vec<_> = configuration
            .parts()
            .iter()
            .map(|part| part.multiplicity())
            .collect();
        let mut output = Vec::new();
        // LLM: Reject an infeasible capacity state before exploring its completions.
        if choose_part(
            problem,
            degree_index,
            &choices,
            &counts,
            0,
            labels,
            compatible,
            candidates,
            &mut output,
            strategy,
            computation,
        )? {
            return Ok(true);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(false)
}
/// LLM: Choose one output label per condensed part while narrowing the globally compatible
/// LLM: support. Used only by the retained recursive degree-two prototype.
#[allow(clippy::too_many_arguments)]
fn choose_part(
    problem: &Problem,
    degree_index: usize,
    choices: &[Vec<usize>],
    counts: &[u32],
    part: usize,
    labels: &[LabelId],
    compatible: &[Vec<bool>],
    candidates: &[usize],
    output: &mut Vec<LabelId>,
    strategy: &mut Vec<LocalRule>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<bool> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    computation.cancellation.check()?;
    if part == choices.len() {
        // LLM: Retain the newly constructed choices in the result for this stage.
        strategy.push(LocalRule {
            degree: problem.graph_class().active()[degree_index],
            input: None,
            output: output.clone(),
        });
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if choose_degree(
            problem,
            degree_index + 1,
            labels,
            compatible,
            candidates,
            strategy,
            computation,
        )? {
            return Ok(true);
        }
        // LLM: Update the current working state before continuing this operation.
        strategy.pop();
        return Ok(false);
    }
    // LLM: Explore the next counted alternative while preserving the required total
    // LLM: capacities.
    for &label in &choices[part] {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if !candidates.contains(&label) {
            continue;
        }
        let narrowed: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|&other| compatible[label][other])
            .collect();
        // LLM: Prepare `length` for the following computation; it stores the length for this
        // LLM: stage.
        let length = output.len();
        output.extend(std::iter::repeat_n(labels[label], counts[part] as usize));
        // LLM: Reject an infeasible capacity state before exploring its completions.
        if choose_part(
            problem,
            degree_index,
            choices,
            counts,
            part + 1,
            labels,
            compatible,
            &narrowed,
            output,
            strategy,
            computation,
        )? {
            return Ok(true);
        }
        // LLM: Update the current working state before continuing this operation.
        output.truncate(length);
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(false)
}
