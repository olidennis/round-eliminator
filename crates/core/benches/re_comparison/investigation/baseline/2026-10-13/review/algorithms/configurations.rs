//! LLM: Multiset enumeration, concrete membership, and condensed containment.

use std::collections::BTreeSet;

use crate::computation::{Cancellation, Cancelled};
use crate::model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part};

mod alignments;
mod coverage;
mod matching;
mod product;
mod substitution;
pub(crate) use alignments::{visit_alignments_avoiding_with, visit_alignments_with};
pub(crate) use coverage::{contexts_with, covers_with};
pub(crate) use matching::matches_counts_with;
pub(crate) use product::product_with;
pub(crate) use substitution::map_parts_with;

/// LLM: Decide whether a concrete multiset belongs to a constraint. Used by callers that do
/// LLM: not need to supply cancellation explicitly.
pub fn accepts<L: Copy + Ord>(constraint: &Constraint<L>, concrete: &Configuration<L>) -> bool {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    accepts_with(constraint, concrete, &Cancellation::default()).expect("not cancelled")
}

/// LLM: Decide concrete membership with cooperative cancellation, checking same-degree
/// LLM: condensed rows through counted matching. Used by verification and exact searches.
pub(crate) fn accepts_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    // LLM: we go through all condensed configuration and check for each of them

    for configuration in constraint.configurations_at_degree(concrete.degree()) {
        cancellation.check()?;
        if matches(configuration, concrete, cancellation)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// LLM: Match a concrete multiset to one condensed row without expanding repeated occurrences.
/// LLM: Used by constraint membership to enforce part capacities.
fn matches<L: Copy + Ord>(
    configuration: &CondensedConfiguration<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Read the target part counts and reject cheap impossible membership cases before
    // LLM: constructing flow.
    let parts = configuration.parts();
    // LLM: Degree 0, easy

    if concrete.degree() == 0 {
        return Ok(true);
    }
    // LLM: the concrete configuration contains a single label many times, easy check

    if let [(label, _)] = concrete.multiplicities() {
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for part in parts {
            cancellation.check()?;
            if !part.labels().contains(*label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // LLM: the target condensed configuration has a single set repeated many tines, easy check

    if let [part] = parts {
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for &(label, _) in concrete.multiplicities() {
            cancellation.check()?;
            if !part.labels().contains(label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    // LLM: Describe target choice domains and their capacities for exact counted membership
    // LLM: matching.
    let allowed: Vec<_> = parts
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    // LLM: Use exact counted matching so repeated occurrences do not need a port-permutation
    // LLM: expansion.
    matches_counts_with(
        concrete.multiplicities(),
        &allowed,
        |label, set| set.contains(*label),
        cancellation,
    )
}

/// LLM: Prove that one target row contains an entire condensed family through count-capacity
/// LLM: matching of subset-compatible parts. Used by maximization, diagram coverage, and
/// LLM: passive verification; this is single-row domination.
pub(crate) fn contains_condensed_with<L: Copy + Ord>(
    target: &CondensedConfiguration<L>,
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    match_parts_with(
        target,
        family,
        |offered, allowed| offered.is_subset_of(allowed),
        cancellation,
    )
}

/// LLM: Decide whether independent per-port label domains admit some configuration in a
/// LLM: constraint. Used by strategy verification and active SAT refinement; the positions are
/// LLM: choices, not a concrete multiset.
pub(crate) fn allows_positions_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    positions: &[LabelSet<L>],
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    if positions.iter().any(LabelSet::is_empty) {
        return Ok(false);
    }
    // LLM: Represent each position as one independent choice-set part for existential
    // LLM: matching.
    let family = CondensedConfiguration::new(
        positions
            .iter()
            .cloned()
            .map(|labels| Part::new(labels, 1).expect("nonempty choices"))
            .collect(),
    )
    .expect("degree fits u32");
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for target in constraint.configurations_at_degree(family.degree()) {
        // LLM: Accept as soon as one allowed row can accommodate all position domains.
        if match_parts_with(
            target,
            &family,
            |offered, allowed| !offered.intersection(allowed).is_empty(),
            cancellation,
        )? {
            return Ok(true);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(false)
}

/// LLM: Match counted parts using a caller-supplied compatibility relation. Used by condensed
/// LLM: containment and generic configuration comparisons.
fn match_parts_with<L: Copy + Ord>(
    target: &CondensedConfiguration<L>,
    family: &CondensedConfiguration<L>,
    compatible: impl Fn(&LabelSet<L>, &LabelSet<L>) -> bool,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Extract the offered part values and counts without expanding their occurrences.
    let offered: Vec<_> = family
        .parts()
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    // LLM: Extract target capacities and pass the caller-defined compatibility relation to
    // LLM: exact flow matching.
    let allowed: Vec<_> = target
        .parts()
        .iter()
        .map(|part| (part.labels(), part.multiplicity()))
        .collect();
    matches_counts_with(&offered, &allowed, |a, b| compatible(a, b), cancellation)
}

/// LLM: Regression test: verify that concrete multisets. Used by the test suite to protect
/// LLM: this behavior while implementations change.
#[cfg(test)]
pub(crate) fn concrete_multisets<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
) -> Vec<Configuration<L>> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    concrete_multisets_with(constraint, degrees, &Cancellation::default()).expect("not cancelled")
}

/// LLM: Enumerate distinct concrete multisets at the declared degrees with cancellation. Used
/// LLM: to prepare explicit active input contexts while preserving counted output storage.
pub(crate) fn concrete_multisets_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
    cancellation: &Cancellation,
) -> Result<Vec<Configuration<L>>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let mut result = BTreeSet::new();
    // LLM: Explore the next counted alternative while preserving the required total
    // LLM: capacities.
    for &degree in degrees {
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        for configuration in constraint.configurations_at_degree(degree) {
            // LLM: Start the Cartesian expansion from the empty multiset, deduplicating after
            // LLM: each counted part.
            let mut prefixes = BTreeSet::from([Configuration::default()]);
            for part in configuration.parts() {
                // LLM: Enumerate distributions over this part and combine them with each
                // LLM: distinct prefix.
                let choices: Vec<_> = part.labels().iter().collect();
                let mut next = BTreeSet::new();
                // LLM: Explore the next counted alternative while preserving the required
                // LLM: total capacities.
                for prefix in &prefixes {
                    // LLM: Enumerate count distributions, using one counter per
                    // LLM: distinct alternative rather than per repeated occurrence.

                    let mut counts = vec![0u32; choices.len()];
                    counts[0] = part.multiplicity();
                    // LLM: Explore the next counted alternative while preserving the required
                    // LLM: total capacities.
                    loop {
                        // LLM: Observe cooperative interruption before continuing this unit of
                        // LLM: work.
                        cancellation.check()?;
                        let mut entries = prefix.multiplicities().to_vec();
                        entries.extend(choices.iter().copied().zip(counts.iter().copied()));
                        next.insert(
                            Configuration::new(entries).expect("expanded degree was checked"),
                        );
                        // LLM: Reject an infeasible capacity state before exploring its
                        // LLM: completions.
                        if !advance_counts(&mut counts) {
                            break;
                        }
                    }
                }
                // LLM: Use the distinct combined rows as prefixes for the next part.
                prefixes = next;
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            result.extend(prefixes);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(result.into_iter().collect())
}

/// LLM: Advance a fixed-total label-count distribution without generating occurrence
/// LLM: permutations. Used by multiset traversal and passive output counterexample
/// LLM: enumeration.
pub(crate) fn advance_counts(counts: &mut [u32]) -> bool {
    // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
    // LLM: traversal to backtrack or continue.
    let Some(position) = counts[..counts.len() - 1]
        .iter()
        .rposition(|&count| count != 0)
    else {
        return false;
    };
    counts[position] -= 1;
    let remainder = counts[position + 1..].iter().sum::<u32>() + 1;
    // LLM: Carry to the next count distribution and reset the later counters while preserving
    // LLM: the total.
    counts[position + 1..].fill(0);
    counts[position + 1] = remainder;
    true
}

/// LLM: Visit each distinct concrete choice of a condensed row and its per-part label counts.
/// LLM: The visitor receives the resulting multiset and `counts[part][choice]` provenance;
/// LLM: used by input enumeration and exact coverage.
pub(crate) fn visit_multisets_with<L: Copy + Ord>(
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
    mut visitor: impl FnMut(&Configuration<L>, &[Vec<u32>]) -> Result<bool, Cancelled>,
) -> Result<bool, Cancelled> {
    // LLM: Prepare each part's label order and one fixed-total count distribution per part.
    let choices: Vec<Vec<L>> = family
        .parts()
        .iter()
        .map(|part| part.labels().iter().collect())
        .collect();
    // LLM: Keep repetitions as count distributions instead of expanding permutations of
    // LLM: occurrences.
    let mut counts: Vec<Vec<u32>> = family
        .parts()
        .iter()
        .map(|part| {
            let mut counts = vec![0; part.labels().len()];
            counts[0] = part.multiplicity();
            counts
        })
        .collect();
    // LLM: Allocate visited rows only when distinct part distributions may coincide as one
    // LLM: concrete multiset.
    let mut visited = BTreeSet::new();
    loop {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let concrete = Configuration::new(
            choices
                .iter()
                .zip(&counts)
                .flat_map(|(labels, counts)| labels.iter().copied().zip(counts.iter().copied()))
                .collect(),
        )
        .expect("family degree already checked");
        // LLM: Reject an infeasible capacity state before exploring its completions.
        if visited.insert(concrete.clone()) && !visitor(&concrete, &counts)? {
            return Ok(false);
        }
        // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
        // LLM: traversal to backtrack or continue.
        let Some(position) = (0..counts.len())
            .rev()
            .find(|&index| advance_counts(&mut counts[index]))
        else {
            break;
        };
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        for (index, counts) in counts.iter_mut().enumerate().skip(position + 1) {
            counts.fill(0);
            counts[0] = family.parts()[index].multiplicity();
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LabelSet;
    use crate::model::Part;

    /// LLM: Regression test: verify that condensed containment uses set inclusion and counted
    /// LLM: capacities. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn condensed_containment_uses_set_inclusion_and_counted_capacities() {
        // LLM: Prepare `configuration` for the following computation; it stores the
        // LLM: configuration for this stage.
        let configuration = |parts: Vec<(Vec<u8>, u32)>| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            CondensedConfiguration::new(
                parts
                    .into_iter()
                    .map(|(labels, count)| Part::new(LabelSet::new(labels), count).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        // LLM: Prepare `target` for the following computation; it stores the target for this
        // LLM: stage.
        let target = configuration(vec![(vec![0, 1], 2), (vec![2], 2)]);
        let matching = configuration(vec![(vec![0], 1), (vec![1], 1), (vec![2], 2)]);
        let competing = configuration(vec![(vec![0], 2), (vec![1], 1), (vec![2], 1)]);
        let cancellation = Cancellation::default();
        assert!(contains_condensed_with(&target, &matching, &cancellation).unwrap());
        assert!(!contains_condensed_with(&target, &competing, &cancellation).unwrap());
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            !contains_condensed_with(&target, &configuration(vec![(vec![0], 3)]), &cancellation)
                .unwrap()
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            !contains_condensed_with(
                &target,
                &configuration(vec![(vec![0, 1, 2], 4)]),
                &cancellation
            )
            .unwrap()
        );
        // LLM: Prepare `empty` for the following computation; it stores the empty for this
        // LLM: stage.
        let empty = CondensedConfiguration::<u8>::new(vec![]).unwrap();
        assert!(contains_condensed_with(&empty, &empty, &cancellation).unwrap());

        // LLM: Prepare `target` for the following computation; it stores the target for this
        // LLM: stage.
        let target = configuration(vec![(vec![0, 1], 1_000_000), (vec![1, 2], 1_000_000)]);
        let family = configuration(vec![(vec![0], 1_000_000), (vec![2], 1_000_000)]);
        assert!(contains_condensed_with(&target, &family, &cancellation).unwrap());
        cancellation.cancel();
        assert!(contains_condensed_with(&target, &family, &cancellation).is_err());
    }

    /// LLM: Regression test: verify that large multiplicities stay counted in membership and
    /// LLM: enumeration. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn large_multiplicities_stay_counted_in_membership_and_enumeration() {
        // LLM: Prepare `part` for the following computation; it stores the part for this
        // LLM: stage.
        let part = |labels: Vec<u8>, count| Part::new(LabelSet::new(labels), count).unwrap();
        let constraint = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0, 1], u32::MAX - 1), part(vec![0], 1)])
                .unwrap(),
        ]);
        let concrete = Configuration::new(vec![(0, 1), (1, u32::MAX - 1)]).unwrap();
        assert!(accepts(&constraint, &concrete));
        // LLM: Prepare `rejected` for the following computation; it stores the rejected for
        // LLM: this stage.
        let rejected = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0], u32::MAX - 1), part(vec![1], 1)])
                .unwrap(),
        ]);
        assert!(!accepts(&rejected, &concrete));

        // LLM: Prepare `singleton` for the following computation; it stores the singleton for
        // LLM: this stage.
        let singleton = Constraint::new(vec![
            CondensedConfiguration::new(vec![part(vec![0], u32::MAX)]).unwrap(),
        ]);
        assert_eq!(
            concrete_multisets(&singleton, &[u32::MAX]),
            vec![Configuration::new(vec![(0, u32::MAX)]).unwrap(),]
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(accepts(
            &singleton,
            &Configuration::new(vec![(0, u32::MAX)]).unwrap()
        ));
    }

    /// LLM: Regression test: verify that membership and enumeration respect cancellation. Used
    /// LLM: by the test suite to protect this behavior while implementations change.
    #[test]
    fn membership_and_enumeration_respect_cancellation() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let constraint = Constraint::<u8>::new(vec![]);
        assert!(accepts_with(&constraint, &Configuration::default(), &cancellation).is_err());
        assert!(concrete_multisets_with(&constraint, &[0], &cancellation).is_err());
    }

    /// LLM: Regression test: verify that membership agrees with explicit expansion. Used by
    /// LLM: the test suite to protect this behavior while implementations change.
    #[test]
    fn membership_agrees_with_explicit_expansion() {
        // LLM: Exercise overlapping parts, repeated counts, and the augmenting paths
        // LLM: needed when a greedy occurrence-to-part assignment gets stuck.

        for first in 1u8..8 {
            // LLM: Process the relevant label choices while preserving their table identities.
            for second in 1u8..8 {
                // LLM: Process the relevant label choices while preserving their table
                // LLM: identities.
                for first_count in 1..=2 {
                    // LLM: Process the relevant label choices while preserving their table
                    // LLM: identities.
                    for second_count in 1..=2 {
                        // LLM: Prepare `set` for the following computation; it stores the set
                        // LLM: for this stage.
                        let set = |mask| {
                            LabelSet::new(
                                (0u8..3).filter(|label| mask & (1 << label) != 0).collect(),
                            )
                        };
                        // LLM: Prepare `configuration` for the following computation; it
                        // LLM: stores the configuration for this stage.
                        let configuration = CondensedConfiguration::new(vec![
                            Part::new(set(first), first_count).unwrap(),
                            Part::new(set(second), second_count).unwrap(),
                        ])
                        .unwrap();
                        let constraint = Constraint::new(vec![configuration]);
                        let degree = first_count + second_count;
                        let mut expanded = BTreeSet::new();
                        // LLM: Process the relevant label choices while preserving their table
                        // LLM: identities.
                        for mut code in 0usize..3usize.pow(degree) {
                            // LLM: Prepare `tuple` for the following computation; it stores
                            // LLM: the tuple for this stage.
                            let mut tuple = Vec::new();
                            let mut allowed = true;
                            // LLM: Process the relevant label choices while preserving their
                            // LLM: table identities.
                            for position in 0..degree {
                                // LLM: Prepare `label` for the following computation; it
                                // LLM: stores the label for this stage.
                                let label = (code % 3) as u8;
                                code /= 3;
                                let mask = if position < first_count {
                                    first
                                } else {
                                    second
                                };
                                allowed &= mask & (1 << label) != 0;
                                // LLM: Retain the newly constructed choices in the result for
                                // LLM: this stage.
                                tuple.push(label);
                            }
                            // LLM: Handle this condition before continuing with the remaining
                            // LLM: search alternatives.
                            if allowed {
                                tuple.sort_unstable();
                                expanded.insert(tuple);
                            }
                        }
                        // LLM: Check the expected result and invariants for this fixture,
                        // LLM: including the relevant edge case.
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
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for mut code in 0usize..3usize.pow(degree) {
                            // LLM: Prepare `tuple` for the following computation; it stores
                            // LLM: the tuple for this stage.
                            let mut tuple = Vec::new();
                            for _ in 0..degree {
                                tuple.push((code % 3) as u8);
                                code /= 3;
                            }
                            // LLM: Evaluate the implementation under test on the same fixture.
                            let actual = accepts(
                                &constraint,
                                &Configuration::from_occurrences(tuple.iter().copied()).unwrap(),
                            );
                            tuple.sort_unstable();
                            assert_eq!(actual, expanded.contains(&tuple));
                        }
                        // LLM: Check the expected result and invariants for this fixture,
                        // LLM: including the relevant edge case.
                        assert!(!accepts(
                            &constraint,
                            &Configuration::new(vec![(0, degree + 1)]).unwrap()
                        ));
                    }
                }
            }
        }
    }

    /// LLM: Regression test: verify that zero degree and duplicate configurations. Used by the
    /// LLM: test suite to protect this behavior while implementations change.
    #[test]
    fn zero_degree_and_duplicate_configurations() {
        // LLM: Prepare `empty` for the following computation; it stores the empty for this
        // LLM: stage.
        let empty = CondensedConfiguration::<u8>::new(vec![]).unwrap();
        let constraint = Constraint::new(vec![empty.clone(), empty]);
        assert!(accepts(&constraint, &Configuration::default()));
        assert_eq!(
            concrete_multisets(&constraint, &[0]),
            vec![Configuration::<u8>::default()]
        );
        assert!(concrete_multisets(&constraint, &[1]).is_empty());
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(!accepts(
            &Constraint::<u8>::new(vec![]),
            &Configuration::default()
        ));
    }
}
