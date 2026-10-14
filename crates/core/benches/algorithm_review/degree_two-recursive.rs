//! Prototype: plain problems with passive degree at most two reduce to finding
//! a compatible output support hitting every part of an active configuration.
use super::{LocalRule, ZeroRoundResult};
use crate::{
    algorithms::configurations::accepts,
    computation::{Computation, Phase},
    model::{Configuration, Input, LabelId, OutputMapping, Problem},
};

pub(super) fn applicable(problem: &Problem) -> bool {
    matches!(problem.input(), Input::Trivial)
        && matches!(problem.mapping(), OutputMapping::Unrestricted)
        && problem
            .graph_class()
            .passive()
            .iter()
            .all(|&degree| degree <= 2)
}
pub(super) fn check(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<ZeroRoundResult> {
    assert!(applicable(problem));
    computation.report(Phase::Preparing, 0)?;
    let passive = problem.output().passive();
    if problem.graph_class().passive().contains(&0) && !accepts(passive, &Configuration::default())
    {
        return Ok(ZeroRoundResult::Unsolvable);
    }
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
    let mut compatible = vec![vec![true; allowed.len()]; allowed.len()];
    if problem.graph_class().passive().contains(&2) {
        for (a, &left) in allowed.iter().enumerate() {
            for (b, &right) in allowed.iter().enumerate() {
                computation.cancellation.check()?;
                compatible[a][b] = accepts(
                    passive,
                    &Configuration::from_occurrences([left, right]).unwrap(),
                );
            }
        }
    }
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
    Ok(if found {
        ZeroRoundResult::Solvable { strategy }
    } else {
        ZeroRoundResult::Unsolvable
    })
}
fn choose_degree(
    problem: &Problem,
    degree_index: usize,
    labels: &[LabelId],
    compatible: &[Vec<bool>],
    candidates: &[usize],
    strategy: &mut Vec<LocalRule>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<bool> {
    computation.cancellation.check()?;
    if degree_index == problem.graph_class().active().len() {
        return Ok(true);
    }
    let degree = problem.graph_class().active()[degree_index];
    for configuration in problem.output().active().configurations_at_degree(degree) {
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
        let counts: Vec<_> = configuration
            .parts()
            .iter()
            .map(|part| part.multiplicity())
            .collect();
        let mut output = Vec::new();
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
    Ok(false)
}
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
    computation.cancellation.check()?;
    if part == choices.len() {
        strategy.push(LocalRule {
            degree: problem.graph_class().active()[degree_index],
            input: None,
            output: output.clone(),
        });
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
        strategy.pop();
        return Ok(false);
    }
    for &label in &choices[part] {
        if !candidates.contains(&label) {
            continue;
        }
        let narrowed: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|&other| compatible[label][other])
            .collect();
        let length = output.len();
        output.extend(std::iter::repeat_n(labels[label], counts[part] as usize));
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
        output.truncate(length);
    }
    Ok(false)
}
