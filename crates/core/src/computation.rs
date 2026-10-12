//! Transport-independent cancellation and progress for synchronous CPU work.

use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use ts_rs::TS;

/// In order to allow the caller to interrupt work, we use Computation, which represents some computation being done.
/// Cancellation holds a boolean denoting whether the computation has been interrupted.
/// Cancellation is also held by the caller, and can call the cancel function.
/// We store a handler where we send the status (Progress) that has been done.
pub struct Computation<'a> {
    pub cancellation: Cancellation,
    report: &'a mut dyn FnMut(Progress),
}

impl<'a> Computation<'a> {
    pub fn new(cancellation: Cancellation, report: &'a mut dyn FnMut(Progress)) -> Self {
        Self {
            cancellation,
            report,
        }
    }

    pub fn report(&mut self, phase: Phase, iterations: u32) -> Result<(), Cancelled> {
        self.cancellation.check()?;
        (self.report)(Progress { phase, iterations });
        // A consumer may cancel in response to an update.
        self.cancellation.check()
    }
}

/// Progress stores what we are doing, and some other special parameters, like how many steps we did
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Progress {
    pub phase: Phase,
    pub iterations: u32,
}

/// These are all the possible things we could be doing
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
}

/// Cancellation is just a bool that we can update atomically
/// In case of cancel, we don't kill threads.
/// The cancel is cooperative: workers periodically check whether they should stop
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Computation cancelled.")
    }
}

impl std::error::Error for Cancelled {}
