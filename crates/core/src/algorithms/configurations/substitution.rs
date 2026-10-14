//! Substitute choices in each condensed part, preserving repetitions.

use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Constraint, LabelSet, Part},
};

/// An empty image makes the whole configuration impossible. Degree-zero
/// configurations have no parts to substitute and remain valid. This operation
/// does not expand occurrences or normalize the order of condensed parts.
pub(crate) fn map_parts_with<L: Copy + Ord, M: Copy + Ord>(
    constraint: &Constraint<L>,
    cancellation: &Cancellation,
    mut image: impl FnMut(&LabelSet<L>) -> Result<LabelSet<M>, Cancelled>,
) -> Result<Constraint<M>, Cancelled> {
    let mut result = Vec::new();
    'configurations: for configuration in constraint.configurations() {
        cancellation.check()?;
        let mut parts = Vec::with_capacity(configuration.parts().len());
        for part in configuration.parts() {
            cancellation.check()?;
            let labels = image(part.labels())?;
            if labels.is_empty() {
                continue 'configurations;
            }
            parts.push(Part::new(labels, part.multiplicity()).expect("nonempty image"));
        }
        result.push(CondensedConfiguration::new(parts).expect("degree preserved"));
    }
    Ok(Constraint::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LabelId;

    #[test]
    fn preserves_counts_and_zero_degree_but_discards_impossible_rows() {
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
        let result = map_parts_with(&initial, &Cancellation::default(), |labels| {
            Ok(if labels.contains(LabelId(0)) {
                LabelSet::new(vec![LabelId(2), LabelId(3)])
            } else {
                LabelSet::default()
            })
        })
        .unwrap();
        assert_eq!(result.configurations().count(), 2);
        assert_eq!(result.configurations_at_degree(0).len(), 1);
        assert_eq!(result.configurations_at_degree(100_000)[0].parts().len(), 1);
        assert_eq!(
            result.configurations_at_degree(100_000)[0].parts()[0].multiplicity(),
            100_000
        );
    }
}
