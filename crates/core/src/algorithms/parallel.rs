//! Indexed CPU work with ordered results and progress on the calling thread.
//!
//! Native callers share Rayon's bounded global pool. Browser wasm stays serial:
//! the existing request worker provides isolation and termination on cancellation.

use crate::computation::{Cancellation, Cancelled};

pub(crate) fn map_with_progress<T: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    mut progress: impl FnMut(usize) -> Result<(), Cancelled>,
) -> Result<Vec<T>, Cancelled> {
    cancellation.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    if parallel
        && count > 1
        && rayon::current_num_threads() > 1
        && rayon::current_thread_index().is_none()
    {
        // Do not block a Rayon worker waiting for its own pool. The coordinator
        // normally runs in the server's blocking executor or a direct caller.
        return rayon::in_place_scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(
                rayon::current_num_threads().saturating_mul(2).max(1),
            );
            let work = &work;
            for index in 0..count {
                let sender = sender.clone();
                scope.spawn(move |_| {
                    let result = cancellation.check().and_then(|()| work(index));
                    let _ = sender.send((index, result));
                });
            }
            drop(sender);
            let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
            for (completed, (index, result)) in receiver.into_iter().enumerate() {
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
            cancellation.check()?;
            Ok(results
                .into_iter()
                .map(|result| result.expect("worker result"))
                .collect())
        });
    }
    #[cfg(target_arch = "wasm32")]
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
        assert_eq!(completed, (1..=32).collect::<Vec<_>>());
        assert!(
            map_with_progress(32, &cancellation, true, Ok, |_| {
                cancellation.cancel();
                cancellation.check()
            })
            .is_err()
        );
    }
}
