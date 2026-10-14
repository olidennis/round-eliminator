//! LLM: Shared SAT matching of output choices for solvability searches.

use crate::algorithms::sat::Formula;
use rustsat::{solvers::Solve, types::Lit};

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
    // LLM: Collect row-selection literals for existential validity of the entire
    // LLM: configuration.
    let mut selectors = Vec::new();
    for configuration in constraint.configurations_at_degree(degree) {
        // LLM: Update the current working state before continuing this operation.
        solver.check()?;
        let selector = solver.new_lit();
        selectors.push(selector);
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
    // LLM: When all gates are enabled, at least one configuration must match.
    // LLM: No configurations at this degree makes that obligation unsatisfiable.

    selectors.extend(enabled.iter().map(|literal| !*literal));
    solver.add_clause(&selectors)
}
