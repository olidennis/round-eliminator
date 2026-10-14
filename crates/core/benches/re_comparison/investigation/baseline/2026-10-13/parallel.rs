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
            // LLM: Update the current working state before continuing this operation.
            let (sender, receiver) = std::sync::mpsc::sync_channel(
                rayon::current_num_threads().saturating_mul(2).max(1),
            );
            let work = &work;
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for index in 0..count {
                let sender = sender.clone();
                scope.spawn(move |_| {
                    let result = cancellation.check().and_then(|()| work(index));
                    let _ = sender.send((index, result));
                });
            }
            drop(sender);
            // LLM: Collect the computed values, preserving their association with the
            // LLM: requested work items.
            let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
            for (completed, (index, result)) in receiver.into_iter().enumerate() {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                match result.and_then(|value| {
                    progress(completed + 1)?;
                    Ok(value)
                }) {
                    Ok(value) => results[index] = Some(value),
                    Err(error) => {
                        cancellation.cancel();
                        return Err(error);
                    }
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
    // LLM: Prepare `_` for the following computation; it stores the   for this stage.
    let _ = parallel;
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
        assert_eq!(completed, (1..=32).collect::<Vec<_>>());
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
}
