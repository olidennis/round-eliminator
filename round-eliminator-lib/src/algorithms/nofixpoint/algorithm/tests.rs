use super::*;

fn problem(text: &str) -> Problem {
    let mut p = Problem::from_string(text).unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

#[test]
fn legacy_helper_uses_the_second_trees_root_mapping() {
    let p = problem("A A\nB B\nC C\nD D\n\nA B\nC D");
    let context = Context::init_from_problem(&p);
    let label = |name| {
        Expr::label(
            p.mapping_label_text
                .iter()
                .find(|(_, s)| s == name)
                .unwrap()
                .0,
        )
    };
    // Two equally-sized trees with DIFFERENT inorder root positions. The
    // wrong mapping selects an incompatible leaf instead of tree 2's root.
    let exprs = vec![
        Expr::right(Expr::right(label("A"), label("B")), label("A")),
        Expr::right(label("A"), Expr::right(label("C"), label("D"))),
    ];
    let result = p
        .nofixpoint_find_algorithm(&exprs, &context)
        .expect("All union games have compatible leaf choices");
    let mut ranks: Vec<usize> = result
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    ranks.sort_unstable();
    assert_eq!(ranks, (0..12).collect::<Vec<_>>());
}

#[test]
fn lazy_order_constraints_match_every_four_event_tournament() {
    use itertools::Itertools;
    for bits in 0..64 {
        let mut encoding = Encoding::new().unwrap();
        let mut edges = Vec::new();
        let mut next = 0;
        for a in 0..4 {
            for b in a + 1..4 {
                let lit = encoding.less(a, b);
                let forward = bits & (1 << next) != 0;
                next += 1;
                encoding
                    .clause(vec![if forward { lit } else { !lit }])
                    .unwrap();
                edges.push(if forward { (a, b) } else { (b, a) });
            }
        }
        assert_eq!(encoding.solver.solve().unwrap(), SolverResult::Sat);
        let model = encoding.solver.full_solution().unwrap();
        let expected = (0..4)
            .permutations(4)
            .any(|ranks| edges.iter().all(|&(a, b)| ranks[a] < ranks[b]));
        match encoding.order_or_cycles(&model, 4) {
            Ok(ranks) => {
                assert!(expected);
                assert!(edges.iter().all(|&(a, b)| ranks[a] < ranks[b]));
            }
            Err(cuts) => {
                assert!(!expected);
                assert!(!cuts.is_empty());
                encoding.exclude_cycles(cuts).unwrap();
                assert_eq!(encoding.solver.solve().unwrap(), SolverResult::Unsat);
            }
        }
    }
}

#[test]
fn extracted_trivial_algorithm_is_checked_and_bad_ranks_are_rejected() {
    let p = problem("A A\n\nA A");
    let certificate = "Original expressions:\n[A→A]\n[A←A]\n";
    let result = extract(
        &p,
        certificate,
        &Options {
            try_simple_orders: false,
            ..Default::default()
        },
        &mut EventHandler::null(),
    )
    .unwrap();
    let Outcome::Found(mut algorithm) = result else {
        panic!("Expected trivial schedule");
    };
    verify(&p, &algorithm).unwrap();
    algorithm.ranks.fill(0);
    assert!(verify(&p, &algorithm).is_err());
}

#[test]
fn concrete_games_match_encoded_games_for_every_small_schedule() {
    use itertools::Itertools;
    let p = problem("A A\nB B\n\nA B");
    let label = |name| {
        Expr::label(
            p.mapping_label_text
                .iter()
                .find(|(_, s)| s == name)
                .unwrap()
                .0,
        )
    };
    let prepared = Prepared::new(
        &p,
        &[
            Expr::right(label("A"), label("B")),
            Expr::left(label("A"), label("B")),
        ],
    )
    .unwrap();
    for ranks in (0..6).permutations(6) {
        let mut e = Encoding::new().unwrap();
        let root = e.game(&prepared, 0, 0, 1, 1).unwrap();
        for (a, b, lit) in e.ordered_pairs.clone() {
            e.clause(vec![if ranks[a] < ranks[b] { lit } else { !lit }])
                .unwrap();
        }
        assert_eq!(e.solver.solve().unwrap(), SolverResult::Sat);
        let model = e.solver.full_solution().unwrap();
        assert_eq!(
            model.lit_value(root) == TernaryVal::True,
            prepared.wins(&ranks, 0, 0, 1, 1)
        );
    }
}

#[test]
fn supplied_certificate_is_accepted_without_searching_for_it() {
    let p = problem(include_str!(
        "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let p = prepare(
        &p,
        include_str!("../../fixpoint_sat/proof/known_certificate.txt"),
    )
    .unwrap();
    assert_eq!(
        (p.degree, p.colors, p.arrows, p.priorities()),
        (4, 5, 61, 1220)
    );
}

#[test]
fn all_nineteen_reduced_loop_certificates_reconstruct_and_round_trip() {
    #[derive(serde::Deserialize)]
    struct Case {
        id: usize,
        problem: String,
        certificate: String,
    }
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/loop_certificates.json")).unwrap();
    assert_eq!(cases.len(), 19);
    for case in cases {
        let p = problem(&case.problem);
        let normalized = normalize_certificate(
            &p,
            &case.certificate,
            Some(Duration::from_secs(30)),
            &mut EventHandler::null(),
        )
        .unwrap_or_else(|e| panic!("case {}: {e}", case.id))
        .unwrap_or_else(|| panic!("case {} timed out", case.id));
        let prepared = prepare(&p, &normalized).unwrap();
        assert_eq!(prepared.degree, 3);
        assert_eq!(prepared.colors, 4);
        assert_eq!(
            normalize_certificate(&p, &normalized, None, &mut EventHandler::null())
                .unwrap()
                .unwrap(),
            normalized
        );
    }
}

#[test]
fn reconstruction_timeout_is_inconclusive_not_unsat() {
    let p = problem("A A\n\nA A");
    let outcome = extract(
        &p,
        "Original expressions:\nA\nA\n",
        &Options {
            time_limit: Some(Duration::ZERO),
            ..Default::default()
        },
        &mut EventHandler::null(),
    )
    .unwrap();
    assert!(matches!(outcome, Outcome::Inconclusive(_)));
}

#[test]
fn recovered_algorithm_saves_full_trees_and_round_trips_without_reconstruction() {
    let p = problem("A A B\nA B C\n\nABC ABC");
    let reduced = "Original expressions:\nA\n[C←A]\nB\n";
    let Outcome::Found(found) =
        extract(&p, reduced, &Options::default(), &mut EventHandler::null()).unwrap()
    else {
        panic!("All passive label pairs are compatible");
    };
    assert_eq!(found.source_certificate.as_deref(), Some(reduced));
    assert_ne!(found.certificate, reduced);
    assert_eq!(found.arrows_per_expression, 1);
    let mut json = serde_json::to_value(&found).unwrap();
    let saved: Algorithm = serde_json::from_value(json.clone()).unwrap();
    verify(&p, &saved).unwrap();
    // Provenance is optional for backwards-compatible algorithm JSON.
    json.as_object_mut().unwrap().remove("source_certificate");
    verify(&p, &serde_json::from_value(json).unwrap()).unwrap();
}
