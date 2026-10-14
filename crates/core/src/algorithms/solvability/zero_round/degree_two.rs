//! Plain problems with passive degree at most two reduce to finding
//! a compatible output support hitting every part of an active configuration.
use super::{LocalRule, ZeroRoundResult};
use crate::{
    algorithms::configurations::accepts,
    computation::{Computation, Phase},
    model::{Configuration, Input, OutputMapping, Problem},
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
    let mut allowed = Vec::new();
    for label in problem
        .graph_class()
        .active()
        .iter()
        .flat_map(|&degree| problem.output().active().configurations_at_degree(degree))
        .flat_map(|configuration| configuration.parts())
        .flat_map(|part| part.labels().iter())
        .collect::<std::collections::BTreeSet<_>>()
    {
        computation.cancellation.check()?;
        if problem.graph_class().passive().iter().all(|&degree| {
            degree == 0 || accepts(passive, &Configuration::new(vec![(label, degree)]).unwrap())
        }) {
            allowed.push(label);
        }
    }
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
    computation.report(Phase::Solving, 0)?;
    let degrees = problem.graph_class().active();
    let mut strategy: Vec<_> = degrees
        .iter()
        .map(|&degree| LocalRule {
            degree,
            input: None,
            output: Vec::new(),
        })
        .collect();
    if degrees.is_empty() {
        return Ok(ZeroRoundResult::Solvable { strategy });
    }
    // Each frame remembers only the next alternative, not every unexplored
    // branch. Stack memory is proportional to the chosen configuration depth.
    struct Frame {
        degree: usize,
        row: Option<usize>,
        part: usize,
        next: usize,
        output_len: usize,
        candidates: Vec<usize>,
    }
    let first = |degree, candidates| Frame {
        degree,
        row: None,
        part: 0,
        next: 0,
        output_len: 0,
        candidates,
    };
    let mut stack = vec![first(0, (0..allowed.len()).collect::<Vec<_>>())];
    let mut visited = 0u32;
    while let Some(frame) = stack.last_mut() {
        computation.cancellation.check()?;
        visited = visited.saturating_add(1);
        computation.report(Phase::Solving, visited)?;
        let degree = frame.degree;
        let configurations = problem
            .output()
            .active()
            .configurations_at_degree(degrees[degree]);
        if let Some(row) = frame.row {
            let configuration = &configurations[row];
            strategy[degree].output.truncate(frame.output_len);
            let mut selected = None;
            while let Some(&label) = frame.candidates.get(frame.next) {
                computation.cancellation.check()?;
                frame.next += 1;
                if configuration.parts()[frame.part]
                    .labels()
                    .contains(allowed[label])
                {
                    selected = Some(label);
                    break;
                }
            }
            let Some(label) = selected else {
                stack.pop();
                continue;
            };
            let candidates: Vec<_> = frame
                .candidates
                .iter()
                .copied()
                .filter(|&other| compatible[label][other])
                .collect();
            strategy[degree].output.extend(std::iter::repeat_n(
                allowed[label],
                configuration.parts()[frame.part].multiplicity() as usize,
            ));
            let part = frame.part + 1;
            if part < configuration.parts().len() {
                stack.push(Frame {
                    degree,
                    row: Some(row),
                    part,
                    next: 0,
                    output_len: strategy[degree].output.len(),
                    candidates,
                });
            } else if degree + 1 == degrees.len() {
                return Ok(ZeroRoundResult::Solvable { strategy });
            } else {
                stack.push(first(degree + 1, candidates));
            }
        } else {
            let row = frame.next;
            if row == configurations.len() {
                stack.pop();
                continue;
            }
            frame.next += 1;
            let candidates = frame.candidates.clone();
            strategy[degree].output.clear();
            if configurations[row].parts().is_empty() {
                if degree + 1 == degrees.len() {
                    return Ok(ZeroRoundResult::Solvable { strategy });
                }
                stack.push(first(degree + 1, candidates));
            } else {
                stack.push(Frame {
                    degree,
                    row: Some(row),
                    part: 0,
                    next: 0,
                    output_len: 0,
                    candidates,
                });
            }
        }
    }
    Ok(ZeroRoundResult::Unsolvable)
}
