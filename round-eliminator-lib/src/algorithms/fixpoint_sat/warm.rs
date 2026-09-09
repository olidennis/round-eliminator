//! A bounded advisory check for unbounded Loop. A successful basic diagram
//! produces a warning only: it never replaces the minimum-size SAT result.

use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(5);

pub(super) fn try_default(
    original: &Problem,
    options: &SatSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
) -> Result<Option<usize>, String> {
    if options.min_nodes != 1
        || options.max_nodes.is_some()
        || options.max_candidates.is_some()
        || options.conflict_limit.is_some()
    {
        return Ok(None);
    }
    bounded(original, eh, control, LIMIT)
}

fn bounded(
    original: &Problem,
    eh: &mut EventHandler,
    control: &SearchControl,
    limit: Duration,
) -> Result<Option<usize>, String> {
    control.check()?;
    if limit.is_zero() || original.active.finite_degree() > 5 || original.labels().len() > 16 {
        return Ok(None);
    }
    let choices = original.active.lines.iter().fold(0usize, |sum, line| {
        sum.saturating_add(line.parts.iter().fold(1usize, |count, p| {
            count.saturating_mul(p.group.len().saturating_pow(p.gtype.value() as u32))
        }))
    });
    if choices > 512 {
        return Ok(None);
    }
    let started = Instant::now();
    let Some(candidate) = seeding::default_candidate(original, 64, control)? else {
        return Ok(None);
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let finished = (Mutex::new(false), Condvar::new());
    // Wake the deadline watcher even when STOP unwinds the construction.
    struct Finished<'a>(&'a (Mutex<bool>, Condvar));
    impl Drop for Finished<'_> {
        fn drop(&mut self) {
            *self.0 .0.lock().unwrap() = true;
            self.0 .1.notify_all();
        }
    }
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut done = finished.0.lock().unwrap();
            while !*done {
                if started.elapsed() >= limit || control.check().is_err() {
                    stopped.store(true, Ordering::Relaxed);
                    break;
                }
                done = finished
                    .1
                    .wait_timeout(done, Duration::from_millis(10))
                    .unwrap()
                    .0;
            }
        });
        let _finished = Finished(&finished);
        let mut events = EventHandler::with(|(message, a, b)| {
            eh.notify(format!("Loop: default candidate {message}"), a, b)
        })
        .with_cancellation(stopped.clone())
        .with_worker_limit(1);
        let nodes = candidate.order.len();
        events.notify("checking", nodes, 0);
        let names = (0..nodes)
            .map(|i| (i as Label, format!("(DEFAULT{i})")))
            .collect();
        let result = original.fixpoint_onestep(
            false,
            &candidate.label_mapping(),
            &names,
            &candidate.diagram(),
            None,
            None,
            &mut events,
        );
        result
            .map(|(mut p, _)| {
                p.compute_triviality(&mut events);
                p
            })
            .map_err(str::to_owned)
    });
    control.check()?;
    if stopped.load(Ordering::Relaxed) || started.elapsed() >= limit {
        eh.notify(
            "Loop: default candidate budget reached; other searches continue",
            0,
            0,
        );
        return Ok(None);
    }
    let problem = result?;
    if !problem
        .trivial_sets
        .as_ref()
        .ok_or("Missing default candidate triviality result")?
        .is_empty()
    {
        return Ok(None);
    }
    eh.notify(
        "Loop: nontrivial default candidate verified",
        candidate.order.len(),
        0,
    );
    Ok(Some(candidate.order.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(text: &str) -> Problem {
        let mut p = Problem::from_string(text).unwrap();
        p.compute_diagram(&mut EventHandler::null());
        p
    }

    #[test]
    fn verifies_the_sixteen_node_positive_example_without_returning_it() {
        let p = problem("A A A A\nB B B B\nC C C C\nD D D D\n\nA B\nA C\nA D\nB C\nB D\nC D");
        assert_eq!(
            try_default(
                &p,
                &Default::default(),
                &mut EventHandler::null(),
                &SearchControl::default()
            )
            .unwrap(),
            Some(16)
        );
    }

    #[test]
    fn honors_explicit_bounds_and_rejects_trivial_default_candidates() {
        let p = problem("A A\nB B\n\nA B");
        let control = SearchControl::default();
        for options in [
            SatSearchOptions {
                max_nodes: Some(10),
                ..Default::default()
            },
            SatSearchOptions {
                min_nodes: 4,
                ..Default::default()
            },
            SatSearchOptions {
                max_candidates: Some(0),
                ..Default::default()
            },
        ] {
            assert!(
                try_default(&p, &options, &mut EventHandler::null(), &control)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            bounded(&p, &mut EventHandler::null(), &control, Duration::ZERO)
                .unwrap()
                .is_none()
        );
        let trivial = problem("A A\n\nA A");
        assert!(try_default(
            &trivial,
            &Default::default(),
            &mut EventHandler::null(),
            &control
        )
        .unwrap()
        .is_none());
        control.stop();
        assert!(bounded(&p, &mut EventHandler::null(), &control, LIMIT).is_err());
    }
}
