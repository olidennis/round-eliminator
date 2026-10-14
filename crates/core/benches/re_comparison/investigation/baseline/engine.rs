//! Pairwise closure of counted configurations under custom union/intersection.
//!
//! Values can be label sets or elements of another finite ordered algebra.
//! Counts avoid expanding repeated occurrences. Complete results are antichains
//! closed under one union and intersections at all remaining occurrences.

use crate::{
    algorithms::{
        configurations::{matches_counts_with, visit_alignments_with},
        parallel::map_with_progress,
    },
    computation::{Cancellation, Cancelled, Computation, Phase},
    model::{Configuration, LabelSet},
};
use std::collections::{BTreeMap, BTreeSet};

/// Operations must be stable and compatible with a finite partial order:
/// union is a join, intersection is a meet, and both are monotone. Admissibility
/// must be upward closed. Ordinary nonempty sets and finite lattices satisfy
/// this contract. These laws justify discarding dominated configurations and
/// skipping comparable union operands; arbitrary callbacks need not do so.
pub trait Operations<T>: Sync {
    fn union(&self, left: &T, right: &T) -> T;
    fn intersection(&self, left: &T, right: &T) -> T;
    fn is_superset(&self, left: &T, right: &T) -> bool;
    /// Custom uses may keep their bottom/empty values; ordinary maximization
    /// overrides this to reject empty output choices.
    fn is_admissible(&self, _value: &T) -> bool {
        true
    }
}

/// Standard set algebra, with the empty-set policy local to the computation.
pub struct SetOperations {
    pub allow_empty: bool,
}
impl<L: Copy + Ord> Operations<LabelSet<L>> for SetOperations {
    fn union(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> LabelSet<L> {
        a.union(b)
    }
    fn intersection(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> LabelSet<L> {
        a.intersection(b)
    }
    fn is_superset(&self, a: &LabelSet<L>, b: &LabelSet<L>) -> bool {
        b.is_subset_of(a)
    }
    fn is_admissible(&self, value: &LabelSet<L>) -> bool {
        self.allow_empty || !value.is_empty()
    }
}

/// An optional derivation of an accepted configuration. Indices refer to the
/// parents' sorted distinct values, and intersection counts are row-major.
/// Parent copies and count matrices are allocated only when tracking is enabled.
pub struct Combination<T> {
    pub left: Configuration<T>,
    pub right: Configuration<T>,
    pub union_parts: (usize, usize),
    pub intersection_counts: Vec<u32>,
}

/// Called on the requesting thread when a derived configuration is admitted.
/// Later configurations may dominate it; this is a derivation trace, not a
/// notification that the configuration will belong to the final maximum.
pub type DerivationObserver<'a, T> = dyn FnMut(&Configuration<T>, Combination<T>) + 'a;

struct Candidate<T> {
    configuration: Configuration<T>,
    combination: Option<Combination<T>>,
}

fn dominates<T>(
    target: &Configuration<T>,
    family: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    matches_counts_with(
        family.multiplicities(),
        target.multiplicities(),
        |a, b| operations.is_superset(b, a),
        cancellation,
    )
}

fn insert<T: Clone + Ord>(
    current: &mut Vec<Configuration<T>>,
    candidate: Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    for old in current.iter() {
        if dominates(old, &candidate, operations, cancellation)? {
            return Ok(false);
        }
    }
    let mut kept = Vec::with_capacity(current.len() + 1);
    for old in current.drain(..) {
        if !dominates(&candidate, &old, operations, cancellation)? {
            kept.push(old)
        }
    }
    kept.push(candidate);
    *current = kept;
    Ok(true)
}

/// Compute the full closure; cancellation returns no purported maximum.
/// Mixed degrees are processed independently. A counted configuration of values
/// is an internal algebraic object, so it can contain empty sets without weakening
/// the nonempty `Part` invariant of the problem model.
pub fn maximize_custom_with<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    mut observe: Option<&mut DerivationObserver<'_, T>>,
) -> Result<Vec<Configuration<T>>, Cancelled> {
    let cancellation = computation.cancellation.clone();
    let mut degrees: BTreeMap<u32, Vec<Configuration<T>>> = BTreeMap::new();
    for configuration in initial {
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
    let batch_size = rayon::current_num_threads().saturating_mul(2).max(1);
    #[cfg(target_arch = "wasm32")]
    let batch_size = 1;
    let tracking = observe.is_some();
    let mut all = Vec::new();
    let mut completed = 0u32;
    computation.report(Phase::Maximizing, completed)?;
    for seeds in degrees.values_mut() {
        seeds.sort();
        let mut current = Vec::new();
        for seed in seeds.drain(..) {
            insert(&mut current, seed, operations, &cancellation)?;
        }
        current.sort();
        let mut seen: BTreeSet<_> = current.iter().cloned().collect();
        let mut previous = BTreeSet::new();
        loop {
            cancellation.check()?;
            let snapshot = current.clone();
            let mut pairs = Vec::with_capacity(batch_size);
            let mut changed = false;
            // Queue only pairs with a newly admitted parent, including self-pairs.
            // Bounded batches cap temporary results and deterministic merging avoids
            // a concurrent mutable antichain or per-request thread pool.
            let mut process = |pairs: &mut Vec<(usize, usize)>| -> Result<(), Cancelled> {
                // On the baseline fixtures, dispatching tiny pairs cost hundreds
                // of microseconds versus 4–15 us for the entire serial closure.
                // Estimate count-matrix freedom to reserve workers for larger
                // batches. This is scheduling only, never a search cutoff.
                let work = pairs.iter().fold(0usize, |sum, &(a, b)| {
                    let dimensions = snapshot[a]
                        .multiplicities()
                        .len()
                        .saturating_sub(1)
                        .saturating_mul(snapshot[b].multiplicities().len().saturating_sub(1));
                    sum.saturating_add(
                        (snapshot[a].degree() as usize)
                            .saturating_add(1)
                            .saturating_pow(u32::try_from(dimensions).unwrap_or(u32::MAX)),
                    )
                });
                let results = map_with_progress(
                    pairs.len(),
                    &cancellation,
                    work >= 20_000,
                    |index| {
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
                for candidates in results {
                    for candidate in candidates {
                        cancellation.check()?;
                        if !seen.insert(candidate.configuration.clone()) {
                            continue;
                        }
                        if insert(
                            &mut current,
                            candidate.configuration.clone(),
                            operations,
                            &cancellation,
                        )? {
                            changed = true;
                            if let (Some(observer), Some(combination)) =
                                (observe.as_mut(), candidate.combination)
                            {
                                observer(&candidate.configuration, combination);
                            }
                        }
                    }
                }
                pairs.clear();
                Ok(())
            };
            for a in 0..snapshot.len() {
                for b in 0..=a {
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
            if !pairs.is_empty() {
                process(&mut pairs)?;
            }
            if !changed {
                break;
            }
            previous = snapshot.into_iter().collect();
            current.sort();
        }
        all.extend(current);
    }
    all.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(all)
}

fn combine<T: Clone + Ord>(
    left: &Configuration<T>,
    right: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    tracking: bool,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    let a = left.multiplicities();
    let b = right.multiplicities();
    // Each intersection is computed once for all union choices and alignments.
    let mut intersections = Vec::with_capacity(a.len() * b.len());
    for (x, _) in a {
        for (y, _) in b {
            cancellation.check()?;
            intersections.push(operations.intersection(x, y));
        }
    }
    let compatible: Vec<_> = intersections
        .iter()
        .map(|v| operations.is_admissible(v))
        .collect();
    let mut result: Vec<Candidate<T>> = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, (x, _)) in a.iter().enumerate() {
        for (j, (y, _)) in b.iter().enumerate() {
            cancellation.check()?;
            if operations.is_superset(x, y) || operations.is_superset(y, x) {
                continue;
            }
            let union = operations.union(x, y);
            if !operations.is_admissible(&union) {
                continue;
            }
            let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
            let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
            rows[i] -= 1;
            columns[j] -= 1;
            visit_alignments_with(&rows, &columns, &compatible, cancellation, |counts| {
                let mut parts = vec![(union.clone(), 1)];
                parts.extend(
                    counts
                        .iter()
                        .zip(&intersections)
                        .filter(|(n, _)| **n > 0)
                        .map(|(&n, value)| (value.clone(), n)),
                );
                let configuration =
                    Configuration::new(parts).expect("combination preserves degree");
                if !seen.insert(configuration.clone()) {
                    return Ok(());
                }
                if dominates(left, &configuration, operations, cancellation)?
                    || dominates(right, &configuration, operations, cancellation)?
                {
                    return Ok(());
                }
                for old in &result {
                    if dominates(&old.configuration, &configuration, operations, cancellation)? {
                        return Ok(());
                    }
                }
                let mut kept = Vec::new();
                for old in result.drain(..) {
                    if !dominates(&configuration, &old.configuration, operations, cancellation)? {
                        kept.push(old)
                    }
                }
                kept.push(Candidate {
                    configuration,
                    combination: tracking.then(|| Combination {
                        left: left.clone(),
                        right: right.clone(),
                        union_parts: (i, j),
                        intersection_counts: counts.to_vec(),
                    }),
                });
                result = kept;
                Ok(())
            })?;
        }
    }
    Ok(result)
}
