//! LLM: Benchmark-only wider fixed/dynamic word sets at the algorithm boundary.

use super::engine::{Operations, maximize_with_minimum};
use crate::{computation::Computation, model::{Configuration, Constraint, LabelSet}};
use std::{collections::BTreeSet, sync::Arc};

/// LLM: A fixed four-word bit mask representing a dense alphabet in the maximization
/// LLM: experiment or implementation. Bit positions refer to its private local alphabet, not
/// LLM: necessarily the original numeric label IDs.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct FourWords([u64; 4]);
impl Ord for FourWords {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // LLM: Compare packed values in deterministic numeric word order, most significant
        // LLM: word first.
        self.0.iter().rev().cmp(other.0.iter().rev()) }
}
impl PartialOrd for FourWords {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // LLM: Compare packed values in deterministic numeric word order, most significant
        // LLM: word first.
        Some(self.cmp(other)) }
}
/// LLM: The join, meet, containment, and nonempty checks for fixed four-word masks used by the
/// LLM: generic maximization engine.
struct FourOperations;
impl Operations<FourWords> for FourOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &FourWords, b: &FourWords) -> FourWords {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        FourWords(std::array::from_fn(|i| a.0[i] | b.0[i]))
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &FourWords, b: &FourWords) -> FourWords {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        FourWords(std::array::from_fn(|i| a.0[i] & b.0[i]))
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &FourWords, b: &FourWords) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().zip(b.0).all(|(&a, b)| b & !a == 0)
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, a: &FourWords) -> bool {
        // LLM: Check whether the value is permitted by this algebra's bottom/empty policy.
        a.0.iter().any(|&w| w != 0) }
}

/// LLM: An owned multiword bit mask for a dense local alphabet. Payload sharing and bitwise
/// LLM: operations are private to maximization and do not change the public label-set
/// LLM: representation.
#[derive(Clone, Eq, PartialEq)]
struct Words(Arc<[u64]>);
impl Ord for Words {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // LLM: Compare packed values in deterministic numeric word order, most significant
        // LLM: word first.
        self.0.iter().rev().cmp(other.0.iter().rev()) }
}
impl PartialOrd for Words {
    /// LLM: Compare packed values in deterministic numeric word order, most significant word
    /// LLM: first. Used for canonical sorting/history keys; semantic containment still uses
    /// LLM: the set algebra.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // LLM: Compare packed values in deterministic numeric word order, most significant
        // LLM: word first.
        Some(self.cmp(other)) }
}
/// LLM: The set algebra for multiword masks, including the policy that excludes empty output
/// LLM: choices during ordinary maximization.
struct WordOperations;
impl Operations<Words> for WordOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &Words, b: &Words) -> Words {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        Words(a.0.iter().zip(b.0.iter()).map(|(&a,&b)| a | b).collect())
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &Words, b: &Words) -> Words {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        Words(a.0.iter().zip(b.0.iter()).map(|(&a,&b)| a & b).collect())
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &Words, b: &Words) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.0.iter().zip(b.0.iter()).all(|(&a,&b)| b & !a == 0)
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, a: &Words) -> bool {
        // LLM: Check whether the value is permitted by this algebra's bottom/empty policy.
        a.0.iter().any(|&w| w != 0) }
}

/// LLM: Choose a suitable private packed-set representation and compute an ordinary constraint
/// LLM: maximum. Used by the public maximizer; sparse alphabets can retain vector sets without
/// LLM: restricting input size.
pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>, computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    // LLM: Prepare the local alphabet shared by the representation conversion.
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() { computation.cancellation.check()?; alphabet.insert(label); }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if crate::four_flags::enabled("fixed") {
        if alphabet.len() > 256 { return Ok(None); }
        maximum(constraint, &alphabet, &FourOperations,
            |labels| { let mut words = [0;4]; for &i in labels { words[i/64] |= 1 << (i%64); } FourWords(words) },
            |words, i| words.0[i/64] & (1 << (i%64)) != 0, computation).map(Some)
    } else {
        maximum(constraint, &alphabet, &WordOperations,
            |labels| { let mut words = vec![0; alphabet.len().div_ceil(64)]; for &i in labels { words[i/64] |= 1 << (i%64); } Words(words.into()) },
            |words, i| words.0[i/64] & (1 << (i%64)) != 0, computation).map(Some)
    }
}

/// LLM: Compute a maximum using the experimental packed-set selection policy. Used by the
/// LLM: retained representation comparison, not the production API.
fn maximum<L: Copy + Ord, M: Clone + Ord + Send + Sync>(
    constraint: &Constraint<L>, alphabet: &[L], operations: &impl Operations<M>,
    encode: impl Fn(&[usize]) -> M, contains: impl Fn(&M, usize) -> bool,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelSet<L>>>> {
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let parts = row.parts().iter().map(|part| {
            let indices: Vec<_> = part.labels().iter().map(|label| alphabet.binary_search(&label).unwrap()).collect();
            (encode(&indices), part.multiplicity())
        }).collect();
        initial.push(Configuration::new(parts)?);
    }
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = maximize_with_minimum(&initial, operations, computation, None, 256)?;
    let mut decoded = Vec::new();
    for row in maximum {
        computation.cancellation.check()?;
        decoded.push(Configuration::new(row.multiplicities().iter().map(|(mask,count)| {
            (LabelSet::new(alphabet.iter().enumerate().filter_map(|(i,&label)| contains(mask,i).then_some(label)).collect()), *count)
        }).collect())?);
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    decoded.sort_by(|a,b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(decoded)
}
