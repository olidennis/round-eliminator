//! LLM: Pairwise closure of counted configurations under custom union/intersection.
//! LLM: Values can be label sets or elements of another finite ordered algebra.
//! LLM: Counts avoid expanding repeated occurrences. Complete results are antichains
//! LLM: closed under one union and intersections at all remaining occurrences.

use crate::{
    algorithms::{
        configurations::{matches_counts_with, visit_alignments_with},
        parallel::map_with_progress,
    },
    computation::{Cancellation, Cancelled, Computation, Phase},
    model::{Configuration, LabelSet},
};
use std::collections::{BTreeMap, BTreeSet};

/// LLM: The ordered algebra required by the generic closure engine: stable monotone join,
/// LLM: meet, containment, and upward-closed admissibility. These laws make dominance pruning
/// LLM: sound; arbitrary unrelated callbacks would not suffice.
pub trait Operations<T>: Sync {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, left: &T, right: &T) -> T;
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, left: &T, right: &T) -> T;
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, left: &T, right: &T) -> bool;

    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, _value: &T) -> bool {
        // LLM: Return the result of this completed search or conversion to its caller.
        true
    }
}

/// LLM: The ordinary label-set algebra and the local policy for admitting the empty set.
/// LLM: Standard maximization excludes empty choices; custom fixed-point closures may retain
/// LLM: them.
pub struct SetOperations {
    pub allow_empty: bool,
}
impl<L: Copy + Ord> Operations<LabelSet<L>> for SetOperations {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> LabelSet<L> {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.union(b)
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> LabelSet<L> {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        a.intersection(b)
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        b.is_subset_of(a)
    }
    /// LLM: Check whether the value is permitted by this algebra's bottom/empty policy. Called
    /// LLM: by the generic pairwise closure and its pruning proofs.
    fn is_admissible(&self, value: &LabelSet<L>) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.allow_empty || !value.is_empty()
    }
}

/// LLM: A recorded pairwise derivation: parent configurations, the chosen union positions, and
/// LLM: row-major counts for remaining intersections. The indices refer to the parents' sorted
/// LLM: distinct values.
pub struct Combination<T> {
    pub left: Configuration<T>,
    pub right: Configuration<T>,
    pub union_parts: (usize, usize),
    pub intersection_counts: Vec<u32>,
}

/// LLM: The optional callback invoked on the requester when a derived configuration is
/// LLM: admitted. Its parents and alignment describe the derivation; later dominance pruning
/// LLM: can still remove the admitted row.
pub type DerivationObserver<'a, T> = dyn FnMut(&Configuration<T>, Combination<T>) + 'a;

/// LLM: A generated counted configuration and its optional derivation. The closure filters
/// LLM: duplicate and dominated candidates before admission; a generated candidate need not
/// LLM: survive in the final maximum.
struct Candidate<T> {
    configuration: Configuration<T>,
    combination: Option<Combination<T>>,
}

/// LLM: Check whether a target counted row contains another under the supplied algebra using
/// LLM: exact capacity matching. Used throughout antichain rejection and dominance pruning.
fn dominates<T>(
    target: &Configuration<T>,
    family: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Use exact counted matching so repeated occurrences do not need a port-permutation
    // LLM: expansion.
    matches_counts_with(
        family.multiplicities(),
        target.multiplicities(),
        |a, b| operations.is_superset(b, a),
        cancellation,
    )
}

/// LLM: Admit an undominated row and remove weaker retained rows. Used by the frozen benchmark
/// LLM: closure's serial antichain implementation.
fn insert<T: Clone + Ord>(
    current: &mut Vec<Configuration<T>>,
    candidate: Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    for old in current.iter() {
        if dominates(old, &candidate, operations, cancellation)? {
            return Ok(false);
        }
    }
    let mut kept = Vec::with_capacity(current.len() + 1);
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    for old in current.drain(..) {
        if !dominates(&candidate, &old, operations, cancellation)? {
            kept.push(old)
        }
    }
    kept.push(candidate);
    *current = kept;
    Ok(true)
}

/// LLM: Compute the full undominated pairwise closure under a supplied ordered algebra. Used
/// LLM: by ordinary maximization and Basic FP; degrees remain separate, and interruption
/// LLM: returns no maximum.
pub fn maximize_custom_with<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    mut observe: Option<&mut DerivationObserver<'_, T>>,
) -> Result<Vec<Configuration<T>>, Cancelled> {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = computation.cancellation.clone();
    let mut degrees: BTreeMap<u32, Vec<Configuration<T>>> = BTreeMap::new();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for configuration in initial {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if configuration
            .multiplicities()
            .iter()
            .all(|(v, _)| operations.is_admissible(v))
        {
            degrees
                .entry(configuration.degree())
                .or_default()
                .push(configuration.clone());
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Keep the generation grain local to this degree so scheduling adapts without sharing
    // LLM: mutable worker state.
    let batch_size = rayon::current_num_threads().saturating_mul(2).max(1);
    #[cfg(target_arch = "wasm32")]
    // LLM: Keep the generation grain local to this degree so scheduling adapts without sharing
    // LLM: mutable worker state.
    let batch_size = 1;
    let tracking = observe.is_some();
    let mut all = Vec::new();
    let mut completed = 0u32;
    computation.report(Phase::Maximizing, completed)?;
    // LLM: Check that computed indices and counts fit the model's integer representation.
    for seeds in degrees.values_mut() {
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        seeds.sort();
        let mut current = Vec::new();
        for seed in seeds.drain(..) {
            insert(&mut current, seed, operations, &cancellation)?;
        }
        current.sort();
        let mut seen: BTreeSet<_> = current.iter().cloned().collect();
        let mut previous = BTreeSet::new();
        // LLM: Check that computed indices and counts fit the model's integer representation.
        loop {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let snapshot = current.clone();
            let mut pairs = Vec::with_capacity(batch_size);
            let mut changed = false;
            // LLM: Queue only pairs with a newly admitted parent, including self-pairs.
            // LLM: Bounded batches cap temporary results and deterministic merging avoids
            // LLM: a concurrent mutable antichain or per-request thread pool.

            let mut process = |pairs: &mut Vec<(usize, usize)>| -> Result<(), Cancelled> {
                // LLM: On the baseline fixtures, dispatching tiny pairs cost hundreds
                // LLM: of microseconds versus 4–15 us for the entire serial closure.
                // LLM: Estimate count-matrix freedom to reserve workers for larger
                // LLM: batches. This is scheduling only, never a search cutoff.

                let work = pairs.iter().fold(0usize, |sum, &(a, b)| {
                    // LLM: Prepare `dimensions` for the following computation; it stores the
                    // LLM: dimensions for this stage.
                    let dimensions = snapshot[a]
                        .multiplicities()
                        .len()
                        .saturating_sub(1)
                        .saturating_mul(snapshot[b].multiplicities().len().saturating_sub(1));
                    // LLM: Check that computed indices and counts fit the model's integer
                    // LLM: representation.
                    sum.saturating_add(
                        (snapshot[a].degree() as usize)
                            .saturating_add(1)
                            .saturating_pow(u32::try_from(dimensions).unwrap_or(u32::MAX)),
                    )
                });
                // LLM: Generate consequences of the selected parent pair under one join and
                // LLM: the remaining meets.
                let results = map_with_progress(
                    pairs.len(),
                    &cancellation,
                    work >= 20_000,
                    |index| {
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        let (a, b) = pairs[index];
                        combine(
                            &snapshot[a],
                            &snapshot[b],
                            operations,
                            &cancellation,
                            tracking,
                        )
                    },
                    |_| {
                        completed = completed.saturating_add(1);
                        computation.report(Phase::Maximizing, completed)
                    },
                )?;
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for candidates in results {
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for candidate in candidates {
                        // LLM: Observe cooperative interruption before continuing this unit of
                        // LLM: work.
                        cancellation.check()?;
                        if !seen.insert(candidate.configuration.clone()) {
                            continue;
                        }
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if insert(
                            &mut current,
                            candidate.configuration.clone(),
                            operations,
                            &cancellation,
                        )? {
                            // LLM: Update the current working state before continuing this
                            // LLM: operation.
                            changed = true;
                            if let (Some(observer), Some(combination)) =
                                (observe.as_mut(), candidate.combination)
                            {
                                observer(&candidate.configuration, combination);
                            }
                        }
                    }
                }
                // LLM: Update the current working state before continuing this operation.
                pairs.clear();
                Ok(())
            };
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for a in 0..snapshot.len() {
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for b in 0..=a {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    if previous.contains(&snapshot[a]) && previous.contains(&snapshot[b]) {
                        continue;
                    }
                    pairs.push((a, b));
                    if pairs.len() == batch_size {
                        process(&mut pairs)?;
                    }
                }
            }
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if !pairs.is_empty() {
                process(&mut pairs)?;
            }
            if !changed {
                break;
            }
            previous = snapshot.into_iter().collect();
            current.sort();
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        all.extend(current);
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    all.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(all)
}

/// LLM: Generate undominated consequences of two same-degree parents by selecting one join and
/// LLM: meeting the remaining counted occurrences. Used by each closure generation pass;
/// LLM: duplicate history and optional derivations are retained.
fn combine<T: Clone + Ord>(
    left: &Configuration<T>,
    right: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    tracking: bool,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    // LLM: Prepare `a` for the following computation; it stores the a for this stage.
    let a = left.multiplicities();
    let b = right.multiplicities();
    // LLM: Each intersection is computed once for all union choices and alignments.

    let mut intersections = Vec::with_capacity(a.len() * b.len());
    for (x, _) in a {
        for (y, _) in b {
            cancellation.check()?;
            intersections.push(operations.intersection(x, y));
        }
    }
    // LLM: Record the admissible pairings used to restrict exact matching or support search.
    let compatible: Vec<_> = intersections
        .iter()
        .map(|v| operations.is_admissible(v))
        .collect();
    let mut result: Vec<Candidate<T>> = Vec::new();
    let mut seen = BTreeSet::new();
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    for (i, (x, _)) in a.iter().enumerate() {
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for (j, (y, _)) in b.iter().enumerate() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if operations.is_superset(x, y) || operations.is_superset(y, x) {
                continue;
            }
            let union = operations.union(x, y);
            if !operations.is_admissible(&union) {
                continue;
            }
            // LLM: Prepare counted rows or remaining row capacities for the next exact
            // LLM: comparison.
            let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
            let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
            rows[i] -= 1;
            columns[j] -= 1;
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            visit_alignments_with(&rows, &columns, &compatible, cancellation, |counts| {
                // LLM: Prepare `parts` for the following computation; it stores the parts for
                // LLM: this stage.
                let mut parts = vec![(union.clone(), 1)];
                parts.extend(
                    counts
                        .iter()
                        .zip(&intersections)
                        .filter(|(n, _)| **n > 0)
                        .map(|(&n, value)| (value.clone(), n)),
                );
                // LLM: Prepare `configuration` for the following computation; it stores the
                // LLM: configuration for this stage.
                let configuration =
                    Configuration::new(parts).expect("combination preserves degree");
                if !seen.insert(configuration.clone()) {
                    return Ok(());
                }
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                if dominates(left, &configuration, operations, cancellation)?
                    || dominates(right, &configuration, operations, cancellation)?
                {
                    return Ok(());
                }
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for old in &result {
                    if dominates(&old.configuration, &configuration, operations, cancellation)? {
                        return Ok(());
                    }
                }
                let mut kept = Vec::new();
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for old in result.drain(..) {
                    if !dominates(&configuration, &old.configuration, operations, cancellation)? {
                        kept.push(old)
                    }
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                kept.push(Candidate {
                    configuration,
                    combination: tracking.then(|| Combination {
                        left: left.clone(),
                        right: right.clone(),
                        union_parts: (i, j),
                        intersection_counts: counts.to_vec(),
                    }),
                });
                // LLM: Update the current working state before continuing this operation.
                result = kept;
                Ok(())
            })?;
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(result)
}
