//! LLM: Substitute choices in each condensed part, preserving repetitions.

use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Constraint, LabelSet, Part},
};

/// LLM: Substitute the choice set of every condensed part, retaining its count. An empty image
/// LLM: removes the whole row; used by RE, simplification, and paired/input transformations.
pub(crate) fn map_parts_with<L: Copy + Ord, M: Copy + Ord>(
    constraint: &Constraint<L>,
    cancellation: &Cancellation,
    mut image: impl FnMut(&LabelSet<L>) -> Result<LabelSet<M>, Cancelled>,
) -> Result<Constraint<M>, Cancelled> {
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let mut result = Vec::new();
    'configurations: for configuration in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let mut parts = Vec::with_capacity(configuration.parts().len());
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for part in configuration.parts() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let labels = image(part.labels())?;
            if labels.is_empty() {
                continue 'configurations;
            }
            parts.push(Part::new(labels, part.multiplicity()).expect("nonempty image"));
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        result.push(CondensedConfiguration::new(parts).expect("degree preserved"));
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LabelId;

    /// LLM: Regression test: verify that preserves counts and zero degree but discards
    /// LLM: impossible rows. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn preserves_counts_and_zero_degree_but_discards_impossible_rows() {
        // LLM: Prepare `initial` for the following computation; it stores the initial for this
        // LLM: stage.
        let initial = Constraint::new(vec![
            CondensedConfiguration::new(vec![]).unwrap(),
            CondensedConfiguration::new(vec![
                Part::new(LabelSet::new(vec![LabelId(0)]), 100_000).unwrap(),
            ])
            .unwrap(),
            CondensedConfiguration::new(vec![
                Part::new(LabelSet::new(vec![LabelId(1)]), 2).unwrap(),
            ])
            .unwrap(),
        ]);
        // LLM: Substitute whole choice sets while preserving counts and dropping impossible
        // LLM: rows.
        let result = map_parts_with(&initial, &Cancellation::default(), |labels| {
            Ok(if labels.contains(LabelId(0)) {
                LabelSet::new(vec![LabelId(2), LabelId(3)])
            } else {
                LabelSet::default()
            })
        })
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(result.configurations().count(), 2);
        assert_eq!(result.configurations_at_degree(0).len(), 1);
        assert_eq!(result.configurations_at_degree(100_000)[0].parts().len(), 1);
        assert_eq!(
            result.configurations_at_degree(100_000)[0].parts()[0].multiplicity(),
            100_000
        );
    }
}
