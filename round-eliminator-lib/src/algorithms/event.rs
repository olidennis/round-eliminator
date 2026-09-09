use chrono::{NaiveTime, Utc,Duration};

use crate::serial::SendOnlyNonWasm;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};


pub struct EventHandler<'a> {
    tx: Option<BoxedEventFunc<'a>>,
    last_msg : String,
    last_time : NaiveTime,
    cancellation: Option<Arc<AtomicBool>>,
    worker_limit: Option<usize>,
}

pub trait EventFunc: FnMut((std::string::String, usize, usize),) + SendOnlyNonWasm {}
type BoxedEventFunc<'a> = Box<dyn EventFunc + 'a>;

impl<T> EventFunc for T where T : FnMut((std::string::String, usize, usize),) + SendOnlyNonWasm {}

impl<'a> EventHandler<'a> {
    pub fn null() -> Self {
        Self { tx: None, last_msg : String::new(), last_time : Utc::now().time() - Duration::seconds(1), cancellation: None, worker_limit: None }
    }

    pub fn with<T>(f: T) -> Self
    where
        T: EventFunc + 'a,
    {
        Self {
            tx: Some(Box::new(f)),
            last_msg : String::new(), last_time : Utc::now().time() - Duration::seconds(1),
            cancellation: None,
            worker_limit: None,
        }
    }

    pub(super) fn with_cancellation(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub(super) fn with_worker_limit(mut self, workers: usize) -> Self {
        self.worker_limit = Some(workers.max(1));
        self
    }

    pub(super) fn worker_limit(&self) -> Option<usize> {
        self.worker_limit
    }

    pub(super) fn cancellation_token(&self) -> Option<Arc<AtomicBool>> {
        self.cancellation.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.as_ref().is_some_and(|c| c.load(Ordering::Relaxed))
    }

    pub fn notify<S: AsRef<str>>(&mut self, s: S, x: usize, t: usize) {
        let s = s.as_ref();
        if let Some(tx) = self.tx.as_mut() {
            if self.last_msg != s || (Utc::now().time() - self.last_time).num_milliseconds() > 300 {
                self.last_msg = String::from(s);
                self.last_time = Utc::now().time();
                tx((s.to_string(), x, t));
            }
        }
    }
}
