//! Independent P / RE²(P) lanes. Preparing the second target never occupies a
//! direct-search worker, and only the owner thread publishes GUI events.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

enum Message {
    Progress(bool, (String, usize, usize)),
    Update(bool, Report),
    Done(bool, Result<Report, String>),
}

fn merge(report: &mut Report, update: &Report, storage_limited: &mut bool) {
    for c in &update.certificates {
        if report.certificates.iter().any(|old| old.added == c.added) {
            continue;
        }
        if fits_report(report, c) {
            report.certificates.push(c.clone());
        } else {
            *storage_limited = true;
        }
    }
}

pub(super) fn search(
    p: &Problem,
    options: &Options,
    eh: &mut EventHandler,
    mut publish: impl FnMut(&Report),
) -> Result<Report, String> {
    search_with_prepare_control(
        p,
        options,
        eh,
        |report| {
            publish(report);
            true
        },
        None,
        re_target::prepare,
    )
}

pub(super) fn search_first(
    p: &Problem,
    options: &Options,
    eh: &mut EventHandler,
) -> Result<Option<Certificate>, String> {
    let mut first = None;
    search_with_prepare_control(
        p,
        options,
        eh,
        |report| {
            if let Some(certificate) = report.certificates.first() {
                first = Some(certificate.clone());
                false
            } else {
                true
            }
        },
        Some(rand::random()),
        re_target::prepare,
    )?;
    Ok(first)
}

#[cfg(test)]
fn search_with_prepare(
    p: &Problem,
    options: &Options,
    eh: &mut EventHandler,
    mut publish: impl FnMut(&Report),
    prepare: impl Fn(&Problem, &Budget<'_>, &mut EventHandler<'_>) -> Result<Re2Target, String> + Sync,
) -> Result<Report, String> {
    search_with_prepare_control(
        p,
        options,
        eh,
        |report| {
            publish(report);
            true
        },
        None,
        prepare,
    )
}

fn search_with_prepare_control(
    p: &Problem,
    options: &Options,
    eh: &mut EventHandler,
    mut publish: impl FnMut(&Report) -> bool,
    candidate_seed: Option<u64>,
    prepare: impl Fn(&Problem, &Budget<'_>, &mut EventHandler<'_>) -> Result<Re2Target, String> + Sync,
) -> Result<Report, String> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(options.seconds);
    if !options.re2 {
        return search_branch(
            p,
            options,
            None,
            started,
            deadline,
            candidate_seed,
            eh,
            publish,
        );
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| {
        // These locals are dropped before scope joins, including on callback
        // panic: cancellation is set and blocked bounded-channel sends wake up.
        let (tx, rx) = mpsc::sync_channel(64);
        struct Stop(Arc<AtomicBool>);
        impl Drop for Stop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _stop = Stop(cancelled.clone());
        for re2 in [false, true] {
            let prepare = &prepare;
            let tx = tx.clone();
            let cancelled = cancelled.clone();
            scope.spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let progress = tx.clone();
                    let mut events = EventHandler::with(move |event| {
                        let _ = progress.send(Message::Progress(re2, event));
                    })
                    .with_cancellation(cancelled)
                    .with_worker_limit(1);
                    let mut lane_options = options.clone();
                    if re2 {
                        lane_options.threads = 1;
                    }
                    let target = if re2 {
                        events.notify("Reversible edges: RE² target worker started", 1, 0);
                        let budget = Budget {
                            options: &lane_options,
                            deadline: deadline.min(Instant::now() + Duration::from_secs(10)),
                        };
                        Some(prepare(p, &budget, &mut events)?)
                    } else {
                        None
                    };
                    search_branch(
                        p,
                        &lane_options,
                        target.as_ref(),
                        started,
                        deadline,
                        candidate_seed,
                        &mut events,
                        |r| {
                            let _ = tx.send(Message::Update(re2, r.clone()));
                            true
                        },
                    )
                }))
                .unwrap_or_else(|payload| {
                    let message = panic_message(payload);
                    Err(if re2 {
                        format!("RE² worker panicked: {message}; direct search was left running")
                    } else {
                        format!("Direct reversible-edge worker panicked: {message}")
                    })
                });
                let _ = tx.send(Message::Done(re2, result));
            });
        }
        drop(tx);
        let mut report = Report {
            original: p.clone(),
            certificates: vec![],
            stats: Stats::default(),
            complete: false,
            message: "Searching P and RE²(P) independently; only verified additions are listed."
                .into(),
        };
        let mut latest: [Option<Report>; 2] = [None, None];
        let mut done = [false, false];
        let mut re2_error = None;
        let mut storage_limited = false;
        let mut stopped = false;
        while !done.iter().all(|&b| b) {
            if eh.is_cancelled() {
                return Err(CANCELLED.into());
            }
            let mut changed = false;
            match rx.recv_timeout(Duration::from_millis(30)) {
                Ok(Message::Progress(re2, (s, a, b))) => {
                    let s = if re2 {
                        format!("Reversible edges: RE² lane: {s}")
                    } else {
                        s
                    };
                    eh.notify(s, a, b);
                }
                Ok(Message::Update(re2, update)) => {
                    merge(&mut report, &update, &mut storage_limited);
                    latest[usize::from(re2)] = Some(update);
                    changed = true;
                }
                Ok(Message::Done(re2, result)) => {
                    let lane = usize::from(re2);
                    done[lane] = true;
                    match result {
                        Ok(update) => {
                            merge(&mut report, &update, &mut storage_limited);
                            latest[lane] = Some(update);
                        }
                        Err(_) if stopped => {}
                        Err(e) if re2 && e != CANCELLED => {
                            eh.notify(format!("Reversible edges: RE² lane stopped: {e}; direct results retained"), 0, 0);
                            re2_error = Some(e);
                        }
                        Err(e) => return Err(e),
                    }
                    changed = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eh.notify(
                        "Reversible edges: independent target searches running",
                        report.stats.mapping_attempts,
                        0,
                    );
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Reversible-edge target workers disconnected".into())
                }
            }
            if changed && !stopped {
                report.stats = Stats::default();
                for lane in latest.iter().flatten() {
                    report.stats.candidates += lane.stats.candidates;
                    report.stats.mapping_attempts += lane.stats.mapping_attempts;
                    report.stats.bounded_attempts += lane.stats.bounded_attempts;
                }
                report.stats.re2_mapping_attempts =
                    latest[1].as_ref().map_or(0, |r| r.stats.mapping_attempts);
                report.stats.elapsed_ms = started.elapsed().as_millis() as u64;
                report.complete = done.iter().all(|&b| b)
                    && !storage_limited
                    && re2_error.is_none()
                    && latest.iter().flatten().all(|r| r.complete);
                report.message = format!(
                    "{} verified additions/sets. P and RE²(P) use independent workers. {} Unlisted additions are not certified, NOT proved irreversible. Joint sets are searched heuristically, not exhaustively.",
                    report.certificates.len(),
                    if let Some(error) = &re2_error { format!("RE² lane stopped: {error}; direct results retained.") }
                    else if !done.iter().all(|&b| b) { "Searching; completed-lane results are retained.".into() }
                    else if report.complete { "Both scheduled searches finished.".into() }
                    else { "Some searches reached their limits.".into() },
                );
                if !publish(&report) {
                    stopped = true;
                    cancelled.store(true, Ordering::Release);
                }
            }
        }
        Ok(report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Problem {
        Problem::from_string("A A\nB B\n\nA A\nA B").unwrap()
    }

    #[test]
    fn target_preparation_panic_keeps_its_message_and_direct_results() {
        let report = search_with_prepare(
            &small(),
            &Options {
                seconds: 2,
                threads: 1,
                ..Default::default()
            },
            &mut EventHandler::null(),
            |_| {},
            |_, _, _| panic!("specific target preparation error"),
        )
        .unwrap();
        assert!(!report.complete);
        assert!(!report.certificates.is_empty());
        assert!(report.message.contains("specific target preparation error"));
        assert!(report.message.contains("direct results retained"));
    }

    #[test]
    fn both_targets_run_and_duplicate_additions_are_published_once() {
        let p = small();
        let report = search(
            &p,
            &Options {
                seconds: 2,
                threads: 1,
                ..Default::default()
            },
            &mut EventHandler::null(),
            |r| {
                let additions: BTreeSet<_> = r.certificates.iter().map(|c| &c.added).collect();
                assert_eq!(additions.len(), r.certificates.len());
            },
        )
        .unwrap();
        assert!(report.stats.re2_mapping_attempts > 0);
        assert!(report.stats.mapping_attempts > report.stats.re2_mapping_attempts);
        assert_eq!(report.certificates.len(), 1);
        apply(&p, &report.certificates[0], &mut EventHandler::null()).unwrap();
    }

    #[test]
    fn direct_results_arrive_before_blocked_re2_preparation_finishes() {
        let published = AtomicBool::new(false);
        let report = search_with_prepare(
            &small(),
            &Options {
                seconds: 2,
                threads: 1,
                ..Default::default()
            },
            &mut EventHandler::null(),
            |r| {
                if !r.certificates.is_empty() {
                    published.store(true, Ordering::Release);
                }
            },
            |_, budget, events| {
                while !published.load(Ordering::Acquire) {
                    budget.check(events)?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(LIMIT.into())
            },
        )
        .unwrap();
        assert!(published.load(Ordering::Acquire));
        assert!(!report.certificates.is_empty());
        assert!(report.certificates.iter().all(|c| c.target.is_none()));
        assert!(report.message.contains("direct results retained"));
        assert!(!report.complete);
    }

    #[test]
    fn callback_panic_cancels_and_joins_both_target_lanes() {
        let started = AtomicBool::new(false);
        let stopped = AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            search_with_prepare(
                &small(),
                &Options {
                    seconds: 2,
                    threads: 1,
                    ..Default::default()
                },
                &mut EventHandler::null(),
                |r| {
                    if !r.certificates.is_empty() {
                        while !started.load(Ordering::Acquire) {
                            std::thread::yield_now();
                        }
                        panic!("simulated GUI STOP during target preparation");
                    }
                },
                |_, budget, events| {
                    struct Mark<'a>(&'a AtomicBool);
                    impl Drop for Mark<'_> {
                        fn drop(&mut self) {
                            self.0.store(true, Ordering::Release);
                        }
                    }
                    let _mark = Mark(&stopped);
                    started.store(true, Ordering::Release);
                    loop {
                        budget.check(events)?;
                        std::thread::sleep(Duration::from_millis(1));
                    }
                },
            )
        }));
        assert!(result.is_err());
        assert!(stopped.load(Ordering::Acquire));
    }

    #[test]
    fn target_preparation_uses_the_shared_deadline_and_can_be_disabled() {
        let p = small();
        let options = Options {
            seconds: 1,
            threads: 1,
            ..Default::default()
        };
        let started = Instant::now();
        let report = search_with_prepare(
            &p,
            &options,
            &mut EventHandler::null(),
            |_| {},
            |_, budget, events| loop {
                budget.check(events)?;
                std::thread::sleep(Duration::from_millis(1));
            },
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!report.certificates.is_empty());
        let report = search_with_prepare(
            &p,
            &Options {
                re2: false,
                ..options
            },
            &mut EventHandler::null(),
            |_| {},
            |_, _, _| panic!("disabled target must not be built"),
        )
        .unwrap();
        assert!(report.certificates.iter().all(|c| c.target.is_none()));
        assert_eq!(report.stats.re2_mapping_attempts, 0);
    }
}
