//! Diagnostic hooks inserted only into temporary copies of the libraries.
//! Thread-local counters avoid an atomic/cache-line contention per operation.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

const N: usize = 43;
const NAMES: [&str; N] = [
    "pairs",
    "union_choices",
    "alignments",
    "constructed",
    "local_duplicates",
    "early_known_duplicates",
    "late_global_duplicates",
    "matches_local",
    "matches_global",
    "flows",
    "union_pairs_considered",
    "swap_rejected",
    "local_survivors",
    "global_accepted",
    "frontiers",
    "shared_duplicates",
    "old_alignment_survivors",
    "union_pruned",
    "parallel_eligible_batches",
    "serial_eligible_batches",
    "generation_wall_ns",
    "merge_wall_ns",
    "combine_sum_wall_ns",
    "pruning_sum_wall_ns",
    "prefilter_candidates",
    "prefilter_rejected",
    "prefilter_batches",
    "prefilter_chunks",
    "prefilter_wall_ns",
    "commit_wall_ns",
    "dedup_wall_ns",
    "pruning_dispatch_wall_ns",
    "mask_apply_wall_ns",
    "cache_checks",
    "cache_rejections",
    "scan_checks",
    "scan_rejections",
    "removal_checks",
    "new_candidate_checks",
    "dedup_clones",
    "history_queries",
    "refresh_checks",
    "refresh_wall_ns",
];
struct Local {
    counters: [u64; N],
    combining: bool,
}
impl Default for Local {
    fn default() -> Self {
        Self {
            counters: [0; N],
            combining: false,
        }
    }
}
impl Drop for Local {
    fn drop(&mut self) {
        let mut total = TOTAL.lock().unwrap();
        for (all, count) in total.iter_mut().zip(self.counters) {
            *all += count;
        }
    }
}
thread_local! { static LOCAL: RefCell<Local> = RefCell::new(Local::default()); }
static TOTAL: Mutex<[u64; N]> = Mutex::new([0; N]);

#[inline]
pub fn inc(index: usize) {
    add(index, 1);
}
#[inline]
pub fn add(index: usize, value: u64) {
    LOCAL.with(|local| local.borrow_mut().counters[index] += value);
}
#[inline]
pub fn matched() {
    LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let index = if local.combining { 7 } else { 8 };
        local.counters[index] += 1;
    });
}
fn flush() {
    LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let mut total = TOTAL.lock().unwrap();
        for (all, count) in total.iter_mut().zip(&mut local.counters) {
            *all += *count;
            *count = 0;
        }
    });
}
pub fn snapshot() -> BTreeMap<&'static str, u64> {
    flush();
    NAMES.into_iter().zip(*TOTAL.lock().unwrap()).collect()
}
pub fn reset() {
    LOCAL.with(|local| *local.borrow_mut() = Local::default());
    *TOTAL.lock().unwrap() = [0; N];
}
pub struct Guard {
    start: Instant,
    index: usize,
    previous: Option<bool>,
}
pub fn timer(index: usize) -> Guard {
    Guard {
        start: Instant::now(),
        index,
        previous: None,
    }
}
pub fn combining() -> Guard {
    let previous = LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let previous = local.combining;
        local.combining = true;
        previous
    });
    Guard {
        start: Instant::now(),
        index: 22,
        previous: Some(previous),
    }
}
pub fn pruning() -> Guard {
    let previous = LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let previous = local.combining;
        local.combining = false;
        previous
    });
    Guard {
        start: Instant::now(),
        index: 23,
        previous: Some(previous),
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        add(self.index, self.start.elapsed().as_nanos() as u64);
        if let Some(previous) = self.previous {
            LOCAL.with(|local| local.borrow_mut().combining = previous);
            flush();
        }
    }
}

/// Controlled experiments, never part of the production implementation.
pub fn enabled(name: &str) -> bool {
    static FLAGS: OnceLock<String> = OnceLock::new();
    FLAGS
        .get_or_init(|| std::env::var("RE_PROFILE_VARIANT").unwrap_or_default())
        .split('+')
        .any(|flag| flag == name)
}
