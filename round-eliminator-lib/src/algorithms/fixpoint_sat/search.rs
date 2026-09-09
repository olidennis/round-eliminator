//! Native Loop coordinator. Workers never call the GUI callback concurrently.
//! Scoped threads, cooperative checkpoints, and lifetime-guarded SAT
//! interrupters ensure a winner (or a panicking STOP callback) leaves no worker.

use super::*;
use rustsat::solvers::{Interrupt, InterruptSolver, SolveIncremental};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

const CANCELLED: &str = "Fixed-point search cancelled";

pub(super) fn check_cancelled(token: Option<&AtomicBool>) -> Result<(), String> {
    if token.is_some_and(|t| t.load(Ordering::Relaxed)) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn native_solver_is_interrupted_and_unregistered_before_drop() {
        for worker_id in 0..3 {
            let control = SearchControl::default();
            std::thread::scope(|scope| {
                let worker = scope.spawn(|| {
                    let mut solver = Minisat::default();
                    // Unsatisfiable pigeonhole formula: slow enough that the
                    // controller can reliably interrupt a genuinely active solve.
                    let holes = 19;
                    let lit = |p: usize, h: usize| Lit::new((p * holes + h) as u32, false);
                    for p in 0..=holes {
                        solver
                            .add_clause((0..holes).map(|h| lit(p, h)).collect())
                            .unwrap();
                    }
                    for h in 0..holes {
                        for p in 0..=holes {
                            for q in 0..p {
                                solver
                                    .add_clause([!lit(p, h), !lit(q, h)].into_iter().collect())
                                    .unwrap();
                            }
                        }
                    }
                    control.solve(worker_id, &mut solver, None)
                });
                let started = Instant::now();
                while control.solvers[worker_id].lock().unwrap().is_none() {
                    assert!(started.elapsed() < Duration::from_secs(5));
                    std::thread::sleep(Duration::from_millis(1));
                }
                control.stop();
                assert!(worker.join().unwrap().is_err());
            });
            assert!(control.solvers.iter().all(|s| s.lock().unwrap().is_none()));
            // No dangling interrupter may survive its solver.
            control.stop();
        }
    }

    #[test]
    fn simultaneous_pool_stop_preserves_independent_solver_registrations() {
        let root = SearchControl::with_guided_workers(4);
        let pool = root.guided_scope();
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for worker in 0..6 {
                let control = if worker < 2 { &root } else { &pool };
                handles.push(scope.spawn(move || {
                    let mut solver = Minisat::default();
                    let holes = 19;
                    let lit = |p: usize, h: usize| Lit::new((p * holes + h) as u32, false);
                    for p in 0..=holes {
                        solver
                            .add_clause((0..holes).map(|h| lit(p, h)).collect())
                            .unwrap();
                    }
                    for h in 0..holes {
                        for p in 0..=holes {
                            for q in 0..p {
                                solver
                                    .add_clause([!lit(p, h), !lit(q, h)].into_iter().collect())
                                    .unwrap();
                            }
                        }
                    }
                    control.solve(worker, &mut solver, None)
                }));
            }
            let started = Instant::now();
            while root.solvers.iter().any(|s| s.lock().unwrap().is_none()) {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut duplicate = Minisat::default();
            assert!(root
                .solve(0, &mut duplicate, None)
                .unwrap_err()
                .contains("interrupter slot"));
            pool.stop();
            while root.solvers[2..]
                .iter()
                .any(|s| s.lock().unwrap().is_some())
            {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(root.check().is_ok());
            assert!(root.solvers[..2]
                .iter()
                .all(|s| s.lock().unwrap().is_some()));
            root.stop();
            for handle in handles {
                assert!(handle.join().unwrap().is_err());
            }
        });
        assert!(root.solvers.iter().all(|s| s.lock().unwrap().is_none()));
        root.stop();
    }

    #[test]
    fn proof_winner_cancels_large_diagram_encoding() {
        let problem = Problem::from_string("A A\n\nA A").unwrap();
        let outcome = problem
            .fixpoint_search(
                &SatSearchOptions {
                    min_nodes: 40,
                    ..Default::default()
                },
                &Default::default(),
                &mut EventHandler::null(),
            )
            .unwrap();
        assert!(matches!(outcome, SatSearchOutcome::NoFixedPoint { .. }));
    }

    #[test]
    fn finite_workers_finishing_without_a_proof_remain_inconclusive() {
        let problem = Problem::from_string("A A\nB B\n\nA B").unwrap();
        let outcome = problem
            .fixpoint_search(
                &SatSearchOptions {
                    max_nodes: Some(2),
                    ..Default::default()
                },
                &CertificateSearchOptions {
                    max_steps: Some(1),
                    ..Default::default()
                },
                &mut EventHandler::null(),
            )
            .unwrap();
        assert!(matches!(
            outcome,
            SatSearchOutcome::Exhausted { max_nodes: 2, .. }
        ));
    }

    #[test]
    fn stop_callback_unwinds_after_cancelling_and_joining_workers() {
        let problem = Problem::from_string("A A\nB B\n\nA B").unwrap();
        let mut stopped = false;
        let mut events = EventHandler::with(|(message, _, _): (String, usize, usize)| {
            if message == "SAT: encoding lattice" {
                stopped = true;
                panic!("simulated server STOP");
            }
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            problem.fixpoint_search(
                &SatSearchOptions {
                    min_nodes: 40,
                    ..Default::default()
                },
                &Default::default(),
                &mut events,
            )
        }));
        drop(events);
        assert!(stopped);
        assert!(result.is_err());
    }

    #[test]
    fn native_loop_seeds_default_once_and_stop_joins_all_workers() {
        let problem = Problem::from_string(include_str!(
            "../../../examples/fixpoint_sat/hard_nonexistence.txt"
        ))
        .unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut starts = 0;
        let mut retained = 0;
        let mut events = EventHandler::with(|(message, current, _)| {
            if message == "Proof: default diagram seed starting" {
                starts += 1;
            }
            if message == "Proof: default diagram fragments retained" {
                retained = current;
                cancelled.store(true, Ordering::Relaxed);
            }
        })
        .with_cancellation(cancelled.clone());
        let result = problem.fixpoint_search(
            &SatSearchOptions {
                max_nodes: Some(1),
                ..Default::default()
            },
            &CertificateSearchOptions {
                max_steps: Some(1),
                conflict_limit: Some(1),
            },
            &mut events,
        );
        drop(events);
        assert_eq!(starts, 1);
        assert!(retained > 256);
        assert_eq!(result.unwrap_err(), CANCELLED);
    }
}

pub(super) fn check_event(eh: &EventHandler) -> Result<(), String> {
    if eh.is_cancelled() {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

pub(super) struct SearchControl {
    pub(super) cancelled: Arc<AtomicBool>,
    local_cancelled: Option<Arc<AtomicBool>>,
    solvers: Arc<Vec<Mutex<Option<rustsat_minisat::core::Interrupter>>>>,
    solver_range: std::ops::Range<usize>,
    pub(super) guided_variable_budget: usize,
    diagram_stats: Mutex<SatSearchStats>,
}

impl Default for SearchControl {
    fn default() -> Self {
        Self::with_guided_workers(1)
    }
}

impl SearchControl {
    pub(super) fn with_guided_workers(workers: usize) -> Self {
        assert!(workers > 0);
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            local_cancelled: None,
            solvers: Arc::new((0..workers + 2).map(|_| Mutex::new(None)).collect()),
            solver_range: 0..workers + 2,
            guided_variable_budget: 1_500_000,
            diagram_stats: Mutex::new(Default::default()),
        }
    }

    pub(super) fn guided_workers(&self) -> usize {
        self.solvers.len() - 2
    }

    // A pool winner must stop/join its sibling jobs before reporting a result,
    // without cancelling the independent searches ahead of that report.
    pub(super) fn guided_scope(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
            local_cancelled: Some(Arc::new(AtomicBool::new(false))),
            solvers: self.solvers.clone(),
            solver_range: 2..self.solvers.len(),
            guided_variable_budget: self.guided_variable_budget,
            diagram_stats: Mutex::new(Default::default()),
        }
    }
    pub(super) fn record_diagram_stats(&self, stats: &SatSearchStats) {
        *self.diagram_stats.lock().unwrap() = stats.clone();
    }

    pub(super) fn stop(&self) {
        self.local_cancelled
            .as_ref()
            .unwrap_or(&self.cancelled)
            .store(true, Ordering::Relaxed);
        for slot in &self.solvers[self.solver_range.clone()] {
            if let Some(interrupter) = slot.lock().unwrap().as_ref() {
                interrupter.interrupt();
            }
        }
    }

    pub(super) fn check(&self) -> Result<(), String> {
        check_cancelled(Some(&self.cancelled))?;
        check_cancelled(self.local_cancelled.as_deref())
    }

    pub(super) fn solve(
        &self,
        worker: usize,
        solver: &mut Minisat,
        assumptions: Option<&[Lit]>,
    ) -> Result<SolverResult, String> {
        self.check()?;
        if !self.solver_range.contains(&worker) {
            return Err("SAT worker is outside this cancellation scope".into());
        }
        // Minisat's interrupter contains a raw C handle. Clear it under the
        // same mutex used by stop(), BEFORE the borrowed solver can be freed,
        // including on error/unwind. It is registered only during this call.
        struct Registration<'a>(&'a Mutex<Option<rustsat_minisat::core::Interrupter>>);
        impl Drop for Registration<'_> {
            fn drop(&mut self) {
                self.0.lock().unwrap().take();
            }
        }
        let slot = &self.solvers[worker];
        {
            let mut registered = slot.lock().unwrap();
            if registered.is_some() {
                return Err("Concurrent SAT jobs share an interrupter slot".into());
            }
            *registered = Some(solver.interrupter());
        }
        let _registration = Registration(slot);
        self.check()?;
        let result = match assumptions {
            Some(assumptions) => solver.solve_assumps(assumptions),
            None => solver.solve(),
        }
        .map_err(|e| e.to_string());
        self.check()?;
        result
    }
}

struct CancelOnDrop<'a>(&'a SearchControl);
impl Drop for CancelOnDrop<'_> {
    fn drop(&mut self) {
        self.0.stop();
    }
}

enum Message {
    Event((String, usize, usize)),
    Diagram(Result<SatSearchOutcome, String>),
    Proof(Result<CertificateSearchOutcome, String>),
    Guided(Result<CertificateSearchOutcome, String>),
}

impl Problem {
    /// Run diagram synthesis, general proof synthesis, and witness-guided
    /// local proof synthesis side by side. None uses symbolic diagram
    /// completion. A conclusive answer cancels and joins all peers. If all
    /// workers finish inconclusively, return the diagram's bounded outcome.
    pub fn fixpoint_search(
        &self,
        diagrams: &SatSearchOptions,
        certificates: &CertificateSearchOptions,
        eh: &mut EventHandler,
    ) -> Result<SatSearchOutcome, String> {
        if !diagrams.check_nonexistence {
            return self.fixpoint_sat(diagrams, eh);
        }
        check_event(eh)?;
        if diagrams.min_nodes == 0 || diagrams.max_nodes.is_some_and(|n| n < diagrams.min_nodes) {
            return Err("SAT diagram search requires 1 <= min_nodes <= max_nodes".into());
        }
        proof::validate(self, certificates)?;
        let mut original = self.clone();
        if original.diagram_indirect.is_none() {
            original.compute_diagram(eh);
        }
        let settings = proof::guided::pool_settings()?;
        let mut control = SearchControl::with_guided_workers(settings.workers);
        control.guided_variable_budget = settings.variables;
        let (messages, received) = mpsc::channel();
        let (hints, seeds) = mpsc::sync_channel(64);
        let (derivations, fragments) = mpsc::channel();
        eh.notify(
            "Loop: starting diagram, certificate, and guided searches",
            0,
            0,
        );
        std::thread::scope(|scope| {
            // This guard is INSIDE the scope closure: STOP unwinds it before
            // thread::scope joins the workers, avoiding an unwind deadlock.
            let _cancel = CancelOnDrop(&control);
            let diagram_tx = messages.clone();
            let default_hints = hints.clone();
            let original = &original;
            let control = &control;
            let diagram_worker = scope.spawn(move || {
                let events = diagram_tx.clone();
                let mut eh = EventHandler::with(move |event| {
                    let _ = events.send(Message::Event(event));
                })
                .with_cancellation(control.cancelled.clone());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.fixpoint_sat_worker(
                        diagrams,
                        &mut eh,
                        Some(control),
                        Some(&hints),
                        Some(&derivations),
                    )
                }))
                .unwrap_or_else(|_| Err("Diagram search worker panicked".into()));
                let _ = diagram_tx.send(Message::Diagram(result));
            });
            let proof_tx = messages.clone();
            let proof_worker = scope.spawn(move || {
                let events = proof_tx.clone();
                let mut eh = EventHandler::with(move |event| {
                    let _ = events.send(Message::Event(event));
                })
                .with_cancellation(control.cancelled.clone());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    proof::run(original, certificates, &mut eh, control, Some(&seeds))
                }))
                .unwrap_or_else(|_| Err("Certificate search worker panicked".into()));
                let _ = proof_tx.send(Message::Proof(result));
            });
            let guided_tx = messages.clone();
            let guided_worker = scope.spawn(move || {
                let events = guided_tx.clone();
                let mut eh = EventHandler::with(move |event| {
                    let _ = events.send(Message::Event(event));
                })
                .with_cancellation(control.cancelled.clone());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    proof::guided::run_with_default_seed(
                        original,
                        certificates,
                        &mut eh,
                        control,
                        &fragments,
                        &default_hints,
                    )
                }))
                .unwrap_or_else(|_| Err("Guided certificate search worker panicked".into()));
                let _ = guided_tx.send(Message::Guided(result));
            });
            drop(messages);
            let mut diagram_result = None;
            let mut proof_finished = false;
            let mut guided_finished = false;
            let mut proof_won = false;
            let mut answer = loop {
                if eh.is_cancelled() {
                    break Err(CANCELLED.into());
                }
                match received.recv_timeout(Duration::from_millis(100)) {
                    Ok(Message::Event((message, current, total))) => {
                        eh.notify(message, current, total)
                    }
                    Ok(Message::Diagram(Err(error)))
                    | Ok(Message::Proof(Err(error)))
                    | Ok(Message::Guided(Err(error))) => break Err(error),
                    Ok(Message::Diagram(Ok(outcome))) => {
                        if matches!(
                            outcome,
                            SatSearchOutcome::Found(_) | SatSearchOutcome::NoFixedPoint { .. }
                        ) {
                            break Ok(outcome);
                        }
                        diagram_result = Some(outcome);
                    }
                    Ok(Message::Proof(Ok(CertificateSearchOutcome::Found {
                        certificate,
                        steps,
                        shared_lines,
                    })))
                    | Ok(Message::Guided(Ok(CertificateSearchOutcome::Found {
                        certificate,
                        steps,
                        shared_lines,
                    }))) => {
                        proof_won = true;
                        break Ok(SatSearchOutcome::NoFixedPoint {
                            certificate,
                            stats: SatSearchStats {
                                certificate_steps: steps,
                                certificate_shared_lines: shared_lines,
                                ..Default::default()
                            },
                        });
                    }
                    Ok(Message::Proof(Ok(_))) => proof_finished = true,
                    Ok(Message::Guided(Ok(_))) => guided_finished = true,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // The server checks STOP in its event callback. Keep
                        // calling it even while all native SAT calls block.
                        eh.notify("Loop: searches running", 0, 0);
                        if eh.is_cancelled() {
                            break Err(CANCELLED.into());
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break Err("Fixed-point search workers disconnected".into())
                    }
                }
                if proof_finished && guided_finished && diagram_result.is_some() {
                    break Ok(diagram_result.take().unwrap());
                }
            };
            control.stop();
            // Explicit joins consume panics after stopping the peer. STOP
            // callback panics instead use CancelOnDrop and scope's own joins.
            let diagram_join = diagram_worker.join();
            let proof_join = proof_worker.join();
            let guided_join = guided_worker.join();
            if diagram_join.is_err() || proof_join.is_err() || guided_join.is_err() {
                return Err("Fixed-point search worker panicked".into());
            }
            if proof_won {
                if let Ok(SatSearchOutcome::NoFixedPoint { stats, .. }) = &mut answer {
                    let mut diagram = control.diagram_stats.lock().unwrap().clone();
                    diagram.certificate_steps = stats.certificate_steps;
                    diagram.certificate_shared_lines = stats.certificate_shared_lines;
                    *stats = diagram;
                }
            }
            if let Ok(outcome) = &answer {
                eh.notify(
                    match outcome {
                        SatSearchOutcome::Found(_) => {
                            "Loop: found a good diagram; certificate search stopped"
                        }
                        SatSearchOutcome::NoFixedPoint { .. } => {
                            "Loop: proved nonexistence; all searches stopped"
                        }
                        _ => "Loop: search budgets exhausted without an all-size conclusion",
                    },
                    0,
                    0,
                );
            }
            answer
        })
    }
}
