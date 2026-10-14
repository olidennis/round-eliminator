//! LLM: Indexed CPU work with ordered results and progress on the calling thread.
//! LLM: Native callers share Rayon's bounded global pool. Browser wasm stays serial:
//! LLM: the existing request worker provides isolation and termination on cancellation.

use crate::computation::{Cancellation, Cancelled};

/// LLM: Return the available algorithm worker count: the bounded Rayon pool on native targets
/// LLM: and one on wasm. Used by scheduling heuristics, not as a mathematical search limit.
pub(crate) fn worker_count() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Update the current working state before continuing this operation.
    {
        rayon::current_num_threads()
    }
    #[cfg(target_arch = "wasm32")]
    // LLM: Update the current working state before continuing this operation.
    {
        1
    }
}

/// LLM: Check whether the caller can coordinate work on the native pool without waiting inside
/// LLM: one of its workers. Used to avoid nested blocking and keep wasm execution serial.
pub(crate) fn can_parallelize() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Update the current working state before continuing this operation.
    {
        worker_count() > 1 && rayon::current_thread_index().is_none()
    }
    #[cfg(target_arch = "wasm32")]
    // LLM: Update the current working state before continuing this operation.
    {
        false
    }
}

/// LLM: Run indexed work and return results in index order, reporting increasing completion
/// LLM: counts on the caller. Shared by diagram comparisons and maximization
/// LLM: generation/pruning.
pub(crate) fn map_with_progress<T: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    progress: impl FnMut(usize) -> Result<(), Cancelled>,
) -> Result<Vec<T>, Cancelled> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    map_with_progress_with_minimum(count, cancellation, parallel, 1, work, progress)
}

/// LLM: Run the same ordered map with a minimum task grain to amortize dispatch. Native
/// LLM: workers batch results; wasm and small maps run serially, with cooperative cancellation
/// LLM: in both paths.
pub(crate) fn map_with_progress_with_minimum<T: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    minimum: usize,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    mut progress: impl FnMut(usize) -> Result<(), Cancelled>,
) -> Result<Vec<T>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if parallel && count > 1 && can_parallelize() {
        // LLM: Do not block a Rayon worker waiting for its own pool. The coordinator
        // LLM: normally runs in the server's blocking executor or a direct caller.

        return rayon::in_place_scope(|scope| {
            // LLM: Prepare the local constants, types, or helper imports used by the following
            // LLM: work.
            use rayon::prelude::*;

            // LLM: Update the current working state before continuing this operation.
            let (sender, receiver) = std::sync::mpsc::sync_channel(
                rayon::current_num_threads().saturating_mul(2).max(1),
            );
            let work = &work;
            // LLM: Keep many stealable ranges per worker without letting tiny
            // LLM: jobs split down to one channel message per item. Small maps
            // LLM: retain single-item ranges (for example, global pruning jobs).

            let minimum = count
                .div_ceil(worker_count().saturating_mul(16))
                .clamp(1, 16)
                .max(minimum);
            // LLM: Rayon splits and steals ranges as workers become available. Fold
            // LLM: results into bounded batches instead of allocating one scoped job
            // LLM: and sending one synchronous-channel message for each tiny item.
            // LLM: The cap also keeps progress responsive on expensive workloads.

            scope.spawn(move |_| {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                (0..count)
                    .into_par_iter()
                    .with_min_len(minimum)
                    .with_max_len(128.max(minimum))
                    .fold(Vec::new, |mut batch, index| {
                        let result = cancellation.check().and_then(|()| work(index));
                        batch.push((index, result));
                        batch
                    })
                    .for_each(|batch| {
                        if !batch.is_empty() {
                            let _ = sender.send(batch);
                        }
                    });
            });
            // LLM: Collect the computed values, preserving their association with the
            // LLM: requested work items.
            let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
            let mut completed = 0;
            // LLM: Receive bounded worker batches on the caller and place each result back at
            // LLM: its original index.
            for batch in receiver {
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for (index, result) in batch {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    match result {
                        Ok(value) => results[index] = Some(value),
                        Err(error) => {
                            cancellation.cancel();
                            return Err(error);
                        }
                    }
                    // LLM: Update the current working state before continuing this operation.
                    completed += 1;
                }
                // LLM: If the caller cancels or rejects a progress update, request that all
                // LLM: outstanding workers stop too.
                if let Err(error) = progress(completed) {
                    cancellation.cancel();
                    return Err(error);
                }
            }
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            Ok(results
                .into_iter()
                .map(|result| result.expect("worker result"))
                .collect())
        });
    }
    #[cfg(target_arch = "wasm32")]
    // LLM: The wasm path ignores parallel scheduling hints and executes the same indexed work
    // LLM: serially.
    let _ = (parallel, minimum);
    let mut results = Vec::with_capacity(count);
    for index in 0..count {
        cancellation.check()?;
        results.push(work(index)?);
        progress(index + 1)?;
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LLM: Regression test: verify that results are ordered and progress can cancel. Used by
    /// LLM: the test suite to protect this behavior while implementations change.
    #[test]
    fn results_are_ordered_and_progress_can_cancel() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let mut completed = Vec::new();
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let results = map_with_progress(
            32,
            &cancellation,
            true,
            |i| Ok(i * i),
            |n| {
                completed.push(n);
                Ok(())
            },
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(results, (0..32).map(|i| i * i).collect::<Vec<_>>());
        assert_eq!(completed.last(), Some(&32));
        assert!(completed.windows(2).all(|pair| pair[0] < pair[1]));
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            map_with_progress(32, &cancellation, true, Ok, |_| {
                cancellation.cancel();
                cancellation.check()
            })
            .is_err()
        );
    }

    /// LLM: Regression test: verify that batched work keeps progress on the caller and can
    /// LLM: cancel large jobs. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn batched_work_keeps_progress_on_the_caller_and_can_cancel_large_jobs() {
        // LLM: Prepare `caller` for the following computation; it stores the caller for this
        // LLM: stage.
        let caller = std::thread::current().id();
        let cancellation = Cancellation::default();
        let mut completed = 0;
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = map_with_progress(
            4096,
            &cancellation,
            true,
            |i| Ok(i),
            |n| {
                assert_eq!(std::thread::current().id(), caller);
                assert!(n > completed);
                completed = n;
                cancellation.check()
            },
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(result, (0..4096).collect::<Vec<_>>());
        assert_eq!(completed, 4096);

        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        assert!(
            map_with_progress(100_000, &cancellation, true, Ok, |_| {
                cancellation.cancel();
                cancellation.check()
            },)
            .is_err()
        );
    }
}
