use super::*;

#[test]
fn validates_models_and_does_not_turn_unknown_or_errors_into_unsat() {
    let a = Lit::new(0, false);
    let b = Lit::new(1, false);
    let clauses = vec![vec![a], vec![!a, b]];
    let parse = |code, text: &str| parse_output(code, text.as_bytes(), 2, &clauses, || Ok(()));
    assert_eq!(
        parse(Some(10), "s SATISFIABLE\nv 1 2 0\n").unwrap().0,
        SolverResult::Sat
    );
    assert_eq!(
        parse(Some(20), "s UNSATISFIABLE\n").unwrap().0,
        SolverResult::Unsat
    );
    assert_eq!(
        parse(Some(0), "s UNKNOWN\n").unwrap().0,
        SolverResult::Interrupted
    );
    for (code, text) in [
        (Some(10), "s SATISFIABLE\nv 1 -2 0\n"),
        (Some(10), "s SATISFIABLE\nv 1 0\n"),
        (Some(10), "s SATISFIABLE\nv 1 2 -2 0\n"),
        (Some(10), "s SATISFIABLE\nv 1 2 3 0\n"),
        (Some(10), "s SATISFIABLE\nv -2147483648 0\n"),
        (Some(10), "s SATISFIABLE\ns UNSATISFIABLE\n"),
        (Some(0), "s UNSATISFIABLE\n"),
        (Some(20), ""),
        (None, "s UNSATISFIABLE\n"),
        (Some(1), "could not read input\n"),
    ] {
        assert!(parse(code, text).is_err(), "{code:?}: {text}");
    }
}

#[test]
fn conflict_budgets_keep_native_semantics_and_threads_can_be_reassigned() {
    let mut settings = Settings::minisat();
    settings.binary = Some(PathBuf::from("gimsatul"));
    settings.initial_threads = 5;
    settings.solo_threads = 10;
    settings.total_threads = 10;
    assert_eq!(settings.certificate_threads(), 5);
    assert!(!settings.uses_external(11, &Default::default()));
    assert!(settings.uses_external(12, &Default::default()));
    assert!(!settings.uses_external(
        12,
        &SatSearchOptions {
            conflict_limit: Some(10),
            ..Default::default()
        }
    ));
    let runtime = Runtime::new(settings, false);
    assert_eq!(runtime.threads.load(Ordering::Relaxed), 5);
    runtime.release_certificate_budget();
    assert_eq!(runtime.threads.load(Ordering::Relaxed), 10);
}

#[cfg(unix)]
#[test]
fn child_guard_kills_and_reaps_on_unwind() {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    let id = child.id();
    let result = std::panic::catch_unwind(move || {
        let _running = Running(child);
        panic!("simulated STOP callback");
    });
    assert!(result.is_err());
    assert!(!Command::new("kill")
        .args(["-0", &id.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
}

fn installed() -> Option<Settings> {
    let mut settings = Settings::environment().unwrap();
    if settings.binary.is_none() {
        return None;
    }
    settings.initial_threads = 2;
    settings.solo_threads = 2;
    settings.min_nodes = 1;
    settings.seconds = Some(5);
    Some(settings)
}

#[test]
fn gimsatul_and_minisat_agree_on_small_diagrams_and_blockers_when_installed() {
    let Some(settings) = installed() else { return };
    let runtime = Runtime::new(settings, false);
    let mut p = Problem::from_string("A A\nB B\n\nA B").unwrap();
    p.compute_diagram(&mut EventHandler::null());
    for nodes in 1..=4 {
        let mut encoding = Encoding::new_recorded(&p, nodes, None, true).unwrap();
        for _ in 0..6 {
            let native = encoding.solver.solve().unwrap();
            let (external, model) =
                solve(&encoding, &runtime, &mut EventHandler::null(), None).unwrap();
            assert_eq!(native, external);
            if external != SolverResult::Sat {
                break;
            }
            let candidate = encoding.candidate(&model.unwrap()).unwrap();
            encoding.block_exact(&candidate).unwrap();
        }
    }
}

#[test]
fn gimsatul_stop_while_solving_is_cancellation_when_installed() {
    let Some(settings) = installed() else { return };
    let runtime = Runtime::new(settings, false);
    let mut p = Problem::from_string("A A\n\nA A").unwrap();
    p.compute_diagram(&mut EventHandler::null());
    let mut encoding = Encoding::new_recorded(&p, 1, None, true).unwrap();
    let holes = 24;
    let lits: Vec<Vec<_>> = (0..=holes)
        .map(|_| (0..holes).map(|_| encoding.literal()).collect())
        .collect();
    for row in &lits {
        encoding.clause(row.iter().copied()).unwrap();
    }
    for h in 0..holes {
        for p in 0..=holes {
            for q in 0..p {
                encoding.clause([!lits[p][h], !lits[q][h]]).unwrap();
            }
        }
    }
    let control = SearchControl::default();
    let mut notifications = 0;
    let mut events = EventHandler::with(|_| {
        notifications += 1;
        if notifications >= 2 {
            control.stop();
        }
    });
    let started = Instant::now();
    let result = solve(&encoding, &runtime, &mut events, Some(&control));
    drop(events);
    assert!(notifications >= 2);
    assert_eq!(result.unwrap_err(), "Fixed-point search cancelled");
    assert!(started.elapsed() < Duration::from_secs(5));
}
