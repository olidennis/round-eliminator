//! LLM: Shared SAT matching of output choices for solvability searches.

use crate::algorithms::sat::Formula;
use rustsat::{solvers::Solve, types::Lit};
use std::collections::BTreeMap;

use crate::model::{Constraint, LabelId};

/// LLM: One output label and the SAT literal that makes it available at a position. A true
/// LLM: membership permits that choice; it does not require a globally unique output at that
/// LLM: position.
#[derive(Clone, Copy)]
pub(crate) struct Choice {
    pub label: LabelId,
    pub literal: Lit,
}

/// LLM: Require an existential matching of per-position SAT choices to some allowed
/// LLM: same-degree row, gated by all supplied activation literals. Shared by zero-round and
/// LLM: coloring active encodings.
pub(crate) fn encode_constraint<S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint,
    degree: u32,
    positions: &[Vec<Choice>],
    enabled: &[Lit],
) -> anyhow::Result<()> {
    // LLM: Ordinary matching only requires sufficient output choices; callers that
    // LLM: shrink complete strategy supports use the support-collecting entry point.
    encode_with_support(solver, constraint, degree, positions, enabled, None)
}

/// LLM: Encode one active realization and collect which matching literals can use
/// LLM: each output membership. Vertex coloring unions this support across all input
/// LLM: contexts before excluding memberships absent from every selected realization.
pub(crate) fn encode_constraint_supports<S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint,
    degree: u32,
    positions: &[Vec<Choice>],
    enabled: &[Lit],
    supports: &mut BTreeMap<Lit, Vec<Lit>>,
) -> anyhow::Result<()> {
    // LLM: Selecting one row suffices for an existential active obligation and
    // LLM: gives support clauses a definite realization to refer to.
    encode_with_support(
        solver,
        constraint,
        degree,
        positions,
        enabled,
        Some(supports),
    )
}

/// LLM: Shared matching implementation, optionally retaining support provenance.
/// LLM: Identical position domains need only intersect each condensed part; otherwise
/// LLM: counted position-to-part assignments enforce an existential matching.
fn encode_with_support<S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint,
    degree: u32,
    positions: &[Vec<Choice>],
    enabled: &[Lit],
    mut supports: Option<&mut BTreeMap<Lit, Vec<Lit>>>,
) -> anyhow::Result<()> {
    // LLM: When all positions share one membership table, every part can fill its
    // LLM: slots independently. Label repetition is allowed, so no assignment flow is needed.
    let identical = !positions.is_empty()
        && positions[1..].iter().all(|position| {
            position.len() == positions[0].len()
                && position
                    .iter()
                    .zip(&positions[0])
                    .all(|(a, b)| a.label == b.label && a.literal == b.literal)
        });
    // LLM: Concrete degree-two rows are binary relations. Index per-position choices
    // LLM: once so their two orientations can use direct conjunctions instead of
    // LLM: constructing position-to-part matching variables for every row.
    let binary_choices = (degree == 2 && !identical).then(|| {
        positions
            .iter()
            .map(|choices| {
                choices
                    .iter()
                    .map(|choice| (choice.label, choice.literal))
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>()
    });
    // LLM: Collect row-selection literals for existential validity of the entire
    // LLM: configuration.
    let mut selectors = Vec::new();
    for configuration in constraint.configurations_at_degree(degree) {
        // LLM: Update the current working state before continuing this operation.
        solver.check()?;
        // LLM: Each concrete binary row allows one or two endpoint orientations.
        // LLM: A selector requires the corresponding output literal at each position;
        // LLM: missing mapped outputs simply make that orientation unavailable.
        if let Some(choices) = &binary_choices
            && configuration
                .parts()
                .iter()
                .all(|part| part.labels().len() == 1)
        {
            let labels: Vec<_> = configuration
                .parts()
                .iter()
                .flat_map(|part| {
                    std::iter::repeat_n(
                        part.labels().iter().next().unwrap(),
                        part.multiplicity() as usize,
                    )
                })
                .collect();
            for (orientation, [a, b]) in [[labels[0], labels[1]], [labels[1], labels[0]]]
                .into_iter()
                .enumerate()
            {
                if orientation == 1 && a == b {
                    continue;
                }
                let (Some(&a), Some(&b)) = (choices[0].get(&a), choices[1].get(&b)) else {
                    continue;
                };
                let selector = solver.new_lit();
                selectors.push(selector);
                solver.add_clause(&[!selector, a])?;
                solver.add_clause(&[!selector, b])?;
                if let Some(supports) = supports.as_deref_mut() {
                    supports.entry(a).or_default().push(selector);
                    supports.entry(b).or_default().push(selector);
                }
            }
            continue;
        }
        let selector = solver.new_lit();
        selectors.push(selector);
        // LLM: Shared domains reduce active validity to one intersection clause per
        // LLM: part. Retain row selectors as supports for any compatible output membership.
        if identical {
            for part in configuration.parts() {
                let mut clause = vec![!selector];
                for choice in &positions[0] {
                    if part.labels().contains(choice.label) {
                        clause.push(choice.literal);
                        if let Some(supports) = supports.as_deref_mut() {
                            supports.entry(choice.literal).or_default().push(selector);
                        }
                    }
                }
                solver.add_clause(&clause)?;
            }
            continue;
        }
        let mut assigned_to_part = vec![Vec::new(); configuration.parts().len()];
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        for choices in positions {
            // LLM: Update the current working state before continuing this operation.
            solver.check()?;
            let mut assignments = vec![!selector];
            let mut matching_literals = Vec::new();
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            for (part, specification) in configuration.parts().iter().enumerate() {
                // LLM: Record the admissible pairings used to restrict exact matching or
                // LLM: support search.
                let compatible: Vec<_> = choices
                    .iter()
                    .filter(|choice| specification.labels().contains(choice.label))
                    .map(|choice| choice.literal)
                    .collect();
                if compatible.is_empty() {
                    continue;
                }
                // LLM: Prepare `matching` for the following computation; it stores the
                // LLM: matching for this stage.
                let matching = solver.new_lit();
                // LLM: A membership is relevant only when some chosen matching
                // LLM: assigns its original-input occurrence to a part permitting that label.
                if let Some(supports) = supports.as_deref_mut() {
                    for &literal in &compatible {
                        supports.entry(literal).or_default().push(matching);
                    }
                }
                assignments.push(matching);
                matching_literals.push(matching);
                assigned_to_part[part].push(matching);
                solver.add_clause(&[!matching, selector])?;
                let mut clause = vec![!matching];
                clause.extend(compatible);
                solver.add_clause(&clause)?;
            }
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            solver.add_clause(&assignments)?;
            solver.at_most(&matching_literals, 1)?;
        }
        // LLM: Bound assignment counts so a selected row's part capacities cannot be exceeded.
        for (part, assignments) in configuration.parts().iter().zip(assigned_to_part) {
            solver.at_most(&assignments, part.multiplicity() as usize)?;
        }
    }
    // LLM: Support-aware callers choose one realization per active context. Multiple
    // LLM: contexts may still contribute different outputs to a shared input's table.
    if supports.is_some() {
        solver.at_most(&selectors, 1)?;
    }
    // LLM: When all gates are enabled, at least one configuration must match.
    // LLM: No configurations at this degree makes that obligation unsatisfiable.

    selectors.extend(enabled.iter().map(|literal| !*literal));
    solver.add_clause(&selectors)
}
