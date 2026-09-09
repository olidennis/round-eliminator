use super::*;

fn problem(text: &str) -> Problem {
    let mut p = Problem::from_string(text).unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

fn term(algebra: &Algebra, labels: &[Label], id: Id) -> Term {
    let n = &algebra.terms[id];
    if let Some(op) = n.op {
        n.children
            .iter()
            .map(|&c| term(algebra, labels, c))
            .reduce(|a, b| {
                Term::Expr(
                    Box::new(a),
                    Box::new(b),
                    if op == JoinMeet::Join {
                        Operation::Union
                    } else {
                        Operation::Intersection
                    },
                )
            })
            .unwrap()
    } else {
        Term::Terminal(labels[id])
    }
}

#[test]
fn algebra_and_profiles_match_the_original_oracle() {
    for text in [
        "A A B\nB B C\n\nA AB\nB C\nC C",
        "A A B\nC C B\nD D E\n\nA AC\nB ED\nD D",
    ] {
        let p = problem(text);
        let control = SearchControl::default();
        let mut e = Engine::new(&p, &control).unwrap().unwrap();
        let mut raw = (0..e.labels.len())
            .map(|id| (id, Term::Terminal(e.labels[id])))
            .collect_vec();
        // Fixed schedule independent of any certificate; test nested,
        // non-distributive operations, not only shallow atomic formulas.
        let mut state = 73usize;
        for i in 0..70 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let a = state % raw.len();
            state = state.rotate_left(7);
            let b = state % raw.len();
            let op = if i % 2 == 0 {
                JoinMeet::Join
            } else {
                JoinMeet::Meet
            };
            let id = e.algebra.operation(raw[a].0, raw[b].0, op);
            let value = Term::Expr(
                Box::new(raw[a].1.clone()),
                Box::new(raw[b].1.clone()),
                if op == JoinMeet::Join {
                    Operation::Union
                } else {
                    Operation::Intersection
                },
            );
            raw.push((id, value));
        }
        let mut oracle = NonexistenceOracle::new(&p);
        for (a, x) in &raw {
            let normalized = term(&e.algebra, &e.labels, *a);
            assert!(oracle.terms_precede(x, &normalized));
            assert!(oracle.terms_precede(&normalized, x));
            for (b, y) in &raw {
                assert_eq!(e.algebra.precedes(*a, *b), oracle.terms_precede(x, y));
                assert_eq!(e.algebra.compatible(*a, *b), oracle.terms_compatible(x, y));
            }
        }
        let selected = (0..e.algebra.terms.len()).collect_vec();
        assert!(selected.len() <= 128);
        e.algebra.set_observers(&selected);
        // Create further terms AFTER choosing observers, so this checks the
        // profile transition algebra rather than just its atomic table.
        for a in 0..raw.len() {
            for b in 0..e.labels.len() {
                for op in [JoinMeet::Join, JoinMeet::Meet] {
                    let id = e.algebra.operation(raw[a].0, b, op);
                    let value = e.algebra.observe(id);
                    for (i, &t) in selected.iter().enumerate() {
                        assert_eq!(
                            e.algebra.observer.values[value] & (1 << i) != 0,
                            e.algebra.compatible(id, t)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn finite_equality_never_becomes_symbolic_equality() {
    let p = problem("A A B\nB B C\n\nA B\nA C\nB C");
    let control = SearchControl::default();
    let mut e = Engine::new(&p, &control).unwrap().unwrap();
    let ab = e.algebra.operation(0, 1, JoinMeet::Join);
    let ac = e.algebra.operation(0, 2, JoinMeet::Join);
    let lhs = e.algebra.operation(ab, ac, JoinMeet::Meet);
    let bc = e.algebra.operation(1, 2, JoinMeet::Meet);
    let rhs = e.algebra.operation(0, bc, JoinMeet::Join);
    assert_eq!(e.algebra.terms[lhs].default, e.algebra.terms[rhs].default);
    assert!(!e.algebra.precedes(lhs, rhs));
    assert!(e.algebra.precedes(rhs, lhs));
}

#[test]
fn bounds_and_stop_are_inconclusive_not_certificates() {
    let p = problem("A A\nB B\n\nA B");
    let control = SearchControl::default();
    assert!(matches!(
        run(
            &p,
            &CertificateSearchOptions {
                max_steps: Some(1),
                ..Default::default()
            },
            &mut EventHandler::null(),
            &control
        )
        .unwrap(),
        CertificateSearchOutcome::Inconclusive { .. }
    ));
    let mut e = Engine::new(&p, &control).unwrap().unwrap();
    e.deadline = Instant::now();
    assert_eq!(e.search(&mut EventHandler::null()).unwrap_err(), BUDGET);
    control.stop();
    assert!(e.check().is_err());
}

#[test]
fn preserves_repeated_occurrences_and_checks_replay() {
    let p = problem("A A A\n\nA A");
    let control = SearchControl::default();
    let e = Engine::new(&p, &control).unwrap().unwrap();
    let root = e.found.unwrap();
    let dag = e.derivation(root).unwrap();
    let replay = dag.replay(&input_terms(&p), 3, &control).unwrap();
    assert_eq!(replay.last().unwrap().len(), 3);
    assert!(NonexistenceOracle::new(&p)
        .check(replay.last().unwrap())
        .is_some());
}

#[test]
fn grouped_inputs_and_accelerator_limits_are_safe() {
    let control = SearchControl::default();
    let p = problem("AB AB\n\nAB AB");
    assert!(matches!(
        run(&p, &Default::default(), &mut EventHandler::null(), &control).unwrap(),
        CertificateSearchOutcome::Found { .. }
    ));
    for text in [
        "A A A A A A\n\nA A",
        "ABCDE ABCDE ABCDE ABCDE\n\nABCDE ABCDE",
    ] {
        assert!(Engine::new(&problem(text), &control).unwrap().is_none());
    }
}

#[test]
fn stop_during_a_closure_joins_cooperatively() {
    let p = problem("A A B\nA B B\n\nA B");
    let control = SearchControl::default();
    let mut e = Engine::new(&p, &control).unwrap().unwrap();
    let mut events = EventHandler::with(|(message, _, _): (String, usize, usize)| {
        if message.contains("reusable-fragment") {
            control.stop();
        }
    });
    assert!(e.search(&mut events).unwrap_err().contains("cancelled"));
}

#[test]
#[ignore = "native release regression: independently discovers the hard certificate"]
fn discovers_hard_certificate_without_a_fixture() {
    let p = problem(include_str!(
        "../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let control = SearchControl::default();
    let started = Instant::now();
    let result = run(&p, &Default::default(), &mut EventHandler::null(), &control).unwrap();
    let CertificateSearchOutcome::Found { certificate, .. } = result else {
        panic!("No certificate: {result:?}")
    };
    assert!(
        crate::algorithms::nofixpoint::algorithm::normalize_certificate(
            &p,
            &certificate,
            Some(Duration::from_secs(10)),
            &mut EventHandler::null()
        )
        .unwrap()
        .is_some()
    );
    eprintln!(
        "Blind closure certificate in {:?}\n{certificate}",
        started.elapsed()
    );
}
