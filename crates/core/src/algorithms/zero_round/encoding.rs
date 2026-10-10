//! SAT encoding of zero-round active validity for ordinary and pair constraints.

use crate::algorithms::sat::Formula;
use rustsat::{solvers::Solve, types::Lit};

use crate::{labels::LabelId, problem::Constraint};

#[derive(Clone, Copy)]
pub(super) struct Choice {
    pub label: LabelId,
    pub literal: Lit,
}

/// Select an allowed condensed configuration and match output occurrences to
/// its parts. Part capacities sum to the degree, so upper bounds plus one
/// matched part per occurrence enforce the exact multiplicities.
pub(super) fn encode_active<L: Copy + Ord, S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint<L>,
    degree: u32,
    positions: &[Vec<Choice>],
    element: impl Fn(usize, LabelId) -> L,
) -> anyhow::Result<()> {
    let mut selectors = Vec::new();
    for configuration in constraint.configurations_at_degree(degree) {
        solver.check()?;
        let selector = solver.new_lit();
        selectors.push(selector);
        let mut assigned_to_part = vec![Vec::new(); configuration.parts().len()];
        for (position, choices) in positions.iter().enumerate() {
            solver.check()?;
            let mut assignments = vec![!selector];
            let mut matching_literals = Vec::new();
            for (part, specification) in configuration.parts().iter().enumerate() {
                let compatible: Vec<_> = choices
                    .iter()
                    .filter(|choice| {
                        specification
                            .labels()
                            .contains(element(position, choice.label))
                    })
                    .map(|choice| choice.literal)
                    .collect();
                if compatible.is_empty() {
                    continue;
                }
                let matching = solver.new_lit();
                assignments.push(matching);
                matching_literals.push(matching);
                assigned_to_part[part].push(matching);
                solver.add_clause(&[!matching, selector])?;
                let mut clause = vec![!matching];
                clause.extend(compatible);
                solver.add_clause(&clause)?;
            }
            solver.add_clause(&assignments)?;
            solver.at_most(&matching_literals, 1)?;
        }
        for (part, assignments) in configuration.parts().iter().zip(assigned_to_part) {
            solver.at_most(&assignments, part.multiplicity() as usize)?;
        }
    }
    // If no configuration has this degree, the empty clause makes it UNSAT.
    solver.add_clause(&selectors)
}
