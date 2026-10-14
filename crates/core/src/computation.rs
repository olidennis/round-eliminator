//! LLM: Transport-independent cancellation and progress for synchronous CPU work.

use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use ts_rs::TS;

/// LLM: The context for synchronous CPU work: a shared cooperative cancellation flag and a
/// LLM: borrowed progress callback. Algorithms report through it on the caller; server and
/// LLM: wasm adapters control event delivery.
pub struct Computation<'a> {
    pub cancellation: Cancellation,
    report: &'a mut dyn FnMut(Progress),
}

impl<'a> Computation<'a> {
    /// LLM: Bind a shared cancellation handle to the caller's progress consumer. Used by APIs
    /// LLM: and direct algorithm entry points.
    pub fn new(cancellation: Cancellation, report: &'a mut dyn FnMut(Progress)) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            cancellation,
            report,
        }
    }

    /// LLM: Publish the phase and iteration count when no finite work total is known. Used by
    /// LLM: SAT searches and preparation stages.
    pub fn report(&mut self, phase: Phase, iterations: u32) -> Result<(), Cancelled> {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.report_progress(Progress {
            phase,
            iterations,
            work: None,
        })
    }

    /// LLM: Publish completed/total counts for one current pass. Used by maximization and
    /// LLM: transformations whose work items can be counted.
    pub fn report_work(
        &mut self,
        phase: Phase,
        iterations: u32,
        work: WorkProgress,
    ) -> Result<(), Cancelled> {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.report_progress(Progress {
            phase,
            iterations,
            work: Some(work),
        })
    }

    /// LLM: Check interruption, invoke the progress consumer, and check again for cancellation
    /// LLM: caused by that callback. Used by both progress helpers.
    pub(crate) fn report_progress(&mut self, progress: Progress) -> Result<(), Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.cancellation.check()?;
        (self.report)(progress);
        // LLM: A consumer may cancel in response to an update.

        self.cancellation.check()
    }
}

/// LLM: A snapshot of the current computation phase and phase-dependent iteration count,
/// LLM: optionally with a known work total. This reports execution status, separately from
/// LLM: proved coloring bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Progress {
    pub phase: Phase,
    pub iterations: u32,
    /// LLM: Absent when the remaining work is unknown, for example SAT search.

    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub work: Option<WorkProgress>,
}

/// LLM: Completed work and its total within one identified pass. Counts describe actual items,
/// LLM: not elapsed time or a prediction of the whole computation; another pass may restart at
/// LLM: zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct WorkProgress {
    #[ts(type = "number")]
    pub completed: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub pass: u32,
}

/// LLM: The stages exposed by computations to either transport. The variants describe current
/// LLM: work, not a fixed sequence that every algorithm must follow.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Queued,
    Parsing,
    Enumerating,
    Encoding,
    Solving,
    Verifying,
    Preparing,
    Comparing,
    Reducing,
    Maximizing,
    Transforming,
    ClosingActive,
    ClosingPassive,
    Cleaning,
}

/// LLM: A clonable shared atomic stop flag. Setting it requests cooperative termination;
/// LLM: algorithms and SAT backends must observe it and return cancellation rather than an
/// LLM: unsolvability proof.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    /// LLM: Set the shared stop flag permanently for this computation. Used by transport
    /// LLM: disconnection, user interruption, and failed workers.
    pub fn cancel(&self) {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.0.store(true, Ordering::Relaxed);
    }

    /// LLM: Read the shared stop request without blocking. Used by API result classification
    /// LLM: and backend termination callbacks.
    pub fn is_cancelled(&self) -> bool {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.0.load(Ordering::Relaxed)
    }

    /// LLM: Return the cancellation marker when interruption was requested. Used at
    /// LLM: cooperative checkpoints throughout parsing and CPU work.
    pub fn check(&self) -> Result<(), Cancelled> {
        // LLM: Keep interruption separate from a negative proof or ordinary computation
        // LLM: failure.
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

/// LLM: The marker error for interrupted work. It contains no mathematical result and must
/// LLM: never be interpreted as UNSAT or a completed maximum.
#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    /// LLM: Write the stored explanation through Rust's Display formatter. Used when
    /// LLM: presenting or converting this error to a human-readable message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // LLM: Update the current working state before continuing this operation.
        f.write_str("Computation cancelled.")
    }
}

impl std::error::Error for Cancelled {}
