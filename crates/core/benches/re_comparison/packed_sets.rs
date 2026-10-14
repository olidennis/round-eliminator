//! LLM: Experimental packed-set adapter; the model and generic closure are unchanged.

use super::*;
use std::collections::BTreeSet;

/// LLM: The bitwise set algebra for one machine-word mask. Nonzero masks are admissible output
/// LLM: choices; local bit positions are decoded to the original alphabet after maximization.
struct PackedOperations;
impl Operations<u64> for PackedOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &u64, b: &u64) -> u64 {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a | b
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &u64, b: &u64) -> u64 {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a & b
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &u64, b: &u64) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        b & !a == 0
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, value: &u64) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        *value != 0
    }
}

/// LLM: Try private packed-mask maximization for the constraint. Used by the public maximizer;
/// LLM: None selects the equivalent vector-set fallback for sparse wide alphabets, rather than
/// LLM: rejecting the problem or limiting its size.
pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    // LLM: Prepare the local alphabet shared by the representation conversion.
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() {
        computation.cancellation.check()?;
        alphabet.insert(label);
        if alphabet.len() > 64 {
            return Ok(None);
        }
    }
    // LLM: Prepare the local alphabet shared by the representation conversion.
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    let mut initial = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for row in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let mut parts = Vec::new();
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for part in row.parts() {
            let mut bits = 0u64;
            for label in part.labels().iter() {
                bits |= 1u64 << alphabet.binary_search(&label).unwrap();
            }
            parts.push((bits, part.multiplicity()));
        }
        initial.push(Configuration::new(parts)?);
    }
    // LLM: Compute exact closure under the selected standard or dual algebra.
    let maximum = maximize_custom_with(&initial, &PackedOperations, computation, None)?;
    let mut decoded = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for row in maximum {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let parts = row
            .multiplicities()
            .iter()
            .map(|&(bits, count)| {
                // LLM: Prepare `labels` for the following computation; it stores the labels
                // LLM: for this stage.
                let labels = alphabet
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &label)| (bits & (1u64 << i) != 0).then_some(label))
                    .collect();
                (LabelSet::new(labels), count)
            })
            .collect();
        // LLM: Retain the newly constructed choices in the result for this stage.
        decoded.push(Configuration::new(parts)?);
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    decoded.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(Some(decoded))
}
