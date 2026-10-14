//! LLM: Diagnostic hooks inserted only into temporary copies of the libraries.
//! LLM: Thread-local counters avoid an atomic/cache-line contention per operation.

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
/// LLM: Thread-local profiling counters buffered between flushes. The benchmark aggregates
/// LLM: them without placing a shared atomic operation in every hot algorithmic step.
struct Local {
    counters: [u64; N],
    combining: bool,
}
impl Default for Local {
    /// LLM: Create the empty initial local state. Used before collecting values or admitting
    /// LLM: computed rows.
    fn default() -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            counters: [0; N],
            combining: false,
        }
    }
}
impl Drop for Local {
    /// LLM: Flush thread-local profiling counts when the profiling buffer is dropped. Used to
    /// LLM: retain worker totals when benchmark threads exit.
    fn drop(&mut self) {
        // LLM: Count the current pass's work items so reported completion can advance
        // LLM: monotonically.
        let mut total = TOTAL.lock().unwrap();
        for (all, count) in total.iter_mut().zip(self.counters) {
            *all += count;
        }
    }
}
thread_local! { static LOCAL: RefCell<Local> = RefCell::new(Local::default()); }
static TOTAL: Mutex<[u64; N]> = Mutex::new([0; N]);

/// LLM: Increment one thread-local profiling counter. Used by temporary benchmark hooks to
/// LLM: avoid a shared atomic increment at each hot step.
#[inline]
pub fn inc(index: usize) {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    add(index, 1);
}
/// LLM: Accumulate an amount in one thread-local profiling counter. Used by benchmark
/// LLM: instrumentation before aggregation.
#[inline]
pub fn add(index: usize, value: u64) {
    // LLM: Update the current working state before continuing this operation.
    LOCAL.with(|local| local.borrow_mut().counters[index] += value);
}
/// LLM: Record a containment/matching observation in the benchmark counters. Used by
/// LLM: instrumented maximization kernels.
#[inline]
pub fn matched() {
    // LLM: Update the current working state before continuing this operation.
    LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let index = if local.combining { 7 } else { 8 };
        local.counters[index] += 1;
    });
}
/// LLM: Merge buffered thread-local counters into shared benchmark totals and clear the
/// LLM: buffer. Used between profiling samples and at worker exit.
fn flush() {
    // LLM: Update the current working state before continuing this operation.
    LOCAL.with(|local| {
        // LLM: Prepare `local` for the following computation; it stores the local for this
        // LLM: stage.
        let mut local = local.borrow_mut();
        let mut total = TOTAL.lock().unwrap();
        for (all, count) in total.iter_mut().zip(&mut local.counters) {
            *all += *count;
            *count = 0;
        }
    });
}
/// LLM: Read the aggregated benchmark counters. Used when reporting measured generation,
/// LLM: matching, and pruning work.
pub fn snapshot() -> BTreeMap<&'static str, u64> {
    // LLM: Deliver retained updates before returning, preserving the latest proof even after a
    // LLM: burst.
    flush();
    NAMES.into_iter().zip(*TOTAL.lock().unwrap()).collect()
}
/// LLM: Clear benchmark instrumentation between samples. Used to prevent previous runs from
/// LLM: contaminating reported totals.
pub fn reset() {
    // LLM: Update the current working state before continuing this operation.
    LOCAL.with(|local| *local.borrow_mut() = Local::default());
    *TOTAL.lock().unwrap() = [0; N];
}
/// LLM: A scoped benchmark timer that records elapsed work when dropped. Its category
/// LLM: distinguishes combining and pruning costs without changing the production algorithm.
pub struct Guard {
    start: Instant,
    index: usize,
    previous: Option<bool>,
}
/// LLM: Create a scoped benchmark timer for the selected category. Used by temporary
/// LLM: instrumentation to distinguish generation and pruning costs.
pub fn timer(index: usize) -> Guard {
    // LLM: Update the current working state before continuing this operation.
    Guard {
        start: Instant::now(),
        index,
        previous: None,
    }
}
/// LLM: Create the instrumentation guard for pair combination. Used only in benchmark-patched
/// LLM: kernels, not production execution.
pub fn combining() -> Guard {
    // LLM: Record prior work so the next pass need only visit newly relevant parent pairs.
    let previous = LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let previous = local.combining;
        local.combining = true;
        previous
    });
    // LLM: Update the current working state before continuing this operation.
    Guard {
        start: Instant::now(),
        index: 22,
        previous: Some(previous),
    }
}
/// LLM: Create the instrumentation guard for global pruning. Used only in benchmark-patched
/// LLM: kernels.
pub fn pruning() -> Guard {
    // LLM: Record prior work so the next pass need only visit newly relevant parent pairs.
    let previous = LOCAL.with(|local| {
        let mut local = local.borrow_mut();
        let previous = local.combining;
        local.combining = false;
        previous
    });
    // LLM: Update the current working state before continuing this operation.
    Guard {
        start: Instant::now(),
        index: 23,
        previous: Some(previous),
    }
}
impl Drop for Guard {
    /// LLM: Accumulate elapsed time in the guard's profiling category. Used automatically when
    /// LLM: an instrumented benchmark scope ends.
    fn drop(&mut self) {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        add(self.index, self.start.elapsed().as_nanos() as u64);
        if let Some(previous) = self.previous {
            LOCAL.with(|local| local.borrow_mut().combining = previous);
            flush();
        }
    }
}

/// LLM: Check whether the retained experiment has enabled its diagnostic policy. Used only by
/// LLM: benchmark-instrumented copies.
pub fn enabled(name: &str) -> bool {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    static FLAGS: OnceLock<String> = OnceLock::new();
    FLAGS
        .get_or_init(|| std::env::var("RE_PROFILE_VARIANT").unwrap_or_default())
        .split('+')
        .any(|flag| flag == name)
}
