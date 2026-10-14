//! LLM: Plain problems with passive degree at most two reduce to finding
//! LLM: a compatible output support hitting every part of an active configuration.

use super::{LocalRule, ZeroRoundResult};
use crate::{
    algorithms::configurations::accepts,
    computation::{Computation, Phase},
    model::{Configuration, Input, OutputMapping, Problem},
};

/// LLM: Recognize trivial-input, unrestricted-mapping problems whose passive degrees are at
/// LLM: most two. Used before selecting the exact compatible-support zero-round search.
pub(super) fn applicable(problem: &Problem) -> bool {
    // LLM: The specialized proof requires no informative inputs, no permission restrictions,
    // LLM: and only passive degrees zero, one, or two.
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
    let mut allowed = Vec::new();
    // LLM: Keep only active-relevant labels whose homogeneous use is legal at every required
    // LLM: passive degree.
    for label in problem
        .graph_class()
        .active()
        .iter()
        .flat_map(|&degree| problem.output().active().configurations_at_degree(degree))
        .flat_map(|configuration| configuration.parts())
        .flat_map(|part| part.labels().iter())
        .collect::<std::collections::BTreeSet<_>>()
    {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        if problem.graph_class().passive().iter().all(|&degree| {
            degree == 0 || accepts(passive, &Configuration::new(vec![(label, degree)]).unwrap())
        }) {
            allowed.push(label);
        }
    }
    // LLM: Record the admissible pairings used to restrict exact matching or support search.
    let mut compatible = vec![vec![true; allowed.len()]; allowed.len()];
    if problem.graph_class().passive().contains(&2) {
        // LLM: Build the exact pair-compatibility relation over labels that are individually
        // LLM: safe on every passive degree.
        for (a, &left) in allowed.iter().enumerate() {
            // LLM: Check this label pair against the full passive constraint; diagonal pairs
            // LLM: were already required by individual safety.
            for (b, &right) in allowed.iter().enumerate() {
                computation.cancellation.check()?;
                compatible[a][b] = accepts(
                    passive,
                    &Configuration::from_occurrences([left, right]).unwrap(),
                );
            }
        }
    }
    // LLM: Switch from compatibility preparation to support selection across every required
    // LLM: active degree.
    computation.report(Phase::Solving, 0)?;
    let degrees = problem.graph_class().active();
    // LLM: Allocate or reconstruct one local rule per promised active context.
    let mut strategy: Vec<_> = degrees
        .iter()
        .map(|&degree| LocalRule {
            degree,
            input: None,
            output: Vec::new(),
        })
        .collect();
    // LLM: Handle the empty case explicitly before the general traversal or allocation path.
    if degrees.is_empty() {
        return Ok(ZeroRoundResult::Solvable { strategy });
    }
    // LLM: Each frame remembers only the next alternative, not every unexplored
    // LLM: branch. Stack memory is proportional to the chosen configuration depth.
    /// LLM: One active-support search decision: degree index, optional active row, part index,
    /// LLM: next alternative, output-prefix length, and remaining compatible label indices.
    /// LLM: Backtracking restores the prefix and keeps heap usage proportional to decision
    /// LLM: depth.
    struct Frame {
        degree: usize,
        row: Option<usize>,
        part: usize,
        next: usize,
        output_len: usize,
        candidates: Vec<usize>,
    }
    // LLM: Create the initial row-selection frame for one required active degree.
    let first = |degree, candidates| Frame {
        degree,
        row: None,
        part: 0,
        next: 0,
        output_len: 0,
        candidates,
    };
    // LLM: Keep backtracking decisions on the heap, avoiding recursion proportional to
    // LLM: configuration depth.
    let mut stack = vec![first(0, (0..allowed.len()).collect::<Vec<_>>())];
    let mut visited = 0u32;
    // LLM: Resume the next backtracking decision, restoring previous choices before trying
    // LLM: another feasible branch.
    while let Some(frame) = stack.last_mut() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        visited = visited.saturating_add(1);
        computation.report(Phase::Solving, visited)?;
        let degree = frame.degree;
        let configurations = problem
            .output()
            .active()
            .configurations_at_degree(degrees[degree]);
        // LLM: Continue assigning the chosen active row; otherwise try the next row for this
        // LLM: degree.
        if let Some(row) = frame.row {
            // LLM: Restore the output prefix of this part decision, then find its next
            // LLM: permitted compatible label.
            let configuration = &configurations[row];
            strategy[degree].output.truncate(frame.output_len);
            let mut selected = None;
            // LLM: Resume the next backtracking decision, restoring previous choices before
            // LLM: trying another feasible branch.
            while let Some(&label) = frame.candidates.get(frame.next) {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                computation.cancellation.check()?;
                frame.next += 1;
                // LLM: Require the selected label to be allowed by the current condensed
                // LLM: active part.
                if configuration.parts()[frame.part]
                    .labels()
                    .contains(allowed[label])
                {
                    selected = Some(label);
                    break;
                }
            }
            // LLM: Stop this alternative when the required choice is absent, leaving the
            // LLM: enclosing traversal to backtrack or continue.
            let Some(label) = selected else {
                stack.pop();
                continue;
            };
            // LLM: Restrict future labels to those compatible with the new choice and every
            // LLM: previously chosen label.
            let candidates: Vec<_> = frame
                .candidates
                .iter()
                .copied()
                .filter(|&other| compatible[label][other])
                .collect();
            // LLM: Use the chosen label for every occurrence of this part; shrinking a valid
            // LLM: support preserves universal passive validity.
            strategy[degree].output.extend(std::iter::repeat_n(
                allowed[label],
                configuration.parts()[frame.part].multiplicity() as usize,
            ));
            let part = frame.part + 1;
            // LLM: Continue this row, move to the next degree, or return when every required
            // LLM: active degree has a complete output.
            if part < configuration.parts().len() {
                // LLM: Retain the newly constructed choices in the result for this stage.
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
            // LLM: Try the next active row for this degree; exhausting its rows causes
            // LLM: backtracking.
            let row = frame.next;
            if row == configurations.len() {
                stack.pop();
                continue;
            }
            frame.next += 1;
            let candidates = frame.candidates.clone();
            strategy[degree].output.clear();
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if configurations[row].parts().is_empty() {
                if degree + 1 == degrees.len() {
                    return Ok(ZeroRoundResult::Solvable { strategy });
                }
                stack.push(first(degree + 1, candidates));
            } else {
                // LLM: Retain the newly constructed choices in the result for this stage.
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
    // LLM: Return the negative decision only after the required validity/search condition has
    // LLM: been settled.
    Ok(ZeroRoundResult::Unsolvable)
}
