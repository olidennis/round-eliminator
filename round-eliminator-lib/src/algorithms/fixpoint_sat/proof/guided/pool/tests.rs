use super::*;

fn problem(text: &str) -> Problem {
    let mut p = Problem::from_string(text).unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

#[test]
fn configuration_is_bounded_and_independent_of_saturation_threads() {
    assert_eq!(parse_settings(None, None, 10).unwrap().workers, 6);
    assert_eq!(parse_settings(None, None, 2).unwrap().workers, 1);
    assert_eq!(parse_settings(Some("4"), None, 10).unwrap().workers, 4);
    for bad in ["0", "33", "no", "-1"] {
        assert!(parse_settings(Some(bad), None, 10).is_err());
    }
    assert!(parse_settings(None, Some("151999"), 10).is_err());
    assert_eq!(
        parse_settings(None, Some("152000"), 10).unwrap().variables,
        JOB_VARIABLES
    );
}

#[test]
fn parallel_pool_finds_verified_certificates_and_does_not_stop_independent_searches() {
    let p = problem(include_str!(
        "../../../../../../examples/fixpoint_sat/maximal_matching.txt"
    ));
    for workers in [1, 4] {
        let outer = SearchControl::with_guided_workers(workers);
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let result = run(
            &p,
            &Default::default(),
            &mut EventHandler::null(),
            &outer,
            &rx,
            false,
            None,
        )
        .unwrap();
        assert!(matches!(result, CertificateSearchOutcome::Found { .. }));
        assert!(
            outer.check().is_ok(),
            "The outer coordinator owns global cancellation"
        );
    }
}

#[test]
fn bounded_pool_jobs_finish_inconclusively() {
    let p = problem("A A\nB B\n\nA B");
    let control = SearchControl::with_guided_workers(4);
    let (tx, rx) = mpsc::channel();
    drop(tx);
    let result = run(
        &p,
        &CertificateSearchOptions {
            max_steps: Some(1),
            conflict_limit: Some(1),
        },
        &mut EventHandler::null(),
        &control,
        &rx,
        false,
        None,
    )
    .unwrap();
    assert!(matches!(
        result,
        CertificateSearchOutcome::Inconclusive { .. }
    ));
    assert!(control.check().is_ok());
}

#[test]
fn memory_credits_bound_live_jobs_across_the_whole_pool() {
    let p = problem("A A\nB B\n\nA B");
    let mut control = SearchControl::with_guided_workers(4);
    control.guided_variable_budget = JOB_VARIABLES;
    let (tx, rx) = mpsc::channel();
    drop(tx);
    let mut peak = 0;
    let mut events = EventHandler::with(|(message, n, _)| {
        if message == "Proof: guided peak busy workers" {
            peak = peak.max(n);
        }
    });
    run(
        &p,
        &CertificateSearchOptions {
            max_steps: Some(1),
            conflict_limit: Some(1),
        },
        &mut events,
        &control,
        &rx,
        false,
        None,
    )
    .unwrap();
    drop(events);
    assert_eq!(peak, 1);
}

#[test]
fn stop_callback_with_busy_pool_joins_every_worker() {
    let p = problem(include_str!(
        "../../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let outer = SearchControl::with_guided_workers(4);
    let (tx, rx) = mpsc::channel();
    drop(tx);
    let mut peak = 0;
    let mut events = EventHandler::with(|(message, n, _)| {
        if message == "Proof: guided peak busy workers" {
            peak = n;
            if n == 4 {
                panic!("simulated STOP while pool is busy");
            }
        }
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(
            &p,
            &Default::default(),
            &mut events,
            &outer,
            &rx,
            true,
            None,
        )
    }));
    drop(events);
    assert!(result.is_err());
    assert_eq!(peak, 4);
    assert!(outer.check().is_ok());
}

#[test]
fn hot_bridge_keeps_solved_bounds_and_owns_its_original_terms() {
    let p = problem(include_str!(
        "../../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let control = SearchControl::default();
    let mut bank = Bank::new(input_terms(&p));
    let seeds = bank.tuples.clone();
    let ids: Vec<_> = (0..seeds.len()).collect();
    let result = Bridge::fresh(&bank, &ids)
        .execute(
            &p,
            &Default::default(),
            2,
            &control,
            &mut NonexistenceOracle::new(&p),
            &mut EventHandler::null(),
        )
        .unwrap();
    let retry = result.retry.unwrap();
    let saved = retry.saved.as_ref().unwrap();
    assert_eq!(saved.goals.len(), 1);
    assert_eq!(saved.exhausted, [true]);
    let variables = retry.variables();
    assert!(retry.seeds.is_empty());
    // A later archive change cannot alter a queued solver's proof leaves.
    bank.tuples[0].clear();
    let mut retries = VecDeque::from([retry]);
    let mut feedback = feedback::Engine::new(&p, &control).unwrap();
    let Some(Work::Bridge(mut resumed)) = cached_work(
        &mut retries,
        &mut feedback,
        variables,
        false,
        &mut EventHandler::null(),
    ) else {
        panic!("missing cached bridge");
    };
    assert!(retries.is_empty());
    assert_eq!(resumed.variables(), variables);
    let job = resumed.saved.as_mut().unwrap();
    let mut calls = 0;
    let mut events = EventHandler::with(|(message, _, _)| {
        if message.starts_with("Proof: guided SAT") {
            calls += 1;
        }
    });
    assert!(job
        .search(
            0,
            &mut NonexistenceOracle::new(&p),
            &Default::default(),
            &mut events
        )
        .unwrap()
        .is_none());
    drop(events);
    assert_eq!(calls, 0, "A proved UNSAT bound must not be solved again");
    assert_eq!(job.leaves, seeds.len());
}

#[test]
fn hot_work_cap_retires_locally_instead_of_requeuing_forever() {
    let p = problem("A A\nB B\n\nA B");
    let control = SearchControl::default();
    let bank = Bank::new(input_terms(&p));
    let mut bridge = Bridge::fresh(&bank, &[0, 1]);
    // It would ordinarily deepen after this slice, even if this bound is UNSAT.
    bridge.slices = MAX_BRIDGE_SLICES - 1;
    let result = bridge
        .execute(
            &p,
            &Default::default(),
            2,
            &control,
            &mut NonexistenceOracle::new(&p),
            &mut EventHandler::null(),
        )
        .unwrap();
    assert!(result.retry.is_none());
    assert!(result.found.is_none());
    assert!(control.check().is_ok());
}

#[test]
fn one_slot_budget_resumes_hot_jobs_without_deadlock_or_credit_overflow() {
    let p = problem(include_str!(
        "../../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let mut control = SearchControl::with_guided_workers(4);
    control.guided_variable_budget = JOB_VARIABLES;
    let (tx, rx) = mpsc::channel();
    drop(tx);
    let mut hot = 0;
    let mut peak = 0;
    let mut events = EventHandler::with(|(message, a, b)| {
        if message == "Proof: guided fresh/hot bridge jobs" {
            hot = hot.max(b);
        }
        if message == "Proof: guided peak busy workers" {
            peak = peak.max(a);
        }
    });
    let outcome = run(
        &p,
        &CertificateSearchOptions {
            max_steps: Some(2),
            conflict_limit: Some(1),
        },
        &mut events,
        &control,
        &rx,
        false,
        None,
    )
    .unwrap();
    drop(events);
    assert!(matches!(
        outcome,
        CertificateSearchOutcome::Inconclusive { .. }
    ));
    assert_eq!(peak, 1);
    assert!(
        hot > 0,
        "The test must exercise a cached-to-live credit transfer"
    );
    assert!(control.check().is_ok());
}

#[test]
fn each_bridge_slice_makes_at_most_one_sat_call() {
    let p = problem(include_str!(
        "../../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let control = SearchControl::default();
    let bank = Bank::new(input_terms(&p));
    let ids: Vec<_> = (0..bank.inputs).collect();
    let mut bridge = Bridge::fresh(&bank, &ids);
    let options = CertificateSearchOptions {
        max_steps: Some(3),
        conflict_limit: Some(1),
    };
    for step in 1..=3 {
        let mut calls = 0;
        let mut events = EventHandler::with(|(message, _, _)| {
            if message.starts_with("Proof: guided SAT") {
                calls += 1;
            }
        });
        let result = bridge
            .execute(
                &p,
                &options,
                2,
                &control,
                &mut NonexistenceOracle::new(&p),
                &mut events,
            )
            .unwrap();
        drop(events);
        assert_eq!(calls, 1);
        assert!(result.found.is_none());
        if step == 3 {
            assert!(result.retry.is_none());
            break;
        }
        bridge = result.retry.unwrap();
        assert_eq!(bridge.saved.as_ref().unwrap().goals.len(), step);
    }
}
