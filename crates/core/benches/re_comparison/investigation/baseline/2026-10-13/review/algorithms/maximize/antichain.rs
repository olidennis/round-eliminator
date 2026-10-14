//! Retained configurations, cached dominance checks, and concurrent pruning.

#[cfg(not(target_arch = "wasm32"))]
mod live;

use super::{
    combination::{Candidate, DerivationObserver},
    engine::Operations,
};
use crate::{
    algorithms::{
        configurations::matches_counts_with,
        parallel::{can_parallelize, map_with_progress, worker_count},
    },
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::{collections::VecDeque, sync::Mutex};

type Cache<T> = VecDeque<Configuration<T>>;

pub(super) fn dominates<T>(
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

/// Working antichain for one degree. A removed cache entry remains a sound
/// dominator: a stronger retained configuration covers it by transitivity.
pub(super) struct Antichain<T> {
    configurations: Vec<Configuration<T>>,
    recent: Cache<T>,
    #[cfg(not(target_arch = "wasm32"))]
    work_per_candidate_ns: u64,
    // Each worker owns one cache during a batch. Slots retain learned
    // dominators across batches, including entries subsequently replaced.
    caches: Vec<Mutex<Cache<T>>>,
}

impl<T> Default for Antichain<T> {
    fn default() -> Self {
        Self {
            configurations: Vec::new(),
            recent: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            work_per_candidate_ns: 0,
            caches: Vec::new(),
        }
    }
}

impl<T: Clone + Ord> Antichain<T> {
    pub(super) fn as_slice(&self) -> &[Configuration<T>] {
        &self.configurations
    }

    pub(super) fn sort(&mut self) {
        self.configurations.sort();
    }

    pub(super) fn into_vec(self) -> Vec<Configuration<T>> {
        self.configurations
    }

    pub(super) fn insert(
        &mut self,
        candidate: Configuration<T>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
    ) -> Result<bool, Cancelled> {
        cancellation.check()?;
        for old in &self.recent {
            if dominates(old, &candidate, operations, cancellation)? {
                return Ok(false);
            }
        }
        for old in &self.configurations {
            if dominates(old, &candidate, operations, cancellation)? {
                self.recent.push_front(old.clone());
                self.recent
                    .truncate((self.configurations.len() / 25).max(1));
                return Ok(false);
            }
        }
        let mut kept = Vec::with_capacity(self.configurations.len() + 1);
        for old in self.configurations.drain(..) {
            if !dominates(&candidate, &old, operations, cancellation)? {
                kept.push(old);
            }
        }
        kept.push(candidate);
        self.configurations = kept;
        Ok(true)
    }
}

impl<T: Clone + Ord + Send + Sync> Antichain<T> {
    /// The closure's history has already removed duplicate candidates.
    /// Untraced native batches share admissions and removals between workers.
    /// Tracing preserves ordered admissions; small batches and wasm use insertion.
    pub(super) fn merge(
        &mut self,
        candidates: Vec<Candidate<T>>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
        observe: &mut Option<&mut DerivationObserver<'_, T>>,
    ) -> Result<bool, Cancelled> {
        cancellation.check()?;
        let parallel = can_parallelize()
            && candidates.len() >= 32
            && candidates.len().saturating_mul(self.configurations.len()) >= 4096;
        #[cfg(not(target_arch = "wasm32"))]
        if parallel && observe.is_none() {
            // Estimate aggregate worker time from the last batch, aiming for
            // roughly 8 ms per worker. Cheap pruning starts with two workers;
            // expensive batches can use the whole shared pool. This is a
            // scheduling heuristic, not a bound on the mathematical search.
            let capacity = worker_count();
            let estimated =
                (candidates.len() as u128 * self.work_per_candidate_ns as u128).div_ceil(8_000_000);
            let workers = usize::try_from(estimated)
                .unwrap_or(usize::MAX)
                .clamp(2.min(capacity), capacity);
            let (changed, cost) = live::merge_with(
                &mut self.configurations,
                candidates,
                &mut self.caches,
                operations,
                cancellation,
                workers,
            )?;
            self.work_per_candidate_ns = cost;
            return Ok(changed);
        }
        let candidates = if parallel {
            self.reject(candidates, operations, cancellation)?
        } else {
            candidates
        };
        // Tracing must preserve intermediate admissions, including candidates
        // dominated by a later candidate in the same batch.
        let mut changed = false;
        for Candidate {
            configuration,
            combination,
        } in candidates
        {
            cancellation.check()?;
            if let (Some(observer), Some(combination)) = (observe.as_mut(), combination) {
                if self.insert(configuration.clone(), operations, cancellation)? {
                    changed = true;
                    observer(&configuration, *combination);
                }
            } else {
                changed |= self.insert(configuration, operations, cancellation)?;
            }
        }
        Ok(changed)
    }

    fn reject(
        &mut self,
        candidates: Vec<Candidate<T>>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
    ) -> Result<Vec<Candidate<T>>, Cancelled> {
        let workers = worker_count();
        self.caches
            .resize_with(workers, || Mutex::new(self.recent.clone()));
        let chunk_size = candidates.len().div_ceil(workers);
        let caches = &self.caches;
        let current = &self.configurations;
        let rejected = map_with_progress(
            candidates.len().div_ceil(chunk_size),
            cancellation,
            true,
            |index| {
                let mut cache = caches[index].lock().expect("dominator cache poisoned");
                let start = index * chunk_size;
                let end = (start + chunk_size).min(candidates.len());
                let mut rejected = Vec::with_capacity(end - start);
                for candidate in &candidates[start..end] {
                    cancellation.check()?;
                    let mut dominated = false;
                    for old in cache.iter() {
                        if dominates(old, &candidate.configuration, operations, cancellation)? {
                            dominated = true;
                            break;
                        }
                    }
                    if !dominated {
                        for old in current {
                            if dominates(old, &candidate.configuration, operations, cancellation)? {
                                cache.push_front(old.clone());
                                cache.truncate((current.len() / 25).max(1));
                                dominated = true;
                                break;
                            }
                        }
                    }
                    rejected.push(dominated);
                }
                Ok(rejected)
            },
            |_| cancellation.check(),
        )?;
        Ok(candidates
            .into_iter()
            .zip(rejected.into_iter().flatten())
            .filter_map(|(candidate, rejected)| (!rejected).then_some(candidate))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::engine::SetOperations;
    use super::*;
    use crate::model::LabelSet;
    use std::collections::BTreeSet;

    type Row = Configuration<LabelSet<u16>>;

    fn row(labels: Vec<u16>) -> Row {
        Configuration::new(vec![(LabelSet::new(labels), 2)]).unwrap()
    }

    fn roots() -> Vec<Row> {
        (0..16)
            .flat_map(|a| ((a + 1)..16).map(move |b| row(vec![a, b])))
            .take(64)
            .collect()
    }

    fn batches() -> Vec<Vec<Row>> {
        let first = roots()
            .into_iter()
            .flat_map(|root| {
                let labels: Vec<_> = root.multiplicities()[0].0.iter().collect();
                let mut larger = labels.clone();
                larger.push(32 + labels[0] % 4);
                [row(vec![labels[0]]), row(larger), root, row(vec![])]
            })
            .collect();
        let second = (0..16)
            .flat_map(|a| {
                ((a + 1)..16).map(move |b| {
                    Configuration::new(vec![
                        (LabelSet::new(vec![a]), 1),
                        (LabelSet::new(vec![b]), 1),
                    ])
                    .unwrap()
                })
            })
            .collect();
        vec![first, second]
    }

    fn candidates(rows: &[Row]) -> Vec<Candidate<LabelSet<u16>>> {
        rows.iter()
            .cloned()
            .map(|configuration| Candidate {
                configuration,
                combination: None,
            })
            .collect()
    }

    #[test]
    fn batch_maxima_match_ordered_insertion_and_keep_removed_cache_entries_sound() {
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = Antichain::default();
        let mut parallel = Antichain::default();
        for root in roots() {
            serial
                .insert(root.clone(), &operations, &cancellation)
                .unwrap();
            parallel.insert(root, &operations, &cancellation).unwrap();
        }
        let mut seen: BTreeSet<_> = roots().into_iter().collect();
        for rows in batches() {
            let fresh: Vec<_> = rows
                .into_iter()
                .filter(|row| seen.insert(row.clone()))
                .collect();
            for row in &fresh {
                serial
                    .insert(row.clone(), &operations, &cancellation)
                    .unwrap();
            }
            parallel
                .merge(candidates(&fresh), &operations, &cancellation, &mut None)
                .unwrap();
            assert_eq!(
                serial.as_slice().iter().collect::<BTreeSet<_>>(),
                parallel.as_slice().iter().collect::<BTreeSet<_>>()
            );
        }
        cancellation.cancel();
        assert!(
            parallel
                .merge(
                    candidates(&[row(vec![99])]),
                    &operations,
                    &cancellation,
                    &mut None
                )
                .is_err()
        );
    }

    #[test]
    fn parallel_rejection_preserves_the_sequence_of_admissions() {
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = Antichain::default();
        let mut parallel = Antichain::default();
        for root in roots() {
            serial
                .insert(root.clone(), &operations, &cancellation)
                .unwrap();
            parallel.insert(root, &operations, &cancellation).unwrap();
        }
        let mut seen: BTreeSet<_> = roots().into_iter().collect();
        for rows in batches() {
            let fresh: Vec<_> = rows
                .into_iter()
                .filter(|row| seen.insert(row.clone()))
                .collect();
            let mut expected = Vec::new();
            for row in &fresh {
                if serial
                    .insert(row.clone(), &operations, &cancellation)
                    .unwrap()
                {
                    expected.push(row.clone());
                }
            }
            let filtered = parallel
                .reject(candidates(&fresh), &operations, &cancellation)
                .unwrap();
            let mut admitted = Vec::new();
            for candidate in filtered {
                if parallel
                    .insert(candidate.configuration.clone(), &operations, &cancellation)
                    .unwrap()
                {
                    admitted.push(candidate.configuration);
                }
            }
            assert_eq!(admitted, expected);
            assert_eq!(serial.as_slice(), parallel.as_slice());
        }
    }
}
