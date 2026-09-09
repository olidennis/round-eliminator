use super::*;

fn problem(text: &str) -> Problem {
    let mut original = Problem::from_string(text).unwrap();
    original.compute_diagram(&mut EventHandler::null());
    original
}

#[test]
fn replay_preserves_whole_inputs_occurrences_and_pivots() {
    let original = problem("A A B\nA B B\n\nA B");
    let inputs = input_terms(&original);
    let labels = |i: usize| {
        inputs[i]
            .iter()
            .map(|t| match t {
                Term::Terminal(l) => *l,
                _ => unreachable!(),
            })
            .collect()
    };
    let derivation = Derivation {
        steps: vec![
            Step::Input(labels(0)),
            Step::Input(labels(1)),
            Step::Combine {
                parents: [0, 1],
                permutations: [vec![2, 0, 1], vec![0, 2, 1]],
                pivot: 1,
            },
        ],
    };
    let values = derivation
        .replay(&inputs, 3, &SearchControl::default())
        .unwrap();
    let expected: Vec<_> = (0..3)
        .map(|i| {
            combine(
                inputs[0][[2, 0, 1][i]].clone(),
                inputs[1][[0, 2, 1][i]].clone(),
                if i == 1 {
                    Operation::Union
                } else {
                    Operation::Intersection
                },
            )
        })
        .collect();
    assert!(values[2] == expected);
    let mut bad = derivation.clone();
    bad.steps[0] = Step::Input(vec![999; 3]);
    assert!(bad.replay(&inputs, 3, &SearchControl::default()).is_err());
    for (parents, permutations, pivot) in [
        ([0, 2], [vec![0, 1, 2], vec![0, 1, 2]], 0),
        ([0, 1], [vec![0, 0, 2], vec![0, 1, 2]], 0),
        ([0, 1], [vec![0, 1, 2], vec![0, 1, 2]], 3),
    ] {
        let mut bad = derivation.clone();
        bad.steps[2] = Step::Combine {
            parents,
            permutations,
            pivot,
        };
        assert!(bad.replay(&inputs, 3, &SearchControl::default()).is_err());
    }
}

#[test]
fn local_unsat_and_cancellation_are_not_global_conclusions() {
    let original = problem("A A\nB B\n\nA B");
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    let control = SearchControl::default();
    let options = CertificateSearchOptions {
        max_steps: Some(2),
        ..Default::default()
    };
    assert!(matches!(
        run(
            &original,
            &options,
            &mut EventHandler::null(),
            &control,
            &rx
        )
        .unwrap(),
        CertificateSearchOutcome::Inconclusive { .. }
    ));
    control.stop();
    assert!(run(
        &original,
        &Default::default(),
        &mut EventHandler::null(),
        &control,
        &rx
    )
    .is_err());
}

#[test]
fn guided_search_finds_a_certificate_without_external_hints() {
    let original = problem(include_str!(
        "../../../../../examples/fixpoint_sat/maximal_matching.txt"
    ));
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    assert!(matches!(
        run(
            &original,
            &Default::default(),
            &mut EventHandler::null(),
            &SearchControl::default(),
            &rx
        )
        .unwrap(),
        CertificateSearchOutcome::Found { .. }
    ));
}

#[test]
fn all_default_fragments_get_archive_space_without_using_the_game_allowance() {
    let original = problem(include_str!(
        "../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let control = SearchControl::default();
    let dag = super::super::super::seeding::collect(&original, &mut EventHandler::null(), &control)
        .unwrap()
        .unwrap();
    let mut bank = Bank::new(input_terms(&original));
    let values = dag.replay(&bank.tuples, 4, &control).unwrap();
    let mut expected = bank.known.clone();
    for mut row in values {
        row.sort();
        expected.insert(row);
    }
    let mut oracle = NonexistenceOracle::new(&original);
    assert!(bank
        .import_default(dag, &mut oracle, &control, &mut EventHandler::null())
        .unwrap()
        .is_none());
    assert!(bank.known == expected);
    assert!(bank.tuples.len() - bank.inputs > MAX_FRAGMENTS);
    assert_eq!(
        bank.fragment_limit - (bank.tuples.len() - bank.inputs),
        MAX_FRAGMENTS
    );
    // All extracted configurations stay eligible, including the last one and
    // its cross-batch combinations. No single giant SAT instance is built.
    let mut seen = HashSet::new();
    for _ in 0..bank.tuples.len().div_ceil(BATCH_SIZE / 2) {
        if let Some(batch) = bank.next_batch(&control).unwrap() {
            seen.extend(batch);
        }
    }
    assert_eq!(seen.len(), bank.tuples.len());
}

#[test]
fn one_step_neighborhood_can_bridge_known_hard_subderivations() {
    // This is only a recombination unit test, NOT automatic rediscovery of the
    // hard certificate: its two children are deliberately supplied here.
    let original = problem(include_str!(
        "../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let terms = super::super::tests::known_terms(&original);
    let mut children = [Vec::new(), Vec::new()];
    for term in terms {
        let Term::Expr(a, b, _) = term else {
            unreachable!()
        };
        children[0].push(*a);
        children[1].push(*b);
    }
    let mut oracle = NonexistenceOracle::new(&original);
    assert!(children.iter().all(|t| oracle.check(t).is_none()));
    let mut bank = Bank::new(input_terms(&original));
    let first = bank.tuples.len();
    bank.tuples.extend(children);
    let control = SearchControl::default();
    let mut local = Neighborhood::new(&original, &bank, &[first, first + 1], &control).unwrap();
    let options = CertificateSearchOptions {
        conflict_limit: Some(100_000),
        ..Default::default()
    };
    assert!(matches!(
        local
            .search(0, &mut oracle, &options, &mut EventHandler::null())
            .unwrap(),
        Some(CertificateSearchOutcome::Found {
            steps: 1,
            shared_lines: 2,
            ..
        })
    ));
    assert!(local.encoding.circuit.next_var < 10_000);
}
