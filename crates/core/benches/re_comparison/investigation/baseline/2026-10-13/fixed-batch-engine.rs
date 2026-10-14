//! Pairwise closure of counted configurations under custom union/intersection.
//!
//! Values can be label sets or elements of another finite ordered algebra.
//! Counts avoid expanding repeated occurrences. Complete results are antichains
//! closed under one union and intersections at all remaining occurrences.

pub use super::combination::{Combination, DerivationObserver};
use super::{
    antichain::Antichain,
    combination::{Candidate, combine},
};
use crate::{
    algorithms::parallel::{can_parallelize, map_with_progress, worker_count},
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
    // Large batches amortize dispatch and parallel global pruning. The cap
    // bounds temporary pair results; this is scheduling, never a search limit.
    #[cfg(not(target_arch = "wasm32"))]
    let batch_size = worker_count().saturating_mul(512).clamp(512, 4096);
    #[cfg(target_arch = "wasm32")]
    let batch_size = 1;
    let tracking = observe.is_some();
    let mut all = Vec::new();
    let mut completed = 0u32;
    computation.report(Phase::Maximizing, completed)?;
    for seeds in degrees.values_mut() {
        seeds.sort();
        let mut current = Antichain::default();
        for seed in seeds.drain(..) {
            current.insert(seed, operations, &cancellation)?;
        }
        current.sort();
        let mut seen: BTreeSet<_> = current.as_slice().iter().cloned().collect();
        let mut previous = BTreeSet::new();
        loop {
            cancellation.check()?;
            let snapshot = current.as_slice().to_vec();
            let mut pairs = Vec::with_capacity(batch_size);
            let mut changed = false;
            // Queue only pairs with a newly admitted parent, including self-pairs.
            // Bounded batches cap temporary results. Global comparisons use
            // immutable snapshots and apply their decisions on this thread.
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
                let batch_started = completed;
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
                            &seen,
                        )
                    },
                    |count| {
                        completed = batch_started.saturating_add(count as u32);
                        computation.report(Phase::Maximizing, completed)
                    },
                )?;
                let fresh = new_candidates(results, &mut seen, &cancellation)?;
                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;
                computation.report(Phase::Maximizing, completed)?;
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
        all.extend(current.into_vec());
    }
    all.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(all)
}

/// Query large histories concurrently before ordered insertion. The immutable
/// pass avoids cloning known configurations; insertion still keeps the first
/// derivation when several jobs produce the same new configuration.
fn new_candidates<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    seen: &mut BTreeSet<Configuration<T>>,
    cancellation: &Cancellation,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    cancellation.check()?;
    let mut candidates: Vec<_> = results.into_iter().flatten().collect();
    // A second lookup costs more than it saves for small histories/batches.
    if can_parallelize() && seen.len() >= 4096 && candidates.len() >= 2048 {
        let chunk_size = candidates.len().div_ceil(worker_count());
        let history = &*seen;
        let known = map_with_progress(
            candidates.len().div_ceil(chunk_size),
            cancellation,
            true,
            |chunk| {
                let start = chunk * chunk_size;
                let end = (start + chunk_size).min(candidates.len());
                let mut known = Vec::with_capacity(end - start);
                for candidate in &candidates[start..end] {
                    cancellation.check()?;
                    known.push(history.contains(&candidate.configuration));
                }
                Ok(known)
            },
            |_| cancellation.check(),
        )?;
        let mut known = known.into_iter().flatten();
        candidates.retain(|_| !known.next().expect("history mask"));
    }
    let mut fresh = Vec::new();
    for candidate in candidates {
        cancellation.check()?;
        if seen.insert(candidate.configuration.clone()) {
            fresh.push(candidate);
        }
    }
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_filter_preserves_first_derivations_in_large_batches() {
        let row = |label| Configuration::new(vec![(label, 1)]).unwrap();
        let mut history: BTreeSet<_> = (0u32..4096).map(row).collect();
        // Include known rows and two copies of every new row across job groups.
        let jobs = (0..3)
            .map(|job| {
                (0..1024)
                    .map(|index| {
                        let label = if job == 0 { index } else { 4096 + index };
                        let configuration = row(label);
                        Candidate {
                            combination: Some(Box::new(Combination {
                                left: configuration.clone(),
                                right: configuration.clone(),
                                union_parts: (job, index as usize),
                                intersection_counts: Vec::new(),
                            })),
                            configuration,
                        }
                    })
                    .collect()
            })
            .collect();
        let cancellation = Cancellation::default();
        let fresh = new_candidates(jobs, &mut history, &cancellation).unwrap();
        assert_eq!(fresh.len(), 1024);
        assert_eq!(history.len(), 5120);
        for (index, candidate) in fresh.iter().enumerate() {
            assert_eq!(candidate.configuration, row(4096 + index as u32));
            assert_eq!(
                candidate.combination.as_ref().unwrap().union_parts,
                (1, index)
            );
        }
        cancellation.cancel();
        assert!(new_candidates(Vec::new(), &mut history, &cancellation).is_err());
    }
}
