//! Benchmark-only overlap using reserved generation workers in one Rayon pool.

use crate::computation::{Cancellation, Cancelled};
use std::sync::{atomic::{AtomicUsize, Ordering}, mpsc::sync_channel};

pub(crate) fn visit<T: Send>(
    count: usize, minimum: usize, cancellation: &Cancellation,
    work: impl Fn(usize) -> Result<T, Cancelled> + Sync,
    mut consume: impl FnMut(Vec<T>, usize) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    let capacity = rayon::current_num_threads();
    let generators = if crate::four_flags::enabled("pipe8") { capacity.saturating_sub(2).max(1) }
        else { (capacity / 2).max(1) };
    // At least one worker remains available to prune while producers send to
    // this bounded channel. No extra worker pool or blocked-pool deadlock.
    let next = AtomicUsize::new(0);
    let chunk = minimum.clamp(8, 256);
    rayon::in_place_scope(|scope| {
        let (sender, receiver) = sync_channel(2 * generators);
        for _ in 0..generators {
            let sender = sender.clone(); let work = &work; let next = &next;
            scope.spawn(move |_| {
                loop {
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
        drop(sender);
        let mut completed = 0;
        for batch in receiver {
            let length = batch.len();
            let batch: Result<Vec<_>, _> = batch.into_iter().collect();
            let result = batch.and_then(|values| {
                completed += length;
                consume(values, completed)
            });
            if let Err(error) = result { cancellation.cancel(); return Err(error); }
        }
        cancellation.check()
    })
}
