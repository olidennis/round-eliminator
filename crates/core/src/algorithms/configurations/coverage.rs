//! Residual contexts and inclusion in a union of condensed configurations.

use super::{accepts_with, contains_condensed_with, visit_multisets_with};
use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part},
};

/// given a `constraint` and a `label`, takes all configurations containing `label`
/// and constructs all configurations with 1 less occurrence of `label`.
pub(crate) fn contexts_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    label: L,
    cancellation: &Cancellation,
) -> Result<Constraint<L>, Cancelled> {
    cancellation.check()?;
    let mut contexts = Vec::new();
    for family in constraint.configurations() {
        for (index, part) in family.parts().iter().enumerate() {
            cancellation.check()?;
            if !part.labels().contains(label) {
                continue;
            }
            let parts = family
                .parts()
                .iter()
                .enumerate()
                .map(|(i, part)| {
                    Part::new(
                        part.labels().clone(),
                        part.multiplicity() - u32::from(i == index),
                    )
                    .expect("existing nonempty label set")
                })
                .collect();
            contexts.push(CondensedConfiguration::new(parts).expect("degree decreases"));
        }
    }
    Ok(Constraint::new(contexts))
}

/// check whether all configurations from `family` are allowed by `constraint`
pub(crate) fn covers_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    let targets = constraint.configurations_at_degree(family.degree());
    if family.degree() == 0 {
        return Ok(!targets.is_empty());
    }
    if family.degree() == 1 {
        let mut allowed = LabelSet::default();
        for target in targets {
            cancellation.check()?;
            allowed = allowed.union(target.parts()[0].labels());
        }
        return Ok(family.parts()[0].labels().is_subset_of(&allowed));
    }
    for target in targets {
        if contains_condensed_with(target, family, cancellation)? {
            return Ok(true);
        }
    }
    // Try counted, nearly homogeneous witnesses before enumerating mixtures.
    // For example, a target requiring A rejects B^n immediately, without
    // traversing all distributions containing A first. These probes can only
    // disprove coverage; successful probes still require the exact fallback.
    let labels: std::collections::BTreeSet<_> = family
        .parts()
        .iter()
        .flat_map(|part| part.labels().iter())
        .collect();
    for preferred in labels {
        cancellation.check()?;
        let concrete = Configuration::new(
            family
                .parts()
                .iter()
                .map(|part| {
                    let label = if part.labels().contains(preferred) {
                        preferred
                    } else {
                        part.labels().iter().next().expect("nonempty part")
                    };
                    (label, part.multiplicity())
                })
                .collect(),
        )
        .expect("family degree already checked");
        if !accepts_with(constraint, &concrete, cancellation)? {
            return Ok(false);
        }
    }
    visit_multisets_with(family, cancellation, |concrete, _| {
        accepts_with(constraint, concrete, cancellation)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn family(parts: &[(Vec<u8>, u32)]) -> CondensedConfiguration<u8> {
        CondensedConfiguration::new(
            parts
                .iter()
                .map(|(labels, count)| Part::new(LabelSet::new(labels.clone()), *count).unwrap())
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn coverage_can_use_multiple_rows_and_contexts_remove_one_occurrence() {
        let cancellation = Cancellation::default();
        let target = Constraint::new(vec![
            family(&[(vec![0], 2)]),
            family(&[(vec![0], 1), (vec![1], 1)]),
            family(&[(vec![1], 2)]),
        ]);
        assert!(covers_with(&target, &family(&[(vec![0, 1], 2)]), &cancellation).unwrap());
        let source = Constraint::new(vec![family(&[(vec![0, 1], 1_000_000)])]);
        let residual = contexts_with(&source, 0, &cancellation).unwrap();
        assert_eq!(
            residual.configurations_at_degree(999_999)[0].parts()[0].multiplicity(),
            999_999
        );
        assert!(
            covers_with(
                &Constraint::new(vec![family(&[(vec![0], 1)]), family(&[(vec![1], 1)])]),
                &family(&[(vec![0, 1], 1)]),
                &cancellation
            )
            .unwrap()
        );
        assert!(!covers_with(&Constraint::new(vec![]), &family(&[]), &cancellation).unwrap());
        cancellation.cancel();
        assert!(contexts_with(&source, 0, &cancellation).is_err());
    }
}
