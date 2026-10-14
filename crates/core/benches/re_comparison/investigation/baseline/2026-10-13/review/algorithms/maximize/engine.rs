//! LLM: Pairwise closure of counted configurations under custom union/intersection.
//! LLM: Values can be label sets or elements of another finite ordered algebra.
//! LLM: Counts avoid expanding repeated occurrences. Complete results are antichains
//! LLM: closed under one union and intersections at all remaining occurrences.

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
};

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

/// LLM: Compute the full undominated pairwise closure under a supplied ordered algebra. Used
/// LLM: by ordinary maximization and Basic FP; degrees remain separate, and interruption
/// LLM: returns no maximum.
pub fn maximize_custom_with<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    observe: Option<&mut DerivationObserver<'_, T>>,
) -> Result<Vec<Configuration<T>>, Cancelled> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    maximize_with_minimum(initial, operations, computation, observe, 1)
}

/// LLM: Select a generation grain before entering the same exact closure. Used by packed
/// LLM: ordinary maximization; the grain changes scheduling rather than admissible results.
pub(super) fn maximize_with_minimum<T: Clone + Ord + Send + Sync>(
    initial: &[Configuration<T>],
    operations: &impl Operations<T>,
    computation: &mut Computation<'_>,
    mut observe: Option<&mut DerivationObserver<'_, T>>,
    minimum: usize,
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
    // LLM: The cap bounds temporary pair results, never the closure's search.
    // LLM: Cheap generation benefits from large batches; expensive generation needs
    // LLM: earlier history updates to avoid repeating local containment work.

    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Prepare `batch_capacity` for the following computation; it stores the batch
    // LLM: capacity for this stage.
    let batch_capacity = worker_count().saturating_mul(512).clamp(512, 4096);
    #[cfg(target_arch = "wasm32")]
    // LLM: Prepare `batch_capacity` for the following computation; it stores the batch
    // LLM: capacity for this stage.
    let batch_capacity = 1;
    let tracking = observe.is_some();
    let mut all = Vec::new();
    let mut completed = 0u32;
    let mut pass = 0u32;
    computation.report(Phase::Maximizing, completed)?;
    // LLM: Check that computed indices and counts fit the model's integer representation.
    for seeds in degrees.values_mut() {
        // LLM: Each degree learns its own cost. Cell lets the queue and its processing
        // LLM: closure share this caller-local setting without sharing mutable state
        // LLM: with workers.

        let batch_size = Cell::new(batch_capacity);
        seeds.sort();
        let mut current = Antichain::default();
        for seed in seeds.drain(..) {
            current.insert(seed, operations, &cancellation)?;
        }
        current.sort();
        let mut seen: BTreeSet<_> = current.as_slice().iter().cloned().collect();
        // LLM: Record prior work so the next pass need only visit newly relevant parent pairs.
        let mut previous = BTreeSet::new();
        loop {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let snapshot = current.as_slice().to_vec();
            pass = pass.saturating_add(1);
            let previous_count = snapshot
                .iter()
                .filter(|row| previous.contains(*row))
                .count();
            let triangular = |n: usize| n as u128 * (n as u128 + 1) / 2;
            // LLM: Check that computed indices and counts fit the model's integer
            // LLM: representation.
            let total = u64::try_from(triangular(snapshot.len()) - triangular(previous_count))
                .unwrap_or(u64::MAX);
            let mut pass_completed = 0u64;
            // LLM: Update the current working state before continuing this operation.
            computation.report_work(
                Phase::Maximizing,
                completed,
                WorkProgress {
                    completed: 0,
                    total,
                    pass,
                },
            )?;
            // LLM: Queue the parent pairs for this bounded generation batch.
            let mut pairs = Vec::with_capacity(batch_capacity);
            let mut changed = false;
            // LLM: Queue only pairs with a newly admitted parent, including self-pairs.
            // LLM: Bounded batches cap temporary results. Untraced native pruning
            // LLM: shares admissions and removals immediately between workers.

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
                // LLM: Serial and wasm retain their fixed batches. Observed
                // LLM: derivations also retain timing-independent admission order.

                #[cfg(not(target_arch = "wasm32"))]
                // LLM: Prepare `generation_started` for the following computation; it stores
                // LLM: the generation started for this stage.
                let generation_started =
                    (!tracking && can_parallelize()).then(std::time::Instant::now);
                let batch_started = completed;
                let pass_started = pass_completed;
                // LLM: Generate consequences of the selected parent pair under one join and
                // LLM: the remaining meets.
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
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
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
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        completed = batch_started.saturating_add(count as u32);
                        pass_completed = pass_started.saturating_add(count as u64);
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
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
                // LLM: Adapt the next batch from observed generation cost while keeping
                // LLM: dispatch amortized.
                if let Some(started) = generation_started {
                    // LLM: Adapt the next batch from observed generation cost while keeping
                    // LLM: dispatch amortized.
                    batch_size.set(next_generation_batch_size(
                        batch_size.get(),
                        batch_capacity,
                        pairs.len(),
                        started.elapsed(),
                    ));
                }
                // LLM: Filter duplicate history before offering fresh rows to global dominance
                // LLM: pruning.
                let fresh = new_candidates(results, &mut seen, &cancellation)?;
                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;
                // LLM: Update the current working state before continuing this operation.
                computation.report_work(
                    Phase::Maximizing,
                    completed,
                    WorkProgress {
                        completed: pass_completed,
                        total,
                        pass,
                    },
                )?;
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
                    if pairs.len() == batch_size.get() {
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
        all.extend(current.into_vec());
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    all.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(all)
}

/// LLM: Estimate a batch size targeting about ten milliseconds of generation, with bounded
/// LLM: adjustments and capacity limits. Used by native untraced closure scheduling; expensive
/// LLM: short tails also provide evidence.
#[cfg(not(target_arch = "wasm32"))]
fn next_generation_batch_size(
    current: usize,
    capacity: usize,
    pairs: usize,
    elapsed: std::time::Duration,
) -> usize {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    const TARGET: std::time::Duration = std::time::Duration::from_millis(10);
    if pairs < current && elapsed < TARGET {
        return current;
    }
    let desired = (pairs as u128 * TARGET.as_nanos() / elapsed.as_nanos().max(1))
        .min(usize::MAX as u128) as usize;
    // LLM: Update the current working state before continuing this operation.
    desired
        .clamp(current / 2, current.saturating_mul(2))
        .clamp(512, capacity)
}

/// LLM: Remove previously generated configurations while preserving the first derivation of
/// LLM: each new row. Used between parallel pair generation and antichain admission.
fn new_candidates<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    seen: &mut BTreeSet<Configuration<T>>,
    cancellation: &Cancellation,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let mut candidates: Vec<_> = results.into_iter().flatten().collect();
    // LLM: A second lookup costs more than it saves for small histories/batches.

    if can_parallelize() && seen.len() >= 4096 && candidates.len() >= 2048 {
        // LLM: Prepare `chunk_size` for the following computation; it stores the chunk size
        // LLM: for this stage.
        let chunk_size = candidates.len().div_ceil(worker_count());
        let history = &*seen;
        // LLM: Observe cooperative interruption before performing the next unit of CPU work.
        let known = map_with_progress(
            candidates.len().div_ceil(chunk_size),
            cancellation,
            true,
            |chunk| {
                // LLM: Prepare `start` for the following computation; it stores the start for
                // LLM: this stage.
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
        // LLM: Prepare `known` for the following computation; it stores the known for this
        // LLM: stage.
        let mut known = known.into_iter().flatten();
        candidates.retain(|_| !known.next().expect("history mask"));
    }
    // LLM: Insert remaining candidates in input order so duplicate children preserve their
    // LLM: first derivation.
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

    /// LLM: Regression test: verify that progress counts the whole pass and resets only for a
    /// LLM: new pass. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn progress_counts_the_whole_pass_and_resets_only_for_a_new_pass() {
        // LLM: Prepare `initial` for the following computation; it stores the initial for this
        // LLM: stage.
        let initial: Vec<_> = (0..=3)
            .map(|a| {
                Configuration::new(vec![
                    (LabelSet::new(vec![0u8]), a),
                    (LabelSet::new(vec![1u8]), 3 - a),
                ])
                .unwrap()
            })
            .collect();
        // LLM: Prepare `updates` for the following computation; it stores the updates for this
        // LLM: stage.
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
        // LLM: Prepare `passes` for the following computation; it stores the passes for this
        // LLM: stage.
        let mut passes = BTreeMap::<_, Vec<_>>::new();
        for work in updates {
            assert!(work.completed <= work.total);
            passes.entry(work.pass).or_default().push(work);
        }
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            passes.len() >= 2,
            "closure continues after discovering stronger configurations"
        );
        assert_eq!(passes[&1][0].total, 10);  // LLM: Four initial parents, including self-pairs.
        for work in passes.values() {
            assert_eq!(work[0].completed, 0);
            assert_eq!(work.last().unwrap().completed, work[0].total);
            assert!(work.windows(2).all(
                |pair| pair[0].total == pair[1].total && pair[0].completed <= pair[1].completed
            ));
        }
    }

    /// LLM: Regression test: verify that expensive round tails reduce batches before the first
    /// LLM: full batch. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn expensive_round_tails_reduce_batches_before_the_first_full_batch() {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use std::time::Duration;
        assert_eq!(
            next_generation_batch_size(4096, 4096, 128, Duration::from_millis(20)),
            2048
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            next_generation_batch_size(4096, 4096, 128, Duration::from_millis(1)),
            4096
        );
    }

    /// LLM: Regression test: verify that batch adjustments keep dispatch amortized and respect
    /// LLM: worker capacity. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn batch_adjustments_keep_dispatch_amortized_and_respect_worker_capacity() {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use std::time::Duration;
        let slow = Duration::from_secs(1);
        assert_eq!(next_generation_batch_size(1024, 4096, 1024, slow), 512);
        assert_eq!(next_generation_batch_size(512, 4096, 512, slow), 512);
        assert_eq!(
            next_generation_batch_size(512, 4096, 512, Duration::ZERO),
            1024
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            next_generation_batch_size(1024, 1024, 1024, Duration::ZERO),
            1024
        );
        assert_eq!(
            next_generation_batch_size(1024, 4096, 1024, Duration::from_millis(10)),
            1024
        );
    }

    /// LLM: Regression test: verify that history filter preserves first derivations in large
    /// LLM: batches. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn history_filter_preserves_first_derivations_in_large_batches() {
        // LLM: Prepare `row` for the following computation; it stores the row for this stage.
        let row = |label| Configuration::new(vec![(label, 1)]).unwrap();
        let mut history: BTreeSet<_> = (0u32..4096).map(row).collect();
        // LLM: Include known rows and two copies of every new row across job groups.

        let jobs = (0..3)
            .map(|job| {
                // LLM: Update the current working state before continuing this operation.
                (0..1024)
                    .map(|index| {
                        // LLM: Prepare `label` for the following computation; it stores the
                        // LLM: label for this stage.
                        let label = if job == 0 { index } else { 4096 + index };
                        let configuration = row(label);
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
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
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let fresh = new_candidates(jobs, &mut history, &cancellation).unwrap();
        assert_eq!(fresh.len(), 1024);
        assert_eq!(history.len(), 5120);
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for (index, candidate) in fresh.iter().enumerate() {
            assert_eq!(candidate.configuration, row(4096 + index as u32));
            assert_eq!(
                candidate.combination.as_ref().unwrap().union_parts,
                (1, index)
            );
        }
        cancellation.cancel();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(new_candidates(Vec::new(), &mut history, &cancellation).is_err());
    }
}
