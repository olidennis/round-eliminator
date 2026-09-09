//! Parallel independent attempts; all publication and GUI callbacks stay on
//! the owner thread. Unwind/STOP interrupts and joins every worker.
use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc,
};

#[derive(Default)]
pub(super) struct Summary {
    pub attempts: usize,
    pub limited: usize,
    pub touched: BTreeSet<usize>,
    pub incomplete: bool,
}

enum Message {
    Started(usize, usize),
    Progress(String, usize, usize),
    Result(usize, Result<Option<Certificate>, String>),
}

pub(super) fn run(
    p: &Problem,
    re2: Option<&Re2Target>,
    candidates: &[Vec<[Label; 2]>],
    schedules: &[Vec<Vec<Step>>],
    options: &Options,
    deadline: Instant,
    eh: &mut EventHandler,
    mut publish: impl FnMut(usize, Certificate, &Summary) -> bool,
) -> Result<Summary, String> {
    let mut jobs = vec![];
    // Interleave candidates so every candidate sees cheap recipes early.
    for r in 0..schedules.iter().map(Vec::len).max().unwrap_or(0) {
        for (i, schedule) in schedules.iter().enumerate() {
            if r < schedule.len() {
                jobs.push((i, r));
            }
        }
    }
    if jobs.is_empty() {
        return Ok(Summary::default());
    }
    let workers = if options.threads == 0 {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(4)
    } else {
        options.threads
    }
    .min(jobs.len());
    eh.notify("Reversible edges: parallel attempt workers", workers, 0);
    let cancelled = Arc::new(AtomicBool::new(false));
    let solved: Vec<_> = candidates.iter().map(|_| AtomicBool::new(false)).collect();
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    let names: BTreeMap<_, _> = p.mapping_label_text.iter().cloned().collect();
    std::thread::scope(|scope| {
        struct Stop(Arc<AtomicBool>);
        impl Drop for Stop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _stop = Stop(cancelled.clone());
        for _ in 0..workers {
            let tx = tx.clone();
            let cancelled = cancelled.clone();
            let next = &next;
            let solved = &solved;
            let jobs = &jobs;
            let names = &names;
            scope.spawn(move || {
                let progress = tx.clone();
                let mut events = EventHandler::with(move |(s, a, b)| {
                    let _ = progress.send(Message::Progress(s, a, b));
                })
                .with_cancellation(cancelled.clone())
                .with_worker_limit(1);
                loop {
                    if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
                        break;
                    }
                    let j = next.fetch_add(1, Ordering::Relaxed);
                    if j >= jobs.len() {
                        break;
                    }
                    let (i, r) = jobs[j];
                    if solved[i].load(Ordering::Acquire) {
                        continue;
                    }
                    if tx.send(Message::Started(i, r)).is_err() {
                        break;
                    }
                    let budget = Budget {
                        options,
                        deadline: deadline
                            .min(Instant::now() + Duration::from_millis(options.attempt_ms)),
                    };
                    // Catch here, before scope replaces the payload with
                    // "a scoped thread panicked". Send the error immediately;
                    // the owner cancels and joins the other attempt workers.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        relaxation(p, &candidates[i]).and_then(|q| {
                            attempt_target(
                                p,
                                &q,
                                &candidates[i],
                                &schedules[i][r],
                                re2,
                                &budget,
                                &mut events,
                            )
                        })
                    }))
                    .unwrap_or_else(|payload| {
                        let added = candidates[i].iter().map(|[a, b]| {
                            format!("{} {}", names.get(a).cloned().unwrap_or_else(|| a.to_string()),
                                names.get(b).cloned().unwrap_or_else(|| b.to_string()))
                        }).collect::<Vec<_>>().join(", ");
                        Err(format!("{} reversible-edge attempt panicked while testing {added} (recipe {}: {:?}): {}",
                            if re2.is_some() { "RE²" } else { "Direct" },
                            r + 1, schedules[i][r], panic_message(payload)))
                    });
                    if tx.send(Message::Result(i, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut stats = Summary::default();
        loop {
            if eh.is_cancelled() {
                return Err(CANCELLED.into());
            }
            if Instant::now() >= deadline {
                stats.incomplete = true;
                break;
            }
            match rx.recv_timeout(Duration::from_millis(30)) {
                Ok(Message::Started(i, r)) => {
                    stats.attempts += 1;
                    stats.touched.insert(i);
                    let description = candidates[i]
                        .iter()
                        .map(|[a, b]| format!("{} {}", names[a], names[b]))
                        .collect::<Vec<_>>()
                        .join(", ");
                    eh.notify(
                        format!("Reversible edges: testing {description} (recipe {})", r + 1),
                        i + 1,
                        candidates.len(),
                    );
                }
                Ok(Message::Progress(s, a, b)) => eh.notify(s, a, b),
                Ok(Message::Result(i, Ok(Some(c)))) => {
                    if !solved[i].swap(true, Ordering::AcqRel) && !publish(i, c, &stats) {
                        stats.incomplete = true;
                        break;
                    }
                    if solved.iter().all(|s| s.load(Ordering::Acquire)) {
                        break;
                    }
                }
                Ok(Message::Result(_, Ok(None))) => {}
                Ok(Message::Result(_, Err(e))) if e == LIMIT => {
                    stats.limited += 1;
                }
                Ok(Message::Result(_, Err(e))) => return Err(e),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eh.notify("Reversible edges: searches running", stats.attempts, 0)
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        stats.incomplete |= next.load(Ordering::Relaxed) < jobs.len()
            && !solved.iter().all(|s| s.load(Ordering::Acquire));
        Ok(stats)
    })
}
