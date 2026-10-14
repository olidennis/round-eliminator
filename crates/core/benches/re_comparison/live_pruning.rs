//! LLM: Experimental live parallel antichain. Comparisons never hold the list lock.

use super::{antichain::dominates, combination::Candidate, engine::Operations};
use crate::{
    algorithms::parallel::map_with_progress,
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
};

/// LLM: A published counted configuration, a monotone atomic removal flag, and whether it came
/// LLM: from this batch. Workers share immutable payloads; removed entries remain safe cached
/// LLM: dominators until the batch finishes.
struct Entry<T> {
    configuration: Configuration<T>,
    removed: AtomicBool,
    fresh: bool,
}

/// LLM: Prune a batch against a live shared antichain, publishing admissions before marking
/// LLM: weaker rows removed. Used by untraced native closure; returns whether surviving rows
/// LLM: changed and worker elapsed cost per candidate.
pub(super) fn merge_with<T: Clone + Ord + Send + Sync>(
    current: &mut Vec<Configuration<T>>,
    candidates: Vec<Candidate<T>>,
    caches: &mut Vec<Mutex<VecDeque<Configuration<T>>>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    workers: usize,
) -> Result<(bool, u64), Cancelled> {
    // LLM: Prepare `workers` for the following computation; it stores the workers for this
    // LLM: stage.
    let workers = workers.max(1).min(candidates.len());
    if workers == 0 {
        return Ok((false, 0));
    }
    caches.resize_with(workers, || Mutex::new(VecDeque::new()));
    // LLM: Publish immutable antichain handles separately from their monotone removal flags.
    let entries = RwLock::new(
        current
            .iter()
            .map(|configuration| {
                Arc::new(Entry {
                    configuration: configuration.clone(),
                    removed: AtomicBool::new(false),
                    fresh: false,
                })
            })
            .collect::<Vec<_>>(),
    );
    // LLM: Prepare `next` for the following computation; it stores the next for this stage.
    let next = AtomicUsize::new(0);
    let busy_ns = AtomicU64::new(0);
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    map_with_progress(
        workers,
        cancellation,
        true,
        |worker| {
            // LLM: Prepare `started` for the following computation; it stores the started for
            // LLM: this stage.
            let started = std::time::Instant::now();
            let mut cache = caches[worker].lock().expect("dominator cache poisoned");
            let mut snapshot = entries.read().expect("antichain poisoned").clone();
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            loop {
                // LLM: Prepare `start` for the following computation; it stores the start for
                // LLM: this stage.
                let start = next.fetch_add(8, Ordering::Relaxed);
                if start >= candidates.len() {
                    break;
                }
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for candidate in &candidates[start..(start + 8).min(candidates.len())] {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    let configuration = &candidate.configuration;
                    let mut rejected = false;
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    for old in cache.iter() {
                        if dominates(old, configuration, operations, cancellation)? {
                            rejected = true;
                            break;
                        }
                    }
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if rejected {
                        continue;
                    }
                    // LLM: Refresh immutable handles, then release the lock before
                    // LLM: matching. Removed entries stay allocated until workers exit.

                    {
                        let shared = entries.read().expect("antichain poisoned");
                        snapshot.extend(shared[snapshot.len()..].iter().cloned());
                    }
                    let mut checked = 0;
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    let published = loop {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for old in snapshot[checked..].iter().rev() {
                            // LLM: Observe cooperative interruption before continuing this
                            // LLM: unit of work.
                            cancellation.check()?;
                            if !old.removed.load(Ordering::Relaxed)
                                && dominates(
                                    &old.configuration,
                                    configuration,
                                    operations,
                                    cancellation,
                                )?
                            {
                                cache.push_front(old.configuration.clone());
                                cache.truncate((snapshot.len() / 25).max(1));
                                rejected = true;
                                break;
                            }
                        }
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if rejected {
                            break None;
                        }
                        checked = snapshot.len();
                        let mut shared = entries.write().expect("antichain poisoned");
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if shared.len() != checked {
                            // LLM: Validate every concurrent admission before ours.
                            // LLM: The next matching pass again runs without the lock.

                            snapshot.extend(shared[checked..].iter().cloned());
                            drop(shared);
                            continue;
                        }
                        // LLM: Prepare `entry` for the following computation; it stores the
                        // LLM: entry for this stage.
                        let entry = Arc::new(Entry {
                            configuration: configuration.clone(),
                            removed: AtomicBool::new(false),
                            fresh: true,
                        });
                        shared.push(entry.clone());
                        break Some(entry);
                    };
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    if let Some(published) = published {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for old in &snapshot {
                            // LLM: Observe cooperative interruption before continuing this
                            // LLM: unit of work.
                            cancellation.check()?;
                            if !old.removed.load(Ordering::Relaxed)
                                && dominates(
                                    configuration,
                                    &old.configuration,
                                    operations,
                                    cancellation,
                                )?
                            {
                                old.removed.store(true, Ordering::Relaxed);
                            }
                        }
                        // LLM: Retain the newly constructed choices in the result for this
                        // LLM: stage.
                        snapshot.push(published);
                    }
                }
            }
            // LLM: Update the current working state before continuing this operation.
            busy_ns.fetch_add(started.elapsed().as_nanos().min(u64::MAX as u128) as u64, Ordering::Relaxed);
            Ok(())
        },
        |_| cancellation.check(),
    )?;
    // LLM: Publish immutable antichain handles separately from their monotone removal flags.
    let entries = entries.into_inner().expect("antichain poisoned");
    let mut changed = false;
    // LLM: Update the current working state before continuing this operation.
    *current = entries
        .into_iter()
        .filter_map(|entry| {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if entry.removed.load(Ordering::Relaxed) {
                None
            } else {
                changed |= entry.fresh;
                Some(entry.configuration.clone())
            }
        })
        .collect();
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok((changed, busy_ns.load(Ordering::Relaxed) / candidates.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::antichain::Antichain;

    /// LLM: A small bit-set algebra used as an independent test/benchmark value domain. Join
    /// LLM: is bitwise OR, meet is AND, and containment tests inclusion of set bits.
    struct Bits;
    impl Operations<u64> for Bits {
        /// LLM: Compute the join in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn union(&self, a: &u64, b: &u64) -> u64 {
            // LLM: Compute the join in this representation's chosen standard or dual order.
            a | b }
        /// LLM: Compute the meet in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn intersection(&self, a: &u64, b: &u64) -> u64 {
            // LLM: Compute the meet in this representation's chosen standard or dual order.
            a & b }
        /// LLM: Test domination in this representation's chosen standard or dual order. Called
        /// LLM: by the generic pairwise closure and its pruning proofs.
        fn is_superset(&self, a: &u64, b: &u64) -> bool {
            // LLM: Test domination in this representation's chosen standard or dual order.
            b & !a == 0 }
    }

    /// LLM: Wrap counted rows as candidates without derivation traces. Used by live-pruning
    /// LLM: test and benchmark fixtures.
    fn offered(rows: &[Configuration<u64>]) -> Vec<Candidate<u64>> {
        // LLM: Update the current working state before continuing this operation.
        rows.iter().cloned().map(|configuration| Candidate {
            configuration, combination: None,
        }).collect()
    }

    /// LLM: Verify mutual semantic coverage and absence of internal dominance between pruning
    /// LLM: results. Used by tests where parallel admission order can choose different
    /// LLM: equivalent representatives.
    fn equivalent(a: &[Configuration<u64>], b: &[Configuration<u64>]) {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        for (source, target) in [(a, b), (b, a)] {
            for row in source {
                assert!(target.iter().any(|other| dominates(other, row, &Bits, &cancellation).unwrap()));
            }
        }
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for (i, row) in a.iter().enumerate() {
            assert!(!a.iter().enumerate().any(|(j, other)| i != j
                && dominates(other, row, &Bits, &cancellation).unwrap()));
        }
    }

    /// LLM: Regression test: verify that concurrent admissions and removals match serial
    /// LLM: insertion. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn concurrent_admissions_and_removals_match_serial_insertion() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        for seed in 1..=8 {
            // LLM: Prepare `random` for the following computation; it stores the random for
            // LLM: this stage.
            let mut random = seed as u64;
            let mut next = || {
                random = random.wrapping_mul(6364136223846793005u64).wrapping_add(1442695040888963407u64);
                (random >> 32) & 255
            };
            let mut actual = Vec::new();
            let mut expected = Antichain::default();
            let mut caches = Vec::new();
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for _ in 0..3 {
                // LLM: Prepare counted rows or remaining row capacities for the next exact
                // LLM: comparison.
                let rows: Vec<_> = (0..80).map(|_| Configuration::new(
                    vec![(next(), 1), (next(), 2), (next(), 3)]).unwrap()).collect();
                for row in &rows {
                    expected.insert(row.clone(), &Bits, &cancellation).unwrap();
                }
                merge_with(&mut actual, offered(&rows), &mut caches, &Bits, &cancellation, 4).unwrap();
                equivalent(&actual, expected.as_slice());
            }
        }
    }

    /// LLM: Regression test: verify that duplicate races and cached removed dominators are
    /// LLM: sound. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn duplicate_races_and_cached_removed_dominators_are_sound() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let row = |bits| Configuration::new(vec![(bits, 5)]).unwrap();
        let mut actual = vec![row(1)];
        let mut caches = Vec::new();
        for rows in [vec![row(3); 80], vec![row(7); 80], vec![row(1); 80]] {
            merge_with(&mut actual, offered(&rows), &mut caches, &Bits, &cancellation, 4).unwrap();
        }
        assert_eq!(actual, vec![row(7)]);
    }

    /// LLM: Regression test: verify that cancellation during comparison returns no maximum.
    /// LLM: Used by the test suite to protect this behavior while implementations change.
    #[test]
    fn cancellation_during_comparison_returns_no_maximum() {
        /// LLM: A test algebra carrying a cancellation handle and comparison counter. It
        /// LLM: deliberately interrupts dominance work to verify that the closure returns
        /// LLM: cancellation rather than a maximum.
        struct Stop { cancellation: Cancellation, calls: AtomicUsize }
        impl Operations<u64> for Stop {
            /// LLM: Compute the join in this representation's chosen standard or dual order.
            /// LLM: Called by the generic pairwise closure and its pruning proofs.
            fn union(&self, a: &u64, b: &u64) -> u64 {
                // LLM: Compute the join in this representation's chosen standard or dual
                // LLM: order.
                a | b }
            /// LLM: Compute the meet in this representation's chosen standard or dual order.
            /// LLM: Called by the generic pairwise closure and its pruning proofs.
            fn intersection(&self, a: &u64, b: &u64) -> u64 {
                // LLM: Compute the meet in this representation's chosen standard or dual
                // LLM: order.
                a & b }
            /// LLM: Test domination in this representation's chosen standard or dual order.
            /// LLM: Called by the generic pairwise closure and its pruning proofs.
            fn is_superset(&self, a: &u64, b: &u64) -> bool {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if self.calls.fetch_add(1, Ordering::Relaxed) == 10 {
                    self.cancellation.cancel();
                }
                b & !a == 0
            }
        }
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let stop = Stop { cancellation: cancellation.clone(), calls: AtomicUsize::new(0) };
        let mut actual = Vec::new();
        let rows: Vec<_> = (1..80).map(|bits| Configuration::new(vec![(bits, 5)]).unwrap()).collect();
        assert!(merge_with(&mut actual, offered(&rows), &mut Vec::new(), &stop, &cancellation, 4).is_err());
    }
}
