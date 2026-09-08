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
