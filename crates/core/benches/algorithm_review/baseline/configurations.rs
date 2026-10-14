//! Multiset enumeration, concrete membership, and condensed containment.

use std::collections::BTreeSet;

use crate::computation::{Cancellation, Cancelled};
use crate::model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part};

mod alignments;
mod coverage;
mod matching;
mod product;
mod substitution;
pub(crate) use alignments::{AlignmentComponents, AlignmentSearch, visit_alignments_with};
pub(crate) use coverage::{contexts_with, covers_with};
pub(crate) use matching::matches_counts_with;
pub(crate) use product::product_with;
pub(crate) use substitution::map_parts_with;

/// Checks whether a constraint allows a configuration
pub fn accepts<L: Copy + Ord>(constraint: &Constraint<L>, concrete: &Configuration<L>) -> bool {
    accepts_with(constraint, concrete, &Cancellation::default()).expect("not cancelled")
}

/// This variant of `accepts` takes a Cancellation explicitly
pub(crate) fn accepts_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    // we go through all condensed configuration and check for each of them
    for configuration in constraint.configurations_at_degree(concrete.degree()) {
        cancellation.check()?;
        if matches(configuration, concrete, cancellation)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// We use a flow algorithm to check if a condensed configuration contains a given concrete configuration
fn matches<L: Copy + Ord>(
    configuration: &CondensedConfiguration<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let parts = configuration.parts();
    // Degree 0, easy
    if concrete.degree() == 0 {
        return Ok(true);
    }
    // the concrete configuration contains a single label many times, easy check
    if let [(label, _)] = concrete.multiplicities() {
        for part in parts {
            cancellation.check()?;
            if !part.labels().contains(*label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // the target condensed configuration has a single set repeated many tines, easy check
    if let [part] = parts {
        for &(label, _) in concrete.multiplicities() {
            cancellation.check()?;
            if !part.labels().contains(label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    let allowed: Vec<_> = parts
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    matches_counts_with(
        concrete.multiplicities(),
        &allowed,
        |label, set| set.contains(*label),
        cancellation,
    )
}

/// checks whether the condensed configuration target dominates the condensed configuration family
pub(crate) fn contains_condensed_with<L: Copy + Ord>(
    target: &CondensedConfiguration<L>,
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    match_parts_with(
        target,
        family,
        |offered, allowed| offered.is_subset_of(allowed),
        cancellation,
    )
}

/// checks whether there exists a choice from `positions` that is allowed by `constraint`
pub(crate) fn allows_positions_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    positions: &[LabelSet<L>],
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    if positions.iter().any(LabelSet::is_empty) {
        return Ok(false);
    }
    let family = CondensedConfiguration::new(
        positions
            .iter()
            .cloned()
            .map(|labels| Part::new(labels, 1).expect("nonempty choices"))
            .collect(),
    )
    .expect("degree fits u32");
    for target in constraint.configurations_at_degree(family.degree()) {
        if match_parts_with(
            target,
            &family,
            |offered, allowed| !offered.is_disjoint(allowed),
            cancellation,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// like contains_condensed_with, but with a custum compatibility relation
fn match_parts_with<L: Copy + Ord>(
    target: &CondensedConfiguration<L>,
    family: &CondensedConfiguration<L>,
    compatible: impl Fn(&LabelSet<L>, &LabelSet<L>) -> bool,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let offered: Vec<_> = family
        .parts()
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    let allowed: Vec<_> = target
        .parts()
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    matches_counts_with(&offered, &allowed, |a, b| compatible(a, b), cancellation)
}

/// Extracts all uncondensed configurations of given degrees from a constraint
#[cfg(test)]
pub(crate) fn concrete_multisets<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
) -> Vec<Configuration<L>> {
    concrete_multisets_with(constraint, degrees, &Cancellation::default()).expect("not cancelled")
}

pub(crate) fn concrete_multisets_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
    cancellation: &Cancellation,
) -> Result<Vec<Configuration<L>>, Cancelled> {
    cancellation.check()?;
    let mut result = BTreeSet::new();
    for &degree in degrees {
        for configuration in constraint.configurations_at_degree(degree) {
            let mut prefixes = BTreeSet::from([Configuration::default()]);
            for part in configuration.parts() {
                let choices: Vec<_> = part.labels().iter().collect();
                let mut next = BTreeSet::new();
                for prefix in &prefixes {
                    // Enumerate count distributions, using one counter per
                    // distinct alternative rather than per repeated occurrence.
                    let mut counts = vec![0u32; choices.len()];
                    counts[0] = part.multiplicity();
                    loop {
                        cancellation.check()?;
                        let mut entries = prefix.multiplicities().to_vec();
                        entries.extend(choices.iter().copied().zip(counts.iter().copied()));
                        next.insert(
                            Configuration::new(entries).expect("expanded degree was checked"),
                        );
                        if !advance_counts(&mut counts) {
                            break;
                        }
                    }
                }
                prefixes = next;
            }
            result.extend(prefixes);
        }
    }
    Ok(result.into_iter().collect())
}

/// Advance a nonempty count vector to the next distribution with the same total.
/// Used by concrete multiset enumeration and searches over output families.
pub(crate) fn advance_counts(counts: &mut [u32]) -> bool {
    let Some(position) = counts[..counts.len() - 1]
        .iter()
        .rposition(|&count| count != 0)
    else {
        return false;
    };
    counts[position] -= 1;
    let remainder = counts[position + 1..].iter().sum::<u32>() + 1;
    counts[position + 1..].fill(0);
    counts[position + 1] = remainder;
    true
}

/// given a condensed configuration, visits all possible choices.
/// it also gives where the labels come from: a vec of vec of labels,
/// value x in position [i][j] means that we use the jth label of the labelset in the ith part for x times
pub(crate) fn visit_multisets_with<L: Copy + Ord>(
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
    mut visitor: impl FnMut(&Configuration<L>, &[Vec<u32>]) -> Result<bool, Cancelled>,
) -> Result<bool, Cancelled> {
    let choices: Vec<Vec<L>> = family
        .parts()
        .iter()
        .map(|part| part.labels().iter().collect())
        .collect();
    let mut counts: Vec<Vec<u32>> = family
        .parts()
        .iter()
        .map(|part| {
            let mut counts = vec![0; part.labels().len()];
            counts[0] = part.multiplicity();
            counts
        })
        .collect();
    let mut visited = BTreeSet::new();
    loop {
        cancellation.check()?;
        let concrete = Configuration::new(
            choices
                .iter()
                .zip(&counts)
                .flat_map(|(labels, counts)| labels.iter().copied().zip(counts.iter().copied()))
                .collect(),
        )
        .expect("family degree already checked");
        if visited.insert(concrete.clone()) && !visitor(&concrete, &counts)? {
            return Ok(false);
        }
        let Some(position) = (0..counts.len())
            .rev()
            .find(|&index| advance_counts(&mut counts[index]))
        else {
            break;
        };
        for (index, counts) in counts.iter_mut().enumerate().skip(position + 1) {
            counts.fill(0);
            counts[0] = family.parts()[index].multiplicity();
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LabelSet;
    use crate::model::Part;

    #[test]
    fn condensed_containment_uses_set_inclusion_and_counted_capacities() {
        let configuration = |parts: Vec<(Vec<u8>, u32)>| {
            CondensedConfiguration::new(
                parts
                    .into_iter()
                    .map(|(labels, count)| Part::new(LabelSet::new(labels), count).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let target = configuration(vec![(vec![0, 1], 2), (vec![2], 2)]);
        let matching = configuration(vec![(vec![0], 1), (vec![1], 1), (vec![2], 2)]);
        let competing = configuration(vec![(vec![0], 2), (vec![1], 1), (vec![2], 1)]);
        let cancellation = Cancellation::default();
        assert!(contains_condensed_with(&target, &matching, &cancellation).unwrap());
        assert!(!contains_condensed_with(&target, &competing, &cancellation).unwrap());
        assert!(
            !contains_condensed_with(&target, &configuration(vec![(vec![0], 3)]), &cancellation)
                .unwrap()
        );
        assert!(
            !contains_condensed_with(
                &target,
                &configuration(vec![(vec![0, 1, 2], 4)]),
                &cancellation
            )
            .unwrap()
        );
        let empty = CondensedConfiguration::<u8>::new(vec![]).unwrap();
        assert!(contains_condensed_with(&empty, &empty, &cancellation).unwrap());

        let target = configuration(vec![(vec![0, 1], 1_000_000), (vec![1, 2], 1_000_000)]);
        let family = configuration(vec![(vec![0], 1_000_000), (vec![2], 1_000_000)]);
        assert!(contains_condensed_with(&target, &family, &cancellation).unwrap());
        cancellation.cancel();
        assert!(contains_condensed_with(&target, &family, &cancellation).is_err());
    }

    #[test]
    fn large_multiplicities_stay_counted_in_membership_and_enumeration() {
        let part = |labels: Vec<u8>, count| Part::new(LabelSet::new(labels), count).unwrap();
        let constraint = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0, 1], u32::MAX - 1), part(vec![0], 1)])
                .unwrap(),
        ]);
        let concrete = Configuration::new(vec![(0, 1), (1, u32::MAX - 1)]).unwrap();
        assert!(accepts(&constraint, &concrete));
        let rejected = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0], u32::MAX - 1), part(vec![1], 1)])
                .unwrap(),
        ]);
        assert!(!accepts(&rejected, &concrete));

        let singleton = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0], u32::MAX)]).unwrap(),
        ]);
        assert_eq!(
            concrete_multisets(&singleton, &[u32::MAX]),
            vec![Configuration::new(vec![(0, u32::MAX)]).unwrap(),]
        );
        assert!(accepts(
            &singleton,
            &Configuration::new(vec![(0, u32::MAX)]).unwrap()
        ));
    }

    #[test]
    fn membership_and_enumeration_respect_cancellation() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let constraint = Constraint::<u8>::new(vec![]);
        assert!(accepts_with(&constraint, &Configuration::default(), &cancellation).is_err());
        assert!(concrete_multisets_with(&constraint, &[0], &cancellation).is_err());
    }

    #[test]
    fn membership_agrees_with_explicit_expansion() {
        // Exercise overlapping parts, repeated counts, and the augmenting paths
        // needed when a greedy occurrence-to-part assignment gets stuck.
        for first in 1u8..8 {
            for second in 1u8..8 {
                for first_count in 1..=2 {
                    for second_count in 1..=2 {
                        let set = |mask| {
                            LabelSet::new(
                                (0u8..3).filter(|label| mask & (1 << label) != 0).collect(),
                            )
                        };
                        let configuration = CondensedConfiguration::new(vec![
                            Part::new(set(first), first_count).unwrap(),
                            Part::new(set(second), second_count).unwrap(),
                        ])
                        .unwrap();
                        let constraint = Constraint::new(vec![configuration]);
                        let degree = first_count + second_count;
                        let mut expanded = BTreeSet::new();
                        for mut code in 0usize..3usize.pow(degree) {
                            let mut tuple = Vec::new();
                            let mut allowed = true;
                            for position in 0..degree {
                                let label = (code % 3) as u8;
                                code /= 3;
                                let mask = if position < first_count {
                                    first
                                } else {
                                    second
                                };
                                allowed &= mask & (1 << label) != 0;
                                tuple.push(label);
                            }
                            if allowed {
                                tuple.sort_unstable();
                                expanded.insert(tuple);
                            }
                        }
                        assert_eq!(
                            concrete_multisets(&constraint, &[degree]),
                            expanded
                                .iter()
                                .map(|occurrences| Configuration::from_occurrences(
                                    occurrences.iter().copied()
                                )
                                .unwrap())
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect::<Vec<_>>()
                        );
                        for mut code in 0usize..3usize.pow(degree) {
                            let mut tuple = Vec::new();
                            for _ in 0..degree {
                                tuple.push((code % 3) as u8);
                                code /= 3;
                            }
                            let actual = accepts(
                                &constraint,
                                &Configuration::from_occurrences(tuple.iter().copied()).unwrap(),
                            );
                            tuple.sort_unstable();
                            assert_eq!(actual, expanded.contains(&tuple));
                        }
                        assert!(!accepts(
                            &constraint,
                            &Configuration::new(vec![(0, degree + 1)]).unwrap()
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn zero_degree_and_duplicate_configurations() {
        let empty = CondensedConfiguration::<u8>::new(vec![]).unwrap();
        let constraint = Constraint::new(vec![empty.clone(), empty]);
        assert!(accepts(&constraint, &Configuration::default()));
        assert_eq!(
            concrete_multisets(&constraint, &[0]),
            vec![Configuration::<u8>::default()]
        );
        assert!(concrete_multisets(&constraint, &[1]).is_empty());
        assert!(!accepts(
            &Constraint::<u8>::new(vec![]),
            &Configuration::default()
        ));
    }
}
