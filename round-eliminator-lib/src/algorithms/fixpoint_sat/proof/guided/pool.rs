//! One scheduler and shared archive; bounded, independently owned SAT jobs.
//! Solvers (and their learned clauses) move only while idle, never while
//! registered for interruption. All event callbacks run on the scheduler.

use super::*;
use std::sync::mpsc;

// Variable credits, not an exact byte/RSS bound. Covers a complete local
// encoding, including feedback goal circuits, while the job is in flight.
const JOB_VARIABLES: usize = 152_000;

pub(crate) struct Settings {
    pub workers: usize,
    pub variables: usize,
}

fn parse_settings(
    threads: Option<&str>,
    variables: Option<&str>,
    available: usize,
) -> Result<Settings, String> {
    let workers = match threads {
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=32).contains(n))
            .ok_or("RE_GUIDED_THREADS must be an integer from 1 to 32")?,
        None => available.saturating_sub(2).clamp(1, 6),
    };
    let variables = match variables {
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|&n| n >= JOB_VARIABLES)
            .ok_or("RE_GUIDED_MAX_VARIABLES must be an integer of at least 152000")?,
        None => 1_500_000,
    };
    Ok(Settings { workers, variables })
}

pub(crate) fn settings(certificate_threads: usize) -> Result<Settings, String> {
    parse_settings(
        std::env::var("RE_GUIDED_THREADS").ok().as_deref(),
        std::env::var("RE_GUIDED_MAX_VARIABLES").ok().as_deref(),
        certificate_threads,
    )
}

// A bounded working set retains complete native solvers. No cold retries:
// after a local work cap a neighborhood is explicitly retired as inconclusive,
// while the independent proof grammar continues without this cap.
const MAX_BRIDGE_SLICES: usize = 16;

struct Bridge<'a> {
    saved: Option<Neighborhood<'a>>,
    seeds: Vec<Vec<Term>>,
    shared: usize,
    cursor: usize,
    slices: usize,
}

struct BridgeResult<'a> {
    retry: Option<Bridge<'a>>,
    found: Option<CertificateSearchOutcome>,
}

impl<'a> Bridge<'a> {
    fn fresh(bank: &Bank, ids: &[usize]) -> Self {
        Self {
            saved: None,
            seeds: ids.iter().map(|&i| bank.tuples[i].clone()).collect(),
            shared: ids.iter().filter(|&&i| i >= bank.inputs).count(),
            cursor: 0,
            slices: 0,
        }
    }

    fn variables(&self) -> usize {
        self.saved
            .as_ref()
            .map_or(0, |j| j.encoding.circuit.next_var as usize)
    }

    fn execute(
        mut self,
        original: &Problem,
        options: &CertificateSearchOptions,
        worker: usize,
        control: &'a SearchControl,
        oracle: &mut NonexistenceOracle,
        eh: &mut EventHandler,
    ) -> Result<BridgeResult<'a>, String> {
        let mut job = if let Some(saved) = self.saved.take() {
            saved
        } else {
            Neighborhood::from_seeds(original, &self.seeds, self.shared, control)?
        };
        // The encoding owns the concrete terms, even if their archive slots
        // rotate while this job is running or waiting for a hot retry.
        self.seeds.clear();
        job.encoding.circuit.worker = worker;
        job.encoding.circuit.variable_limit = Some(JOB_VARIABLES as u32);
        let max_steps = options
            .max_steps
            .unwrap_or(MAX_LOCAL_STEPS)
            .min(MAX_LOCAL_STEPS);
        let local = CertificateSearchOptions {
            // This is a slice, not a growing cold-start budget. Every retry
            // continues the same solver, retaining learned clauses and UNSATs.
            conflict_limit: Some(
                options
                    .conflict_limit
                    .unwrap_or((LOCAL_CONFLICTS as u32) << self.slices.min(2))
                    .min(8_000),
            ),
            ..options.clone()
        };
        let limit = if job.shared == 0 {
            max_steps.min(3)
        } else {
            max_steps
        };
        // A scheduling slice is ONE bounded SAT call, not a loop over all
        // earlier bounds. Grow first, then cycle through unresolved bounds.
        let index = if job.goals.len() < limit {
            Some(job.goals.len())
        } else {
            (0..limit)
                .map(|i| (self.cursor + i) % limit)
                .find(|&i| !job.exhausted[i])
        };
        let Some(index) = index else {
            return Ok(BridgeResult {
                retry: None,
                found: None,
            });
        };
        match job.search(index, oracle, &local, eh) {
            Ok(Some(found)) => {
                return Ok(BridgeResult {
                    retry: None,
                    found: Some(found),
                })
            }
            Err(error) if error == "Proof neighborhood variable budget reached" => {
                eh.notify(
                    "Proof: guided job variable limit (inconclusive)",
                    JOB_VARIABLES,
                    0,
                );
                return Ok(BridgeResult {
                    retry: None,
                    found: None,
                });
            }
            other => {
                other?;
            }
        }
        self.cursor = (index + 1) % limit;
        self.slices += 1;
        let unfinished = job.exhausted.iter().any(|&done| !done);
        let needs_more =
            job.goals.len() < limit || (unfinished && options.conflict_limit.is_none());
        let retry = if needs_more && self.slices < MAX_BRIDGE_SLICES {
            self.saved = Some(job);
            Some(self)
        } else {
            if needs_more {
                eh.notify(
                    "Proof: guided neighborhood retired (inconclusive)",
                    self.slices,
                    MAX_BRIDGE_SLICES,
                );
            }
            None
        };
        Ok(BridgeResult { retry, found: None })
    }
}

enum Work<'a> {
    Bridge(Bridge<'a>),
    Feedback(feedback::Task<'a>),
}

enum Completed<'a> {
    Bridge(BridgeResult<'a>),
    Feedback(feedback::Completed<'a>),
}

enum Message<'a> {
    Event((String, usize, usize)),
    Done {
        worker: usize,
        result: Result<Completed<'a>, String>,
    },
}

fn saved_variables(retries: &VecDeque<Bridge<'_>>) -> usize {
    retries.iter().map(Bridge::variables).sum()
}

fn cached_work<'a>(
    retries: &mut VecDeque<Bridge<'a>>,
    feedback: &mut feedback::Engine<'a>,
    minimum: usize,
    prefer_feedback: bool,
    eh: &mut EventHandler,
) -> Option<Work<'a>> {
    if prefer_feedback {
        if let Some(task) = feedback.next_cached(minimum, eh) {
            return Some(Work::Feedback(task));
        }
    }
    if let Some(pos) = retries.iter().position(|r| r.variables() >= minimum) {
        return retries.remove(pos).map(Work::Bridge);
    }
    feedback.next_cached(minimum, eh).map(Work::Feedback)
}

pub(super) fn run(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    outer: &SearchControl,
    hints: &Receiver<Derivation>,
    default_seed: bool,
    shared: Option<&mpsc::SyncSender<Vec<Term>>>,
) -> Result<CertificateSearchOutcome, String> {
    let local = outer.guided_scope();
    let control = &local;
    control.check()?;
    let max_steps = options
        .max_steps
        .unwrap_or(MAX_LOCAL_STEPS)
        .min(MAX_LOCAL_STEPS);
    let mut oracle = NonexistenceOracle::new(original);
    let mut bank = Bank::new(input_terms(original));
    let mut feedback = feedback::Engine::new(original, control)?;
    if max_steps == 0 || bank.tuples.is_empty() {
        return Ok(CertificateSearchOutcome::Inconclusive { steps: 0 });
    }
    if default_seed {
        if let Some(dag) = super::super::super::seeding::collect(original, eh, control)? {
            let before = bank.tuples.len();
            feedback.seed_default(&dag, control)?;
            if let Some(found) = bank.import_default(dag, &mut oracle, control, eh)? {
                return Ok(found);
            }
            if let Some(shared) = shared {
                for terms in &bank.tuples[before..] {
                    control.check()?;
                    let _ = shared.try_send(terms.clone());
                }
            }
        }
    }
    let workers = control.guided_workers();
    eh.notify("Proof: guided workers", workers, 0);
    eh.notify(
        "Proof: guided shared variable budget",
        control.guided_variable_budget,
        0,
    );
    std::thread::scope(|scope| {
        // Runs before scope's implicit joins, also on a GUI callback panic.
        // Pool-only cancellation must not race an independent worker's result.
        struct Stop<'a>(&'a SearchControl);
        impl Drop for Stop<'_> {
            fn drop(&mut self) {
                self.0.stop();
            }
        }
        let _stop = Stop(control);
        let (tx, rx) = mpsc::channel();
        let mut senders = Vec::new();
        let mut handles = Vec::new();
        for worker in 0..workers {
            let (jobs, received) = mpsc::channel::<Work<'_>>();
            senders.push(jobs);
            let tx = tx.clone();
            handles.push(scope.spawn(move || {
                while let Ok(work) = received.recv() {
                    if control.check().is_err() {
                        break;
                    }
                    let events = tx.clone();
                    let mut events = EventHandler::with(move |(message, a, b)| {
                        let _ = events.send(Message::Event((
                            format!("{message} [worker {}]", worker + 1),
                            a,
                            b,
                        )));
                    })
                    .with_cancellation(control.cancelled.clone())
                    .with_worker_limit(1);
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match work {
                            Work::Bridge(job) => job
                                .execute(
                                    original,
                                    options,
                                    worker + 2,
                                    control,
                                    &mut NonexistenceOracle::new(original),
                                    &mut events,
                                )
                                .map(Completed::Bridge),
                            Work::Feedback(job) => job
                                .execute(original, worker + 2, control, &mut events)
                                .map(Completed::Feedback),
                        }))
                        .unwrap_or_else(|_| Err("Guided pool worker panicked".into()));
                    if tx.send(Message::Done { worker, result }).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(tx);
        let mut busy = vec![false; workers];
        let mut retries = VecDeque::new();
        let mut closed = false;
        let mut prefer_feedback = true;
        let mut prefer_new = true;
        let mut finished = 0;
        let mut peak = 0;
        let mut fresh_bridges = 0;
        let mut hot_bridges = 0;
        let answer = (|| {
            loop {
                control.check()?;
                // Import once into the shared archive, never copy the entire
                // search (or run default saturation) independently per worker.
                for _ in 0..32 {
                    match hints.try_recv() {
                        Ok(dag) => {
                            feedback.import(&dag, control)?;
                            if let Some(found) = bank.import(dag, &mut oracle, control, eh)? {
                                return Ok(found);
                            }
                        }
                        Err(TryRecvError::Disconnected) => {
                            closed = true;
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                    }
                }
                let previous_peak = peak;
                for worker in 0..workers {
                    if busy[worker] {
                        continue;
                    }
                    let active = busy.iter().filter(|&&b| b).count();
                    let reserved = (active + 1) * JOB_VARIABLES;
                    if reserved > control.guided_variable_budget {
                        break;
                    }
                    let cached = saved_variables(&retries) + feedback.saved_variables();
                    let minimum =
                        (reserved + cached).saturating_sub(control.guided_variable_budget);
                    // Pop a hot job BEFORE reserving its slot: its cached
                    // credits become part of the live reservation. No eviction.
                    let pressure = minimum > 0
                        || active + retries.len() + feedback.saved_count() >= workers.max(1) * 16;
                    let mut work = if pressure {
                        cached_work(&mut retries, &mut feedback, minimum, prefer_feedback, eh)
                    } else {
                        None
                    };
                    if !pressure {
                        if prefer_feedback {
                            work = feedback
                                .next_task(options, control, eh)?
                                .map(Work::Feedback);
                        }
                        if work.is_none() {
                            if prefer_new || retries.is_empty() {
                                if let Some(ids) = bank.next_batch(control)? {
                                    prefer_new = false;
                                    work = Some(Work::Bridge(Bridge::fresh(&bank, &ids)));
                                }
                            }
                            if work.is_none() {
                                prefer_new = true;
                                work = retries.pop_front().map(Work::Bridge);
                            }
                        }
                        if work.is_none() {
                            work = feedback
                                .next_task(options, control, eh)?
                                .map(Work::Feedback);
                        }
                    }
                    let Some(work) = work else {
                        continue;
                    };
                    debug_assert!(
                        reserved + saved_variables(&retries) + feedback.saved_variables()
                            <= control.guided_variable_budget
                    );
                    if let Work::Bridge(job) = &work {
                        if job.saved.is_some() {
                            hot_bridges += 1;
                        } else {
                            fresh_bridges += 1;
                        }
                    }
                    prefer_feedback = matches!(&work, Work::Bridge(_));
                    senders[worker]
                        .send(work)
                        .map_err(|_| "Guided pool worker disconnected")?;
                    busy[worker] = true;
                    peak = peak.max(active + 1);
                }
                // Report after filling the pool, otherwise event throttling
                // can hide the final count behind its immediately prior value.
                if peak > previous_peak {
                    eh.notify("Proof: guided peak busy workers", peak, workers);
                }
                if !busy.iter().any(|&b| b)
                    && closed
                    && retries.is_empty()
                    && feedback.saved_count() == 0
                    && !feedback.enabled(options)
                {
                    return Ok(CertificateSearchOutcome::Inconclusive { steps: max_steps });
                }
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(Message::Event((message, a, b))) => eh.notify(message, a, b),
                    Ok(Message::Done { worker, result }) => {
                        assert!(busy[worker]);
                        busy[worker] = false;
                        finished += 1;
                        match result? {
                            Completed::Bridge(done) => {
                                if let Some(found) = done.found {
                                    return Ok(found);
                                }
                                if let Some(retry) = done.retry {
                                    debug_assert!(retry.saved.is_some());
                                    retries.push_back(retry);
                                }
                            }
                            Completed::Feedback(done) => {
                                let (found, fragments) =
                                    feedback.accept(done, options, &mut oracle, control, eh)?;
                                if let Some(found) = found {
                                    return Ok(found);
                                }
                                for dag in fragments {
                                    if let Some(found) =
                                        bank.import(dag, &mut oracle, control, eh)?
                                    {
                                        return Ok(found);
                                    }
                                }
                            }
                        }
                        debug_assert!(
                            busy.iter().filter(|&&b| b).count() * JOB_VARIABLES
                                + saved_variables(&retries)
                                + feedback.saved_variables()
                                <= control.guided_variable_budget
                        );
                        if finished % 16 == 0 {
                            eh.notify("Proof: guided jobs completed", finished, 0);
                            eh.notify(
                                "Proof: guided fresh/hot bridge jobs",
                                fresh_bridges,
                                hot_bridges,
                            );
                            eh.notify(
                                "Proof: guided default fragments used",
                                bank.pinned_selected(),
                                bank.pinned_count(),
                            );
                            eh.notify("Proof: guided new fragments used", bank.new_selected, 0);
                            eh.notify("Proof: guided archive rotations", bank.rotations, 0);
                            eh.notify(
                                "Proof: guided pending changed blocks",
                                bank.pending_blocks(),
                                0,
                            );
                            eh.notify(
                                "Proof: guided cached jobs/variables",
                                retries.len() + feedback.saved_count(),
                                saved_variables(&retries) + feedback.saved_variables(),
                            );
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("Guided pool disconnected".into())
                    }
                }
            }
        })();
        // Local stop cannot convert an independent result into cancellation.
        control.stop();
        drop(senders);
        for handle in handles {
            if handle.join().is_err() {
                return Err("Guided pool worker panicked".into());
            }
        }
        answer
    })
}

#[cfg(test)]
mod tests;
