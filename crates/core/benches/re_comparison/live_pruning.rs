//! Experimental live parallel antichain. Comparisons never hold the list lock.

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

struct Entry<T> {
    configuration: Configuration<T>,
    removed: AtomicBool,
    fresh: bool,
}

pub(super) fn merge_with<T: Clone + Ord + Send + Sync>(
    current: &mut Vec<Configuration<T>>,
    candidates: Vec<Candidate<T>>,
    caches: &mut Vec<Mutex<VecDeque<Configuration<T>>>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    workers: usize,
) -> Result<(bool, u64), Cancelled> {
    let workers = workers.max(1).min(candidates.len());
    if workers == 0 {
        return Ok((false, 0));
    }
    caches.resize_with(workers, || Mutex::new(VecDeque::new()));
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
    let next = AtomicUsize::new(0);
    let busy_ns = AtomicU64::new(0);
    map_with_progress(
        workers,
        cancellation,
        true,
        |worker| {
            let started = std::time::Instant::now();
            let mut cache = caches[worker].lock().expect("dominator cache poisoned");
            let mut snapshot = entries.read().expect("antichain poisoned").clone();
            loop {
                let start = next.fetch_add(8, Ordering::Relaxed);
                if start >= candidates.len() {
                    break;
                }
                for candidate in &candidates[start..(start + 8).min(candidates.len())] {
                    cancellation.check()?;
                    let configuration = &candidate.configuration;
                    let mut rejected = false;
                    for old in cache.iter() {
                        if dominates(old, configuration, operations, cancellation)? {
                            rejected = true;
                            break;
                        }
                    }
                    if rejected {
                        continue;
                    }
                    // Refresh immutable handles, then release the lock before
                    // matching. Removed entries stay allocated until workers exit.
                    {
                        let shared = entries.read().expect("antichain poisoned");
                        snapshot.extend(shared[snapshot.len()..].iter().cloned());
                    }
                    let mut checked = 0;
                    let published = loop {
                        for old in snapshot[checked..].iter().rev() {
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
                        if rejected {
                            break None;
                        }
                        checked = snapshot.len();
                        let mut shared = entries.write().expect("antichain poisoned");
                        if shared.len() != checked {
                            // Validate every concurrent admission before ours.
                            // The next matching pass again runs without the lock.
                            snapshot.extend(shared[checked..].iter().cloned());
                            drop(shared);
                            continue;
                        }
                        let entry = Arc::new(Entry {
                            configuration: configuration.clone(),
                            removed: AtomicBool::new(false),
                            fresh: true,
                        });
                        shared.push(entry.clone());
                        break Some(entry);
                    };
                    if let Some(published) = published {
                        for old in &snapshot {
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
                        snapshot.push(published);
                    }
                }
            }
            busy_ns.fetch_add(started.elapsed().as_nanos().min(u64::MAX as u128) as u64, Ordering::Relaxed);
            Ok(())
        },
        |_| cancellation.check(),
    )?;
    let entries = entries.into_inner().expect("antichain poisoned");
    let mut changed = false;
    *current = entries
        .into_iter()
        .filter_map(|entry| {
            if entry.removed.load(Ordering::Relaxed) {
                None
            } else {
                changed |= entry.fresh;
                Some(entry.configuration.clone())
            }
        })
        .collect();
    Ok((changed, busy_ns.load(Ordering::Relaxed) / candidates.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::antichain::Antichain;

    struct Bits;
    impl Operations<u64> for Bits {
        fn union(&self, a: &u64, b: &u64) -> u64 { a | b }
        fn intersection(&self, a: &u64, b: &u64) -> u64 { a & b }
        fn is_superset(&self, a: &u64, b: &u64) -> bool { b & !a == 0 }
    }

    fn offered(rows: &[Configuration<u64>]) -> Vec<Candidate<u64>> {
        rows.iter().cloned().map(|configuration| Candidate {
            configuration, combination: None,
        }).collect()
    }

    fn equivalent(a: &[Configuration<u64>], b: &[Configuration<u64>]) {
        let cancellation = Cancellation::default();
        for (source, target) in [(a, b), (b, a)] {
            for row in source {
                assert!(target.iter().any(|other| dominates(other, row, &Bits, &cancellation).unwrap()));
            }
        }
        for (i, row) in a.iter().enumerate() {
            assert!(!a.iter().enumerate().any(|(j, other)| i != j
                && dominates(other, row, &Bits, &cancellation).unwrap()));
        }
    }

    #[test]
    fn concurrent_admissions_and_removals_match_serial_insertion() {
        let cancellation = Cancellation::default();
        for seed in 1..=8 {
            let mut random = seed as u64;
            let mut next = || {
                random = random.wrapping_mul(6364136223846793005u64).wrapping_add(1442695040888963407u64);
                (random >> 32) & 255
            };
            let mut actual = Vec::new();
            let mut expected = Antichain::default();
            let mut caches = Vec::new();
            for _ in 0..3 {
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

    #[test]
    fn duplicate_races_and_cached_removed_dominators_are_sound() {
        let cancellation = Cancellation::default();
        let row = |bits| Configuration::new(vec![(bits, 5)]).unwrap();
        let mut actual = vec![row(1)];
        let mut caches = Vec::new();
        for rows in [vec![row(3); 80], vec![row(7); 80], vec![row(1); 80]] {
            merge_with(&mut actual, offered(&rows), &mut caches, &Bits, &cancellation, 4).unwrap();
        }
        assert_eq!(actual, vec![row(7)]);
    }

    #[test]
    fn cancellation_during_comparison_returns_no_maximum() {
        struct Stop { cancellation: Cancellation, calls: AtomicUsize }
        impl Operations<u64> for Stop {
            fn union(&self, a: &u64, b: &u64) -> u64 { a | b }
            fn intersection(&self, a: &u64, b: &u64) -> u64 { a & b }
            fn is_superset(&self, a: &u64, b: &u64) -> bool {
                if self.calls.fetch_add(1, Ordering::Relaxed) == 10 {
                    self.cancellation.cancel();
                }
                b & !a == 0
            }
        }
        let cancellation = Cancellation::default();
        let stop = Stop { cancellation: cancellation.clone(), calls: AtomicUsize::new(0) };
        let mut actual = Vec::new();
        let rows: Vec<_> = (1..80).map(|bits| Configuration::new(vec![(bits, 5)]).unwrap()).collect();
        assert!(merge_with(&mut actual, offered(&rows), &mut Vec::new(), &stop, &cancellation, 4).is_err());
    }
}
