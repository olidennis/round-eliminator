//! Indexed CPU work with ordered results and progress on the calling thread.
//!
//! Native callers share Rayon's bounded global pool. Browser wasm stays serial:
//! the existing request worker provides isolation and termination on cancellation.

use crate::computation::{Cancellation, Cancelled};

pub(crate) fn worker_count() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::current_num_threads()
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

pub(crate) fn can_parallelize() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        worker_count() > 1 && rayon::current_thread_index().is_none()
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

/// Progress receives increasing completed-item counts on the caller. Native
/// results may arrive in batches, so callbacks need not occur for every item.
pub(crate) fn map_with_progress<T: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    progress: impl FnMut(usize) -> Result<(), Cancelled>,
) -> Result<Vec<T>, Cancelled> {
    map_with_progress_with_minimum(count, cancellation, parallel, 1, work, progress)
}

/// A larger minimum amortizes dispatch for computations with very cheap items.
/// It controls task splitting only; each item still checks cancellation.
pub(crate) fn map_with_progress_with_minimum<T: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    minimum: usize,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    mut progress: impl FnMut(usize) -> Result<(), Cancelled>,
) -> Result<Vec<T>, Cancelled> {
    cancellation.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    if parallel && count > 1 && can_parallelize() {
        // Do not block a Rayon worker waiting for its own pool. The coordinator
        // normally runs in the server's blocking executor or a direct caller.
        return rayon::in_place_scope(|scope| {
            use rayon::prelude::*;

            let (sender, receiver) = std::sync::mpsc::sync_channel(
                rayon::current_num_threads().saturating_mul(2).max(1),
            );
            let work = &work;
            // Keep many stealable ranges per worker without letting tiny
            // jobs split down to one channel message per item. Small maps
            // retain single-item ranges (for example, global pruning jobs).
            let minimum = count
                .div_ceil(worker_count().saturating_mul(16))
                .clamp(1, 16)
                .max(minimum);
            // Rayon splits and steals ranges as workers become available. Fold
            // results into bounded batches instead of allocating one scoped job
            // and sending one synchronous-channel message for each tiny item.
            // The cap also keeps progress responsive on expensive workloads.
            scope.spawn(move |_| {
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
            let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
            let mut completed = 0;
            for batch in receiver {
                for (index, result) in batch {
                    cancellation.check()?;
                    match result {
                        Ok(value) => results[index] = Some(value),
                        Err(error) => {
                            cancellation.cancel();
                            return Err(error);
                        }
                    }
                    completed += 1;
                }
                if let Err(error) = progress(completed) {
                    cancellation.cancel();
                    return Err(error);
                }
            }
            cancellation.check()?;
            Ok(results
                .into_iter()
                .map(|result| result.expect("worker result"))
                .collect())
        });
    }
    #[cfg(target_arch = "wasm32")]
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

    #[test]
    fn results_are_ordered_and_progress_can_cancel() {
        let cancellation = Cancellation::default();
        let mut completed = Vec::new();
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
        assert_eq!(results, (0..32).map(|i| i * i).collect::<Vec<_>>());
        assert_eq!(completed.last(), Some(&32));
        assert!(completed.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            map_with_progress(32, &cancellation, true, Ok, |_| {
                cancellation.cancel();
                cancellation.check()
            })
            .is_err()
        );
    }

    #[test]
    fn batched_work_keeps_progress_on_the_caller_and_can_cancel_large_jobs() {
        let caller = std::thread::current().id();
        let cancellation = Cancellation::default();
        let mut completed = 0;
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
        assert_eq!(result, (0..4096).collect::<Vec<_>>());
        assert_eq!(completed, 4096);

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
