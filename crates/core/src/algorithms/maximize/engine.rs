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
    algorithms::parallel::{
        can_parallelize, map_with_progress, map_with_progress_with_minimum, worker_count,
    },
    computation::{Cancellation, Cancelled, Computation, Phase, WorkProgress},
    model::{Configuration, LabelSet},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

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
    observe: Option<&mut DerivationObserver<'_, T>>,
) -> Result<Vec<Configuration<T>>, Cancelled> {
    maximize_with_minimum(initial, operations, computation, observe, 1)
}

/// Ordinary small-alphabet set operations can amortize cheap generation with
/// coarser tasks. Custom algebras retain the normal scheduler through the public
/// entry point; neither changes the algebra or the bounded history batches.
pub(super) fn maximize_with_minimum<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    observe: Option<&mut DerivationObserver<'_, T>>,
    minimum: usize,
) -> Result<Vec<Configuration<T>>, Cancelled> {
    maximize_with_storage(initial, operations, computation, observe, minimum, false)
}

/// Dynamic wide masks benefit from interning repeated value payloads in history.
/// Other representations retain full keys. This private policy does not change
/// the generic public algebra or the closure's mathematical search.
pub(super) fn maximize_with_storage<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    mut observe: Option<&mut DerivationObserver<'_, T>>,
    minimum: usize,
    compact_history: bool,
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
    // The cap bounds temporary pair results, never the closure's search.
    // Cheap generation benefits from large batches; expensive generation needs
    // earlier history updates to avoid repeating local containment work.
    #[cfg(not(target_arch = "wasm32"))]
    let batch_capacity = worker_count().saturating_mul(512).clamp(512, 4096);
    #[cfg(target_arch = "wasm32")]
    let batch_capacity = 1;
    let tracking = observe.is_some();
    let mut all = Vec::new();
    let mut completed = 0u32;
    let mut pass = 0u32;
    computation.report(Phase::Maximizing, completed)?;
    for seeds in degrees.values_mut() {
        // Each degree learns its own cost. Cell lets the queue and its processing
        // closure share this caller-local setting without sharing mutable state
        // with workers.
        let batch_size = Cell::new(batch_capacity);
        // Sharing all generated keys pays on dense algebras. A common admissible
        // lower bound certifies that every future intersection stays admissible;
        // sparse problems retain immutable history and pair-local deduplication.
        let share_generated =
            !tracking && all_intersections_admissible(seeds, operations, &cancellation)?;
        // Unit counts give uniform margin sizes; coarser packed tasks reduced
        // dispatch CPU there. Repeated-count workloads need finer stealing.
        // Custom algebras and traced runs retain their original task splitting.
        let minimum = if minimum > 1
            && !tracking
            && seeds
                .iter()
                .all(|row| row.multiplicities().iter().all(|(_, n)| *n == 1))
        {
            512
        } else {
            minimum
        };
        seeds.sort();
        let mut current = Antichain::default();
        for seed in seeds.drain(..) {
            current.insert(seed, operations, &cancellation)?;
        }
        current.sort();
        let mut seen = super::history::History::new(compact_history);
        for row in current.as_slice() {
            cancellation.check()?;
            seen.insert(row.clone());
        }
        let mut previous = BTreeSet::new();
        loop {
            cancellation.check()?;
            let snapshot = current.as_slice().to_vec();
            pass = pass.saturating_add(1);
            let previous_count = snapshot
                .iter()
                .filter(|row| previous.contains(*row))
                .count();
            let triangular = |n: usize| n as u128 * (n as u128 + 1) / 2;
            let total = u64::try_from(triangular(snapshot.len()) - triangular(previous_count))
                .unwrap_or(u64::MAX);
            let mut pass_completed = 0u64;
            computation.report_work(
                Phase::Maximizing,
                completed,
                WorkProgress {
                    completed: 0,
                    total,
                    pass,
                },
            )?;
            let mut pairs = Vec::with_capacity(batch_capacity);
            let mut changed = false;
            // Queue only pairs with a newly admitted parent, including self-pairs.
            // Bounded batches cap temporary results. Untraced native pruning
            // shares admissions and removals immediately between workers.
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
                // Serial and wasm retain their fixed batches. Observed
                // derivations also retain timing-independent admission order.
                #[cfg(not(target_arch = "wasm32"))]
                let generation_started =
                    (!tracking && can_parallelize()).then(std::time::Instant::now);
                let batch_started = completed;
                let pass_started = pass_completed;
                let generated = share_generated.then(|| Mutex::new(BTreeSet::new()));
                let results = map_with_progress_with_minimum(
                    pairs.len(),
                    &cancellation,
                    work >= 20_000,
                    if !tracking && pairs.len() >= 512 {
                        minimum
                    } else {
                        1
                    },
                    |index| {
                        let (a, b) = pairs[index];
                        combine(
                            &snapshot[a],
                            &snapshot[b],
                            operations,
                            &cancellation,
                            tracking,
                            &seen,
                            generated.as_ref(),
                        )
                    },
                    |count| {
                        completed = batch_started.saturating_add(count as u32);
                        pass_completed = pass_started.saturating_add(count as u64);
                        computation.report_work(
                            Phase::Maximizing,
                            completed,
                            WorkProgress {
                                completed: pass_completed,
                                total,
                                pass,
                            },
                        )
                    },
                )?;
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(started) = generation_started {
                    batch_size.set(next_generation_batch_size(
                        batch_size.get(),
                        batch_capacity,
                        pairs.len(),
                        started.elapsed(),
                    ));
                }
                let fresh = new_candidates(results, &mut seen, &cancellation)?;
                if let Some(generated) = generated {
                    // Insert survivors first: otherwise history would reject them
                    // as already processed. Every other recorded row is dominated
                    // by a parent or a survivor, including local rejections.
                    for configuration in generated.into_inner().unwrap() {
                        cancellation.check()?;
                        seen.insert(configuration);
                    }
                }
                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;
                computation.report_work(
                    Phase::Maximizing,
                    completed,
                    WorkProgress {
                        completed: pass_completed,
                        total,
                        pass,
                    },
                )?;
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
                    if pairs.len() == batch_size.get() {
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

/// Meets stay above this common lower bound under every later join/meet.
/// Upward-closed admissibility therefore certifies dense intersection matrices.
/// This selects a history policy only; false leaves the exact search unchanged.
fn all_intersections_admissible<T: Clone>(
    seeds: &[Configuration<T>],
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let mut values = seeds
        .iter()
        .flat_map(|row| row.multiplicities().iter().map(|(v, _)| v));
    let Some(first) = values.next() else {
        return Ok(false);
    };
    let mut bottom = first.clone();
    for value in values {
        cancellation.check()?;
        bottom = operations.intersection(&bottom, value);
        if !operations.is_admissible(&bottom) {
            return Ok(false);
        }
    }
    Ok(operations.is_admissible(&bottom))
}

/// Target about 10 ms of generation, learning from full batches and expensive
/// round tails. Cheap tails do not provide a reliable estimate of throughput.
/// Bounded adjustments limit sensitivity to a single unusually slow/fast batch.
#[cfg(not(target_arch = "wasm32"))]
fn next_generation_batch_size(
    current: usize,
    capacity: usize,
    pairs: usize,
    elapsed: std::time::Duration,
) -> usize {
    const TARGET: std::time::Duration = std::time::Duration::from_millis(10);
    if pairs < current && elapsed < TARGET {
        return current;
    }
    let desired = (pairs as u128 * TARGET.as_nanos() / elapsed.as_nanos().max(1))
        .min(usize::MAX as u128) as usize;
    desired
        .clamp(current / 2, current.saturating_mul(2))
        .clamp(512, capacity)
}

/// Query large histories concurrently before ordered insertion. The immutable
/// pass avoids cloning known configurations; insertion still keeps the first
/// derivation when several jobs produce the same new configuration.
fn new_candidates<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    seen: &mut super::history::History<T>,
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
    fn dense_generated_history_matches_ordered_traced_closure() {
        struct Bits;
        impl Operations<u8> for Bits {
            fn union(&self, a: &u8, b: &u8) -> u8 {
                a | b
            }
            fn intersection(&self, a: &u8, b: &u8) -> u8 {
                a & b
            }
            fn is_superset(&self, a: &u8, b: &u8) -> bool {
                b & !a == 0
            }
        }
        let mut random = 19u32;
        for _ in 0..16 {
            let mut seeds = Vec::new();
            for _ in 0..5 {
                let mut parts = Vec::new();
                for _ in 0..3 {
                    random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                    parts.push((((random >> 16) & 7) as u8, 1));
                }
                seeds.push(Configuration::new(parts).unwrap());
            }
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            let shared = maximize_custom_with(&seeds, &Bits, &mut computation, None).unwrap();
            let mut observe = |_: &Configuration<u8>, _: Combination<u8>| {};
            let ordered =
                maximize_custom_with(&seeds, &Bits, &mut computation, Some(&mut observe)).unwrap();
            assert_eq!(shared, ordered);
        }
    }

    #[test]
    fn progress_counts_the_whole_pass_and_resets_only_for_a_new_pass() {
        let initial: Vec<_> = (0..=3)
            .map(|a| {
                Configuration::new(vec![
                    (LabelSet::new(vec![0u8]), a),
                    (LabelSet::new(vec![1u8]), 3 - a),
                ])
                .unwrap()
            })
            .collect();
        let mut updates = Vec::new();
        maximize_custom_with(
            &initial,
            &SetOperations { allow_empty: false },
            &mut Computation::new(Cancellation::default(), &mut |progress| {
                if let Some(work) = progress.work {
                    updates.push(work);
                }
            }),
            None,
        )
        .unwrap();
        let mut passes = BTreeMap::<_, Vec<_>>::new();
        for work in updates {
            assert!(work.completed <= work.total);
            passes.entry(work.pass).or_default().push(work);
        }
        assert!(
            passes.len() >= 2,
            "closure continues after discovering stronger configurations"
        );
        assert_eq!(passes[&1][0].total, 10); // Four initial parents, including self-pairs.
        for work in passes.values() {
            assert_eq!(work[0].completed, 0);
            assert_eq!(work.last().unwrap().completed, work[0].total);
            assert!(work.windows(2).all(
                |pair| pair[0].total == pair[1].total && pair[0].completed <= pair[1].completed
            ));
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn expensive_round_tails_reduce_batches_before_the_first_full_batch() {
        use std::time::Duration;
        assert_eq!(
            next_generation_batch_size(4096, 4096, 128, Duration::from_millis(20)),
            2048
        );
        assert_eq!(
            next_generation_batch_size(4096, 4096, 128, Duration::from_millis(1)),
            4096
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn batch_adjustments_keep_dispatch_amortized_and_respect_worker_capacity() {
        use std::time::Duration;
        let slow = Duration::from_secs(1);
        assert_eq!(next_generation_batch_size(1024, 4096, 1024, slow), 512);
        assert_eq!(next_generation_batch_size(512, 4096, 512, slow), 512);
        assert_eq!(
            next_generation_batch_size(512, 4096, 512, Duration::ZERO),
            1024
        );
        assert_eq!(
            next_generation_batch_size(1024, 1024, 1024, Duration::ZERO),
            1024
        );
        assert_eq!(
            next_generation_batch_size(1024, 4096, 1024, Duration::from_millis(10)),
            1024
        );
    }

    #[test]
    fn history_filter_preserves_first_derivations_in_large_batches() {
        let row = |label| Configuration::new(vec![(label, 1)]).unwrap();
        let mut history = super::super::history::History::new(false);
        for value in 0u32..4096 {
            history.insert(row(value));
        }
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
