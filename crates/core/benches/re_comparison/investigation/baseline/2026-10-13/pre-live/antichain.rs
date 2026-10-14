//! LLM: Retained configurations, cached dominance checks, and ordered batch pruning.

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

/// LLM: A reusable queue of configurations that previously proved dominance. Cached entries
/// LLM: accelerate rejection without changing the exact containment relation.
type Cache<T> = VecDeque<Configuration<T>>;

/// LLM: Check whether a target counted row contains another under the supplied algebra using
/// LLM: exact capacity matching. Used throughout antichain rejection and dominance pruning.
pub(super) fn dominates<T>(
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

/// LLM: The current mutually undominated counted configurations of one degree, plus reusable
/// LLM: dominance caches and scheduling state. A cached removed row remains a sound dominator
/// LLM: by transitivity.
pub(super) struct Antichain<T> {
    configurations: Vec<Configuration<T>>,
    recent: Cache<T>,
    // LLM: Each chunk uses one distinct lock; the antichain itself is never shared
    // LLM: mutably between workers. Slots retain learned dominators across batches.

    caches: Vec<Mutex<Cache<T>>>,
}

impl<T> Default for Antichain<T> {
    /// LLM: Create the empty initial antichain state. Used before collecting values or
    /// LLM: admitting computed rows.
    fn default() -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            configurations: Vec::new(),
            recent: VecDeque::new(),
            caches: Vec::new(),
        }
    }
}

impl<T: Clone + Ord> Antichain<T> {
    /// LLM: Return the borrowed retained entries from Antichain. Used by the retained
    /// LLM: comparison benchmarks without exposing mutable storage.
    pub(super) fn as_slice(&self) -> &[Configuration<T>] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.configurations
    }

    /// LLM: Order retained rows deterministically before the next closure snapshot. Used
    /// LLM: between passes so result order does not depend on parallel completion.
    pub(super) fn sort(&mut self) {
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        self.configurations.sort();
    }

    /// LLM: Return the owned retained entries from Antichain. Used by the retained comparison
    /// LLM: benchmarks without exposing mutable storage.
    pub(super) fn into_vec(self) -> Vec<Configuration<T>> {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.configurations
    }

    /// LLM: Reject a dominated row or admit it and remove weaker retained rows. Used by serial
    /// LLM: closure initialization and ordered traced admission.
    pub(super) fn insert(
        &mut self,
        candidate: Configuration<T>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
    ) -> Result<bool, Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        for old in &self.recent {
            if dominates(old, &candidate, operations, cancellation)? {
                return Ok(false);
            }
        }
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for old in &self.configurations {
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            if dominates(old, &candidate, operations, cancellation)? {
                self.recent.push_front(old.clone());
                self.recent
                    .truncate((self.configurations.len() / 25).max(1));
                return Ok(false);
            }
        }
        // LLM: Prepare `kept` for the following computation; it stores the kept for this
        // LLM: stage.
        let mut kept = Vec::with_capacity(self.configurations.len() + 1);
        for old in self.configurations.drain(..) {
            if !dominates(&candidate, &old, operations, cancellation)? {
                kept.push(old);
            }
        }
        kept.push(candidate);
        self.configurations = kept;
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(true)
    }
}

impl<T: Clone + Ord + Send + Sync> Antichain<T> {
    /// LLM: Admit a batch of already deduplicated candidates using the selected serial or
    /// LLM: parallel pruning policy. Used after pair generation; tracing preserves
    /// LLM: deterministic observer order.
    pub(super) fn merge(
        &mut self,
        candidates: Vec<Candidate<T>>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
        observe: &mut Option<&mut DerivationObserver<'_, T>>,
    ) -> Result<bool, Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let parallel = can_parallelize()
            && candidates.len() >= 32
            && candidates.len().saturating_mul(self.configurations.len()) >= 4096;
        // LLM: Track alternatives still compatible with the selected support or current
        // LLM: antichain.
        let candidates = if parallel {
            self.reject(candidates, operations, cancellation)?
        } else {
            candidates
        };
        if parallel && observe.is_none() {
            return self.merge_maxima(candidates, operations, cancellation);
        }
        // LLM: Tracing must preserve intermediate admissions, including candidates
        // LLM: dominated by a later candidate in the same batch.

        let mut changed = false;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for Candidate {
            configuration,
            combination,
        } in candidates
        {
            // LLM: Observe cooperative interruption before continuing this unit of work.
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
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(changed)
    }

    /// LLM: Filter candidates already covered by current rows, consulting reusable dominator
    /// LLM: caches. Used before the remaining global antichain merge.
    fn reject(
        &mut self,
        candidates: Vec<Candidate<T>>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
    ) -> Result<Vec<Candidate<T>>, Cancelled> {
        // LLM: Prepare `workers` for the following computation; it stores the workers for this
        // LLM: stage.
        let workers = worker_count();
        self.caches
            .resize_with(workers, || Mutex::new(self.recent.clone()));
        let chunk_size = candidates.len().div_ceil(workers);
        let caches = &self.caches;
        let current = &self.configurations;
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        let rejected = map_with_progress(
            candidates.len().div_ceil(chunk_size),
            cancellation,
            true,
            |index| {
                // LLM: Retain reusable artifacts or dominators so proven work need not be
                // LLM: repeated.
                let mut cache = caches[index].lock().expect("dominator cache poisoned");
                let start = index * chunk_size;
                let end = (start + chunk_size).min(candidates.len());
                let mut rejected = Vec::with_capacity(end - start);
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for candidate in &candidates[start..end] {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    let mut dominated = false;
                    for old in cache.iter() {
                        if dominates(old, &candidate.configuration, operations, cancellation)? {
                            dominated = true;
                            break;
                        }
                    }
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    if !dominated {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for old in current {
                            // LLM: Check containment before admitting a row or marking a
                            // LLM: weaker row redundant.
                            if dominates(old, &candidate.configuration, operations, cancellation)? {
                                cache.push_front(old.clone());
                                cache.truncate((current.len() / 25).max(1));
                                dominated = true;
                                break;
                            }
                        }
                    }
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    rejected.push(dominated);
                }
                // LLM: Return the result of this completed search or conversion to its caller.
                Ok(rejected)
            },
            |_| cancellation.check(),
        )?;
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(candidates
            .into_iter()
            .zip(rejected.into_iter().flatten())
            .filter_map(|(candidate, rejected)| (!rejected).then_some(candidate))
            .collect())
    }

    /// LLM: Merge batch-local maxima into the global antichain and discard remaining dominated
    /// LLM: rows. Used by the frozen parallel-pruning benchmark kernel.
    fn merge_maxima(
        &mut self,
        candidates: Vec<Candidate<T>>,
        operations: &impl Operations<T>,
        cancellation: &Cancellation,
    ) -> Result<bool, Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if candidates.is_empty() {
            return Ok(false);
        }
        let current = &self.configurations;
        let count = current.len() + candidates.len();
        let chunk_size = count.div_ceil(worker_count());
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        let removed = map_with_progress(
            count.div_ceil(chunk_size),
            cancellation,
            current.len().saturating_mul(candidates.len()) >= 256,
            |chunk| {
                // LLM: Prepare `start` for the following computation; it stores the start for
                // LLM: this stage.
                let start = chunk * chunk_size;
                let end = (start + chunk_size).min(count);
                let mut removed = Vec::with_capacity(end - start);
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for index in start..end {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    let mut dominated = false;
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    if index < current.len() {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for candidate in &candidates {
                            // LLM: Check containment before admitting a row or marking a
                            // LLM: weaker row redundant.
                            if dominates(
                                &candidate.configuration,
                                &current[index],
                                operations,
                                cancellation,
                            )? {
                                dominated = true;
                                break;
                            }
                        }
                    } else {
                        // LLM: Prepare `own` for the following computation; it stores the own
                        // LLM: for this stage.
                        let own = index - current.len();
                        let offered = &candidates[own].configuration;
                        // LLM: Existing configurations cannot dominate a candidate
                        // LLM: that survived reject(). Only its peers remain to test.

                        for (other, candidate) in candidates.iter().enumerate() {
                            // LLM: Check containment before admitting a row or marking a
                            // LLM: weaker row redundant.
                            if other != own
                                && dominates(
                                    &candidate.configuration,
                                    offered,
                                    operations,
                                    cancellation,
                                )?
                            {
                                dominated = true;
                                break;
                            }
                        }
                    }
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    removed.push(dominated);
                }
                // LLM: Return the result of this completed search or conversion to its caller.
                Ok(removed)
            },
            |_| cancellation.check(),
        )?;
        // LLM: Prepare `removed` for the following computation; it stores the removed for this
        // LLM: stage.
        let mut removed = removed.into_iter().flatten();
        self.configurations
            .retain(|_| !removed.next().expect("existing mask"));
        cancellation.check()?;
        let mut changed = false;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for candidate in candidates {
            cancellation.check()?;
            if !removed.next().expect("candidate mask") {
                self.configurations.push(candidate.configuration);
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::super::engine::SetOperations;
    use super::*;
    use crate::model::LabelSet;
    use std::collections::BTreeSet;

    /// LLM: The counted bit-set configuration type used by pruning test and benchmark
    /// LLM: fixtures.
    type Row = Configuration<LabelSet<u16>>;

    /// LLM: Construct one counted bit-set row for a pruning fixture. Used to compare serial
    /// LLM: and parallel antichains with the same algebra.
    fn row(labels: Vec<u16>) -> Row {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Configuration::new(vec![(LabelSet::new(labels), 2)]).unwrap()
    }

    /// LLM: Prepare the initial counted rows for the pruning comparison. Used to seed
    /// LLM: equivalent serial and batch admission tests.
    fn roots() -> Vec<Row> {
        // LLM: Update the current working state before continuing this operation.
        (0..16)
            .flat_map(|a| ((a + 1)..16).map(move |b| row(vec![a, b])))
            .take(64)
            .collect()
    }

    /// LLM: Generate deterministic batches of candidate rows, including dominated
    /// LLM: alternatives. Used to compare pruning implementations across repeated admissions.
    fn batches() -> Vec<Vec<Row>> {
        // LLM: Prepare `first` for the following computation; it stores the first for this
        // LLM: stage.
        let first = roots()
            .into_iter()
            .flat_map(|root| {
                let labels: Vec<_> = root.multiplicities()[0].0.iter().collect();
                let mut larger = labels.clone();
                larger.push(32 + labels[0] % 4);
                [row(vec![labels[0]]), row(larger), root, row(vec![])]
            })
            .collect();
        // LLM: Prepare `second` for the following computation; it stores the second for this
        // LLM: stage.
        let second = (0..16)
            .flat_map(|a| {
                // LLM: Update the current working state before continuing this operation.
                ((a + 1)..16).map(move |b| {
                    Configuration::new(vec![
                        (LabelSet::new(vec![a]), 1),
                        (LabelSet::new(vec![b]), 1),
                    ])
                    .unwrap()
                })
            })
            .collect();
        // LLM: Update the current working state before continuing this operation.
        vec![first, second]
    }

    /// LLM: Wrap fixture rows with the payload needed by admission tests. Used to verify
    /// LLM: ordered tracing or pruning equivalence.
    fn candidates(rows: &[Row]) -> Vec<Candidate<LabelSet<u16>>> {
        // LLM: Update the current working state before continuing this operation.
        rows.iter()
            .cloned()
            .map(|configuration| Candidate {
                configuration,
                combination: None,
            })
            .collect()
    }

    /// LLM: Regression test: verify that batch maxima match ordered insertion and keep removed
    /// LLM: cache entries sound. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn batch_maxima_match_ordered_insertion_and_keep_removed_cache_entries_sound() {
        // LLM: Prepare `operations` for the following computation; it stores the operations
        // LLM: for this stage.
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = Antichain::default();
        let mut parallel = Antichain::default();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for root in roots() {
            serial
                .insert(root.clone(), &operations, &cancellation)
                .unwrap();
            parallel.insert(root, &operations, &cancellation).unwrap();
        }
        let mut seen: BTreeSet<_> = roots().into_iter().collect();
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for rows in batches() {
            // LLM: Collect genuinely new candidates before their antichain admission.
            let fresh: Vec<_> = rows
                .into_iter()
                .filter(|row| seen.insert(row.clone()))
                .collect();
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for row in &fresh {
                serial
                    .insert(row.clone(), &operations, &cancellation)
                    .unwrap();
            }
            let filtered = parallel
                .reject(candidates(&fresh), &operations, &cancellation)
                .unwrap();
            // LLM: Update the current working state before continuing this operation.
            parallel
                .merge_maxima(filtered, &operations, &cancellation)
                .unwrap();
            assert_eq!(
                serial.as_slice().iter().collect::<BTreeSet<_>>(),
                parallel.as_slice().iter().collect::<BTreeSet<_>>()
            );
        }
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(!parallel.caches.is_empty());
        cancellation.cancel();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
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

    /// LLM: Regression test: verify that parallel rejection preserves the sequence of
    /// LLM: admissions. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn parallel_rejection_preserves_the_sequence_of_admissions() {
        // LLM: Prepare `operations` for the following computation; it stores the operations
        // LLM: for this stage.
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = Antichain::default();
        let mut parallel = Antichain::default();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for root in roots() {
            serial
                .insert(root.clone(), &operations, &cancellation)
                .unwrap();
            parallel.insert(root, &operations, &cancellation).unwrap();
        }
        let mut seen: BTreeSet<_> = roots().into_iter().collect();
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for rows in batches() {
            // LLM: Collect genuinely new candidates before their antichain admission.
            let fresh: Vec<_> = rows
                .into_iter()
                .filter(|row| seen.insert(row.clone()))
                .collect();
            let mut expected = Vec::new();
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for row in &fresh {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if serial
                    .insert(row.clone(), &operations, &cancellation)
                    .unwrap()
                {
                    expected.push(row.clone());
                }
            }
            // LLM: Prepare `filtered` for the following computation; it stores the filtered
            // LLM: for this stage.
            let filtered = parallel
                .reject(candidates(&fresh), &operations, &cancellation)
                .unwrap();
            let mut admitted = Vec::new();
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for candidate in filtered {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if parallel
                    .insert(candidate.configuration.clone(), &operations, &cancellation)
                    .unwrap()
                {
                    admitted.push(candidate.configuration);
                }
            }
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(admitted, expected);
            assert_eq!(serial.as_slice(), parallel.as_slice());
        }
    }
}
