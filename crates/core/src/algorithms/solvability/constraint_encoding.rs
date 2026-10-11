//! Shared SAT matching of output choices for solvability searches.

use crate::algorithms::sat::Formula;
use rustsat::{solvers::Solve, types::Lit};

use crate::model::{Constraint, LabelId};

#[derive(Clone, Copy)]
pub(crate) struct Choice {
    pub label: LabelId,
    pub literal: Lit,
}

/// positions[i] gives the choices for the ith position.
/// A true literal makes its label available at that position. Multiple labels
/// may be available: matching only requires one valid choice per position.
/// Require a valid output when every activation literal in `enabled` is true.
/// An empty activation list makes the requirement unconditional.
/// The positions must contain exactly `degree` entries. This encoding applies
/// to any constraint, independently of its active or passive role in a problem.
pub(crate) fn encode_constraint<S: Solve>(
    solver: &mut Formula<S>,
    constraint: &Constraint,
    degree: u32,
    positions: &[Vec<Choice>],
    enabled: &[Lit],
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
    // When all gates are enabled, at least one configuration must match.
    // No configurations at this degree makes that obligation unsatisfiable.
    selectors.extend(enabled.iter().map(|literal| !*literal));
    solver.add_clause(&selectors)
}
