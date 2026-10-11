//! SAT encoding of zero-round active validity for canonical output constraints.

use crate::algorithms::sat::Formula;
use rustsat::{solvers::Solve, types::Lit};

use crate::model::{Constraint, LabelId};

#[derive(Clone, Copy)]
pub(super) struct Choice {
    pub label: LabelId,
    pub literal: Lit,
}

/// positions[i] gives the choices for the ith port.
/// if positions[i][j].literal is true, then we use positions[i][j].label on port i.
/// This function encodes the requirement to pick a valid output configuration.
pub(super) fn encode_active<S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint,
    degree: u32,
    positions: &[Vec<Choice>],
) -> anyhow::Result<()> {
    let mut selectors = Vec::new();
    for configuration in constraint.configurations_at_degree(degree) {
        solver.check()?;
        let selector = solver.new_lit();
        selectors.push(selector);
        let mut assigned_to_part = vec![Vec::new(); configuration.parts().len()];
        for choices in positions {
            solver.check()?;
            let mut assignments = vec![!selector];
            let mut matching_literals = Vec::new();
            for (part, specification) in configuration.parts().iter().enumerate() {
                let compatible: Vec<_> = choices
                    .iter()
                    .filter(|choice| specification.labels().contains(choice.label))
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
