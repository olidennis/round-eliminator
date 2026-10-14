//! LLM: Benchmark-only overlap using reserved generation workers in one Rayon pool.

use crate::computation::{Cancellation, Cancelled};
use std::sync::{atomic::{AtomicUsize, Ordering}, mpsc::sync_channel};

/// LLM: Extend the current enumeration prefix, visit complete choices, and restore it when
/// LLM: backtracking. Used by the enclosing exact traversal or independent fixture oracle.
pub(crate) fn visit<T: Send>(
    count: usize, minimum: usize, cancellation: &Cancellation,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    mut consume: impl FnMut(Vec<T>, usize) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    // LLM: Prepare `capacity` for the following computation; it stores the capacity for this
    // LLM: stage.
    let capacity = rayon::current_num_threads();
    let generators = if crate::four_flags::enabled("pipe8") { capacity.saturating_sub(2).max(1) }
        else { (capacity / 2).max(1) };
    // LLM: At least one worker remains available to prune while producers send to
    // LLM: this bounded channel. No extra worker pool or blocked-pool deadlock.

    let next = AtomicUsize::new(0);
    let chunk = minimum.clamp(8, 256);
    // LLM: Observe cooperative interruption before continuing this unit of work.
    rayon::in_place_scope(|scope| {
        // LLM: Update the current working state before continuing this operation.
        let (sender, receiver) = sync_channel(2 * generators);
        for _ in 0..generators {
            // LLM: Prepare `sender` for the following computation; it stores the sender for
            // LLM: this stage.
            let sender = sender.clone(); let work = &work; let next = &next;
            scope.spawn(move |_| {
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                loop {
                    // LLM: Prepare `start` for the following computation; it stores the start
                    // LLM: for this stage.
                    let start = next.fetch_add(chunk, Ordering::Relaxed);
                    if start >= count { break; }
                    let mut batch = Vec::new();
                    for i in start..(start + chunk).min(count) {
                        batch.push(cancellation.check().and_then(|()| work(i)));
                    }
                    if sender.send(batch).is_err() { break; }
                }
            });
        }
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        drop(sender);
        let mut completed = 0;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for batch in receiver {
            // LLM: Prepare `length` for the following computation; it stores the length for
            // LLM: this stage.
            let length = batch.len();
            let batch: Result<Vec<_>, _> = batch.into_iter().collect();
            let result = batch.and_then(|values| {
                completed += length;
                consume(values, completed)
            });
            if let Err(error) = result { cancellation.cancel(); return Err(error); }
        }
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()
    })
}
