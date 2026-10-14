//! LLM: Residual contexts and inclusion in a union of condensed configurations.

use super::{accepts_with, contains_condensed_with, visit_multisets_with};
use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part},
};

/// LLM: Delete one occurrence of a chosen label from each permitting row and collect the
/// LLM: residual language. Used by exact diagrams to compare passive replacement contexts.
pub(crate) fn contexts_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    label: L,
    cancellation: &Cancellation,
) -> Result<Constraint<L>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let mut contexts = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for family in constraint.configurations() {
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for (index, part) in family.parts().iter().enumerate() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if !part.labels().contains(label) {
                continue;
            }
            // LLM: Delete exactly one occurrence at the selected permitting part, leaving all
            // LLM: other independent choices intact.
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
            // LLM: Retain the newly constructed choices in the result for this stage.
            contexts.push(CondensedConfiguration::new(parts).expect("degree decreases"));
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(contexts))
}

/// LLM: Check whether the union of constraint rows covers every concrete choice of a condensed
/// LLM: family. Used by exact diagrams and coverage proofs when no single row dominates the
/// LLM: family.
pub(crate) fn covers_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let targets = constraint.configurations_at_degree(family.degree());
    if family.degree() == 0 {
        return Ok(!targets.is_empty());
    }
    // LLM: Use containment as the proof or rejection condition before continuing the exact
    // LLM: search.
    if family.degree() == 1 {
        // LLM: For unary families, union all permitted labels; union coverage is then exactly
        // LLM: a subset test.
        let mut allowed = LabelSet::default();
        for target in targets {
            cancellation.check()?;
            allowed = allowed.union(target.parts()[0].labels());
        }
        return Ok(family.parts()[0].labels().is_subset_of(&allowed));
    }
    // LLM: Try a single-row containment proof using counted subset-compatible matching.
    for target in targets {
        if contains_condensed_with(target, family, cancellation)? {
            return Ok(true);
        }
    }
    // LLM: Try counted, nearly homogeneous witnesses before enumerating mixtures.
    // LLM: For example, a target requiring A rejects B^n immediately, without
    // LLM: traversing all distributions containing A first. These probes can only
    // LLM: disprove coverage; successful probes still require the exact fallback.

    let labels: std::collections::BTreeSet<_> = family
        .parts()
        .iter()
        .flat_map(|part| part.labels().iter())
        .collect();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for preferred in labels {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let concrete = Configuration::new(
            family
                .parts()
                .iter()
                .map(|part| {
                    // LLM: Prefer the probe label in each part when available, producing a
                    // LLM: cheap nearly homogeneous candidate.
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
        // LLM: One rejected concrete probe suffices to disprove universal coverage.
        if !accepts_with(constraint, &concrete, cancellation)? {
            return Ok(false);
        }
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    visit_multisets_with(family, cancellation, |concrete, _| {
        accepts_with(constraint, concrete, cancellation)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// LLM: Construct a condensed test family from label domains and their multiplicities.
    /// LLM: Used by small coverage, maximization, or representation fixtures.
    fn family(parts: &[(Vec<u8>, u32)]) -> CondensedConfiguration<u8> {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        CondensedConfiguration::new(
            parts
                .iter()
                .map(|(labels, count)| Part::new(LabelSet::new(labels.clone()), *count).unwrap())
                .collect(),
        )
        .unwrap()
    }

    /// LLM: Regression test: verify that coverage can use multiple rows and contexts remove
    /// LLM: one occurrence. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn coverage_can_use_multiple_rows_and_contexts_remove_one_occurrence() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let target = Constraint::new(vec![
            family(&[(vec![0], 2)]),
            family(&[(vec![0], 1), (vec![1], 1)]),
            family(&[(vec![1], 2)]),
        ]);
        assert!(covers_with(&target, &family(&[(vec![0, 1], 2)]), &cancellation).unwrap());
        let source = Constraint::new(vec![family(&[(vec![0, 1], 1_000_000)])]);
        // LLM: Prepare the residual language obtained by deleting one occurrence of the
        // LLM: compared label.
        let residual = contexts_with(&source, 0, &cancellation).unwrap();
        assert_eq!(
            residual.configurations_at_degree(999_999)[0].parts()[0].multiplicity(),
            999_999
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            covers_with(
                &Constraint::new(vec![family(&[(vec![0], 1)]), family(&[(vec![1], 1)])]),
                &family(&[(vec![0, 1], 1)]),
                &cancellation
            )
            .unwrap()
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(!covers_with(&Constraint::new(vec![]), &family(&[]), &cancellation).unwrap());
        cancellation.cancel();
        assert!(contexts_with(&source, 0, &cancellation).is_err());
    }
}
