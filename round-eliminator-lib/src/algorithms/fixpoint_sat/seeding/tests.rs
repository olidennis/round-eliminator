use super::*;

fn problem(text: &str) -> Problem {
    let mut p = Problem::from_string(text).unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

fn source_terms(p: &Problem) -> Vec<Vec<Term>> {
    p.active
        .all_choices(true)
        .iter()
        .map(|l| {
            let mut row: Vec<_> = expanded(l).into_iter().map(Term::Terminal).collect();
            row.sort();
            row
        })
        .collect()
}

fn hard() -> Problem {
    problem(include_str!(
        "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ))
}

#[test]
fn bounded_completion_matches_the_gui_default_including_equivalent_labels() {
    for mut p in [
        hard(),
        problem("A A\nB B\n\nA B"),
        problem("A A B\nC B C\n\nABC ABC"),
    ] {
        let c = default_candidate(&p, MAX_NODES, &SearchControl::default())
            .unwrap()
            .unwrap();
        p.compute_default_fixpoint_diagram(None, false, vec![], &mut EventHandler::null());
        let json = serde_json::to_value(&p.fixpoint_diagram.as_ref().unwrap().1).unwrap();
        let mapping: HashMap<Label, usize> =
            serde_json::from_value::<Vec<(Label, usize)>>(json["mapping_label_newlabel"].clone())
                .unwrap()
                .into_iter()
                .collect();
        let edges: HashSet<(Label, Label)> =
            serde_json::from_value::<Vec<(Label, Label)>>(json["diagram"].clone())
                .unwrap()
                .into_iter()
                .collect();
        assert_eq!(c.mapping, mapping);
        assert_eq!(c.diagram().into_iter().collect::<HashSet<_>>(), edges);
        for a in 0..c.order.len() {
            for b in 0..c.order.len() {
                assert!(c.order[a][c.join[a][b]] && c.order[b][c.join[a][b]]);
                assert!(c.order[c.meet[a][b]][a] && c.order[c.meet[a][b]][b]);
            }
        }
    }
}

#[test]
fn hard_default_seed_retains_dominated_lines_and_replays_without_a_certificate() {
    let p = hard();
    let control = SearchControl::default();
    let mut count = 0;
    let mut events = EventHandler::with(|(message, current, _)| {
        if message == "Proof: default diagram derivations extracted" {
            count = current;
        }
    });
    let dag = collect(&p, &mut events, &control).unwrap().unwrap();
    drop(events);
    assert!(
        count > 256,
        "Must not truncate the default seed to the old archive size"
    );
    let values = dag.replay(&source_terms(&p), 4, &control).unwrap();
    assert_eq!(values.len(), dag.steps.len());
    let c = default_candidate(&p, MAX_NODES, &control).unwrap().unwrap();
    let mut oracle = NonexistenceOracle::new(&p);
    for (i, row) in values.iter().enumerate() {
        assert!(oracle.check(row).is_none());
        if let Step::Combine {
            parents,
            permutations,
            pivot,
        } = &dag.steps[i]
        {
            for j in 0..4 {
                let a = c.eval(&values[parents[0]][permutations[0][j]]);
                let b = c.eval(&values[parents[1]][permutations[1][j]]);
                assert_eq!(
                    c.eval(&row[j]),
                    if j == *pivot {
                        c.join[a][b]
                    } else {
                        c.meet[a][b]
                    }
                );
            }
        }
    }
}

#[test]
fn merged_labels_keep_whole_original_input_occurrences() {
    let p = problem("A A B\nC B C\n\nABC ABC");
    let control = SearchControl::default();
    let c = default_candidate(&p, MAX_NODES, &control).unwrap().unwrap();
    assert!(c.mapping.values().collect::<HashSet<_>>().len() < p.labels().len());
    let dag = collect(&p, &mut EventHandler::null(), &control)
        .unwrap()
        .unwrap();
    assert!(!dag
        .replay(&source_terms(&p), 3, &control)
        .unwrap()
        .is_empty());
}

#[test]
fn completion_and_saturation_limits_are_only_optional_seed_limits() {
    let p = hard();
    let control = SearchControl::default();
    assert!(default_candidate(&p, 19, &control).unwrap().is_none());
    assert_eq!(
        default_candidate(&p, 20, &control)
            .unwrap()
            .unwrap()
            .order
            .len(),
        20
    );
    let result = collect_bounded(
        &p,
        &mut EventHandler::null(),
        &control,
        MAX_NODES,
        MAX_STEPS,
        Duration::ZERO,
    )
    .unwrap()
    .unwrap();
    result.replay(&source_terms(&p), 4, &control).unwrap();
    assert!(
        control.check().is_ok(),
        "Bootstrap budget must not cancel the other workers"
    );
    control.stop();
    assert!(collect(&p, &mut EventHandler::null(), &control).is_err());
}

#[test]
fn stop_during_default_bootstrap_propagates_to_its_owner() {
    let p = hard();
    let control = SearchControl::default();
    let mut events = EventHandler::with(|(message, _, _)| {
        if message == "Proof: saturating default diagram" {
            control.stop();
        }
    });
    assert!(collect(&p, &mut events, &control).is_err());
}

#[test]
fn cyclic_tracking_is_rejected() {
    let p = hard();
    let line = p.active.lines[0].clone();
    let tracking = DashMap::new();
    tracking.insert(
        line.clone(),
        (line.clone(), line.clone(), line.clone(), vec![], vec![]),
    );
    assert!(append_tracking(
        &line,
        &tracking,
        &mut HashMap::new(),
        &mut Derivation::default(),
        4,
        &SearchControl::default()
    )
    .is_err());
}
