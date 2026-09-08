use super::*;
use std::collections::BTreeSet;

fn problem(text: &str) -> Problem {
    let mut problem = Problem::from_string(text).unwrap();
    problem.compute_diagram(&mut EventHandler::null());
    problem
}

fn signature(candidate: &Candidate, labels: &[Label]) -> Vec<usize> {
    candidate
        .order
        .iter()
        .flatten()
        .map(|&b| usize::from(b))
        .chain(labels.iter().map(|l| candidate.mapping[l]))
        .collect()
}

// Independent brute-force enumeration of numbered orders and their bounds.
// This does not use the SAT encoding or the fixed-point checker.
fn reference_candidates(problem: &Problem, nodes: usize) -> Vec<Candidate> {
    let mut labels = problem.labels();
    labels.sort();
    let pairs: Vec<_> = (1..nodes.saturating_sub(1))
        .flat_map(|a| (a + 1..nodes - 1).map(move |b| (a, b)))
        .collect();
    let mut result = Vec::new();
    for mask in 0..1usize << pairs.len() {
        let mut order = vec![vec![false; nodes]; nodes];
        for a in 0..nodes {
            order[a][a] = true;
            order[0][a] = true;
            order[a][nodes - 1] = true;
        }
        for (bit, &(a, b)) in pairs.iter().enumerate() {
            order[a][b] = mask & (1 << bit) != 0;
        }
        if (0..nodes).any(|a| {
            (0..nodes).any(|b| (0..nodes).any(|c| order[a][b] && order[b][c] && !order[a][c]))
        }) {
            continue;
        }
        let mut join = vec![vec![0; nodes]; nodes];
        let mut meet = join.clone();
        let mut valid = true;
        for a in 0..nodes {
            for b in 0..nodes {
                let uppers: Vec<_> = (0..nodes).filter(|&k| order[a][k] && order[b][k]).collect();
                let lowers: Vec<_> = (0..nodes).filter(|&k| order[k][a] && order[k][b]).collect();
                let joins: Vec<_> = uppers
                    .iter()
                    .copied()
                    .filter(|&k| uppers.iter().all(|&u| order[k][u]))
                    .collect();
                let meets: Vec<_> = lowers
                    .iter()
                    .copied()
                    .filter(|&k| lowers.iter().all(|&l| order[l][k]))
                    .collect();
                if joins.len() != 1 || meets.len() != 1 {
                    valid = false;
                } else {
                    join[a][b] = joins[0];
                    meet[a][b] = meets[0];
                }
            }
        }
        if !valid {
            continue;
        }
        for mut code in 0..nodes.pow(labels.len() as u32) {
            let mut mapping = HashMap::new();
            for &label in &labels {
                mapping.insert(label, code % nodes);
                code /= nodes;
            }
            if !problem
                .diagram_indirect
                .as_ref()
                .unwrap()
                .iter()
                .all(|(a, b)| order[mapping[a]][mapping[b]])
            {
                continue;
            }
            result.push(Candidate {
                order: order.clone(),
                mapping,
                join: join.clone(),
                meet: meet.clone(),
            });
        }
    }
    result
}

fn check(
    problem: &Problem,
    candidate: &Candidate,
) -> (
    Problem,
    Constraint,
    DashMap<Line, Tracking>,
    DashMap<Line, Tracking>,
) {
    let active_tracking = DashMap::new();
    let passive_tracking = DashMap::new();
    let names = (0..candidate.order.len())
        .map(|i| (i as Label, format!("(SAT{i})")))
        .collect();
    let (mut result, passive) = problem
        .fixpoint_onestep(
            false,
            &candidate.label_mapping(),
            &names,
            &candidate.diagram(),
            Some(&active_tracking),
            Some(&passive_tracking),
            &mut EventHandler::null(),
        )
        .unwrap();
    result.compute_triviality(&mut EventHandler::null());
    (result, passive, active_tracking, passive_tracking)
}

#[test]
fn encoding_matches_exhaustive_lattices_and_label_assignments() {
    for text in ["A A\nB B\n\nA B", "A A\nB B\n\nA AB"] {
        let problem = problem(text);
        let mut labels = problem.labels();
        labels.sort();
        for nodes in 1..=5 {
            let expected: BTreeSet<_> = reference_candidates(&problem, nodes)
                .iter()
                .map(|c| signature(c, &labels))
                .collect();
            let mut encoding = Encoding::new(&problem, nodes).unwrap();
            let mut actual = BTreeSet::new();
            while encoding.solver.solve().unwrap() == SolverResult::Sat {
                let candidate = encoding
                    .candidate(&encoding.solver.full_solution().unwrap())
                    .unwrap();
                assert!(
                    actual.insert(signature(&candidate, &labels)),
                    "Repeated candidate"
                );
                encoding.block_exact(&candidate).unwrap();
            }
            assert_eq!(
                actual, expected,
                "Encoding mismatch at {nodes} nodes for {text}"
            );
        }
    }
}

#[test]
fn learned_obstructions_preserve_all_small_good_candidates() {
    let mut compound_passive = false;
    let mut compound_game = false;
    for text in [
        "A A\nB B\n\nA B",
        "A B\n\nA A\nB B",
        "A A B\nA B B\n\nA B",
        "A A A\nB B B\nC C C\n\nA BC\nB C",
        include_str!("../../../examples/fixpoint_sat/maximal_matching.txt"),
    ] {
        let problem = problem(text);
        let mut cases = Vec::new();
        let mut obstructions = Vec::new();
        let mut oracle = NonexistenceOracle::new(&problem);
        let mut global_certificates = Vec::new();
        for nodes in 1..=5 {
            for candidate in reference_candidates(&problem, nodes) {
                let (result, passive, active, passive_tracking) = check(&problem, &candidate);
                let trivial = !result.trivial_sets.as_ref().unwrap().is_empty();
                let game_tracking = DashMap::new();
                let game = game::check(
                    &problem,
                    &candidate,
                    Some(&game_tracking),
                    true,
                    &mut EventHandler::null(),
                )
                .unwrap();
                assert_eq!(
                    !game.active_terms.is_empty(),
                    trivial,
                    "Game/full mismatch at {nodes} nodes for {text}: {:?}",
                    signature(&candidate, &problem.labels()),
                );
                let mut witnesses = game.active_terms.into_iter();
                if let Some(terms) = witnesses.next() {
                    compound_game |= terms.iter().any(|t| matches!(t, Term::Expr(..)));
                    let failure = failure_from_terms(
                        &problem,
                        terms,
                        witnesses.collect(),
                        &game.passive,
                        &candidate,
                        &game_tracking,
                    )
                    .unwrap();
                    assert!(candidate.satisfies(&failure.obstruction));
                    for terms in
                        std::iter::once(&failure.active_terms).chain(&failure.other_active_terms)
                    {
                        if let Some(certificate) = oracle.check(terms) {
                            global_certificates.push(certificate);
                        }
                    }
                    obstructions.push(failure.obstruction);
                }
                if trivial {
                    let failure = failure(
                        &problem,
                        &result,
                        &passive,
                        &candidate,
                        &active,
                        &passive_tracking,
                    )
                    .unwrap();
                    assert!(candidate.satisfies(&failure.obstruction));
                    compound_passive |= failure
                        .obstruction
                        .iter()
                        .any(|(a, _)| matches!(a, Term::Expr(..)));
                    if let Some(certificate) = oracle.check(&failure.active_terms) {
                        global_certificates.push(certificate);
                    }
                    obstructions.push(failure.obstruction);
                }
                cases.push((candidate, trivial));
            }
        }
        if cases.iter().any(|(_, trivial)| !trivial) {
            assert!(
                global_certificates.is_empty(),
                "Global nonexistence claimed despite a good candidate: {global_certificates:?}"
            );
        }
        for obstruction in &obstructions {
            for (candidate, trivial) in &cases {
                assert!(
                    !candidate.satisfies(obstruction) || *trivial,
                    "A blocker excluded a good diagram for {text}"
                );
            }
        }
        // Independently check the SAT evaluation of a compound certificate,
        // including transport from its original size to every other size.
        if let Some(obstruction) = obstructions.iter().max_by_key(|o| o.len()) {
            let mut labels = problem.labels();
            labels.sort();
            for nodes in 1..=5 {
                let expected: BTreeSet<_> = cases
                    .iter()
                    .filter(|(c, _)| c.order.len() == nodes && !c.satisfies(obstruction))
                    .map(|(c, _)| signature(c, &labels))
                    .collect();
                let mut encoding = Encoding::new(&problem, nodes).unwrap();
                encoding.block(obstruction).unwrap();
                let mut actual = BTreeSet::new();
                while encoding.solver.solve().unwrap() == SolverResult::Sat {
                    let candidate = encoding
                        .candidate(&encoding.solver.full_solution().unwrap())
                        .unwrap();
                    assert!(actual.insert(signature(&candidate, &labels)));
                    encoding.block_exact(&candidate).unwrap();
                }
                assert_eq!(actual, expected, "Incorrect encoded blocker");
            }
        }
    }
    assert!(
        compound_passive,
        "The fixtures must exercise dual passive operations"
    );
    assert!(
        compound_game,
        "The fixtures must exercise non-leaf game strategies"
    );
}

#[test]
fn merged_labels_keep_original_leaf_provenance() {
    let problem = problem("A B\n\nA A\nB B");
    let candidate = reference_candidates(&problem, 1).pop().unwrap();
    let (result, passive, active, passive_tracking) = check(&problem, &candidate);
    let failure = failure(
        &problem,
        &result,
        &passive,
        &candidate,
        &active,
        &passive_tracking,
    )
    .unwrap();
    let mut actual: Vec<_> = failure
        .active_terms
        .iter()
        .map(|t| match t {
            Term::Terminal(l) => *l,
            _ => panic!("Expected an original input line"),
        })
        .collect();
    actual.sort();
    let mut expected = problem.labels();
    expected.sort();
    assert_eq!(actual, expected);
    let game = game::check(&problem, &candidate, None, false, &mut EventHandler::null()).unwrap();
    let mut actual: Vec<_> = game.active_terms[0]
        .iter()
        .map(|t| match t {
            Term::Terminal(l) => *l,
            _ => panic!("Expected an original input line"),
        })
        .collect();
    actual.sort();
    assert_eq!(actual, expected);
}

#[test]
fn finds_a_good_diagram_after_rejecting_other_candidates() {
    let original = problem("A A\nB B\n\nA B");
    for (use_game, generalize) in [(false, false), (false, true), (true, false), (true, true)] {
        let options = SatSearchOptions {
            max_nodes: Some(4),
            generalize,
            use_game,
            check_nonexistence: generalize,
            ..Default::default()
        };
        let found = match original
            .fixpoint_sat(&options, &mut EventHandler::null())
            .unwrap()
        {
            SatSearchOutcome::Found(found) => found,
            other => panic!("Expected a fixed point, got {other:?}"),
        };
        assert_eq!(found.nodes, 4);
        assert!(found.stats.candidates > 1);
        if use_game {
            assert_eq!(found.stats.game_checks, found.stats.candidates);
            assert_eq!(found.stats.full_constructions, 1);
        } else {
            assert_eq!(found.stats.game_checks, 0);
            assert_eq!(found.stats.full_constructions, found.stats.candidates);
        }
        assert_eq!(found.stats.exhausted_sizes, vec![1, 2, 3]);
        assert!(found.problem.trivial_sets.as_ref().unwrap().is_empty());
        let label_text: HashMap<_, _> = found.problem.mapping_label_text.iter().cloned().collect();
        for &(a, b) in &found.mapping {
            let original_name = &original
                .mapping_label_text
                .iter()
                .find(|(l, _)| *l == a)
                .unwrap()
                .1;
            assert_eq!(&label_text[&b], original_name);
        }
        let names = found.problem.mapping_label_text.clone();
        let (mut replayed, _) = original
            .fixpoint_onestep(
                false,
                &found.mapping,
                &names,
                &found.diagram,
                None,
                None,
                &mut EventHandler::null(),
            )
            .unwrap();
        replayed.compute_triviality(&mut EventHandler::null());
        assert!(replayed.trivial_sets.unwrap().is_empty());
        let (mut custom, _, _) = original
            .fixpoint_custom(found.diagram_text, false, &mut EventHandler::null())
            .unwrap();
        custom.compute_triviality(&mut EventHandler::null());
        assert!(custom.trivial_sets.unwrap().is_empty());
    }
}

#[test]
fn finite_exhaustion_and_budget_are_not_global_nonexistence() {
    let problem = problem("A A\nB B\n\nA B");
    let options = SatSearchOptions {
        max_nodes: Some(3),
        ..Default::default()
    };
    assert!(matches!(
        problem
            .fixpoint_sat(&options, &mut EventHandler::null())
            .unwrap(),
        SatSearchOutcome::Exhausted {
            min_nodes: 1,
            max_nodes: 3,
            ..
        }
    ));
    let options = SatSearchOptions {
        max_candidates: Some(0),
        ..Default::default()
    };
    assert!(matches!(
        problem
            .fixpoint_sat(&options, &mut EventHandler::null())
            .unwrap(),
        SatSearchOutcome::Inconclusive { .. }
    ));
}

#[test]
fn preserves_global_nonexistence_certificate_and_symbolic_entry_point() {
    let problem = problem("A A\n\nA A");
    let outcome = problem
        .fixpoint_sat(&SatSearchOptions::default(), &mut EventHandler::null())
        .unwrap();
    assert!(
        matches!(outcome, SatSearchOutcome::NoFixedPoint { certificate, .. }
        if certificate.contains("any good diagram"))
    );
    assert!(problem
        .fixpoint_loop(&mut EventHandler::null())
        .unwrap_err()
        .contains("No fixed point"));
    assert!(problem
        .fixpoint_loop_symbolic(&mut EventHandler::null())
        .unwrap_err()
        .contains("No fixed point"));
}

#[test]
fn proves_nonexistence_for_an_initially_nontrivial_problem() {
    let mut problem = problem(include_str!(
        "../../../examples/fixpoint_sat/maximal_matching.txt"
    ));
    problem.compute_triviality(&mut EventHandler::null());
    assert!(problem.trivial_sets.as_ref().unwrap().is_empty());
    let options = SatSearchOptions {
        max_nodes: Some(8),
        max_candidates: Some(200),
        ..Default::default()
    };
    match problem
        .fixpoint_sat(&options, &mut EventHandler::null())
        .unwrap()
    {
        SatSearchOutcome::NoFixedPoint { certificate, stats } => {
            assert!(certificate.contains("Original expressions:"));
            assert!(stats.candidates > 1);
            assert!(stats.generalized_blockers > 0);
            assert_eq!(stats.game_checks, stats.candidates);
            assert_eq!(stats.full_constructions, 0);
        }
        other => panic!("Expected a derived all-size certificate, got {other:?}"),
    }
}

#[test]
fn native_loop_request_returns_a_checked_problem() {
    use crate::serial::{request_json, Request, Response};
    use std::sync::Mutex;

    let original = Problem::from_string("A A\nB B\n\nA B").unwrap();
    let original_names = original.mapping_label_text.clone();
    let request =
        serde_json::to_string(&Request::FixpointLoop(original, false, false, vec![])).unwrap();
    let results = Mutex::new(Vec::new());
    let sizes = Mutex::new(Vec::new());
    request_json(&request, |text, to_client| {
        if to_client {
            match serde_json::from_str::<Response>(&text).unwrap() {
                Response::P(problem) => results.lock().unwrap().push(problem),
                Response::E(error) => panic!("Native loop failed: {error}"),
                Response::Event(message, current, _) if message == "SAT: encoding lattice" => {
                    sizes.lock().unwrap().push(current);
                }
                _ => {}
            }
        }
    });
    let results = results.into_inner().unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].trivial_sets.as_ref().unwrap().is_empty());
    let names: HashMap<_, _> = results[0].mapping_label_text.iter().cloned().collect();
    let mapping: HashMap<_, _> = results[0]
        .mapping_oldlabel_labels
        .clone()
        .unwrap()
        .into_iter()
        .collect();
    assert!(results[0].mapping_label_oldlabels.is_none());
    for (old, text) in original_names {
        assert_eq!(mapping[&old].len(), 1);
        assert_eq!(names[&mapping[&old][0]], text);
    }
    assert_eq!(sizes.into_inner().unwrap(), vec![1, 2, 3, 4]);
}

#[test]
fn automatic_names_show_mergers_and_preserve_custom_diagram_replay() {
    for text in [
        "A A\nB B\nC C\n\nAB C",
        "(A=B) (A=B)\n(C->D) (C->D)\n\n(A=B) (C->D)",
    ] {
        let original = problem(text);
        let options = SatSearchOptions {
            max_nodes: Some(4),
            ..Default::default()
        };
        let SatSearchOutcome::Found(mut found) = original
            .fixpoint_sat(&options, &mut EventHandler::null())
            .unwrap()
        else {
            panic!("Expected a fixed point");
        };
        let original_names: HashMap<_, _> = original.mapping_label_text.iter().cloned().collect();
        let mut names: HashMap<_, _> = found.problem.mapping_label_text.iter().cloned().collect();
        for (&old, original_name) in &original_names {
            let node = found.mapping.iter().find(|(a, _)| *a == old).unwrap().1;
            let expected = if original_name == "A" || original_name == "B" {
                "(A=B)"
            } else {
                original_name
            };
            assert_eq!(names[&node], expected);
            assert!(found
                .diagram_text
                .contains(&format!("{original_name} = {expected}\n")));
        }
        let (mut replayed, _, _) = original
            .fixpoint_custom(found.diagram_text, false, &mut EventHandler::null())
            .unwrap();
        replayed.compute_triviality(&mut EventHandler::null());
        assert!(replayed.trivial_sets.unwrap().is_empty());
        // Display names must also round-trip as ordinary problem text.
        Problem::from_string(found.problem.to_string()).unwrap();

        // The GUI's normal simplification keeps this correspondence, and
        // subsequent manual renaming changes display names, not mapping IDs.
        crate::serial::fix_problem(&mut found.problem, true, true, &mut EventHandler::null());
        let mapping = found.problem.mapping_oldlabel_labels.clone().unwrap();
        names = found.problem.mapping_label_text.iter().cloned().collect();
        assert!(mapping
            .iter()
            .all(|(_, targets)| targets.len() == 1 && names.contains_key(&targets[0])));
        let rename: Vec<_> = found
            .problem
            .mapping_label_text
            .iter()
            .map(|(l, _)| (*l, format!("manual{l}")))
            .collect();
        found.problem.rename(&rename).unwrap();
        assert_eq!(
            found.problem.mapping_oldlabel_labels.as_ref().unwrap(),
            &mapping
        );
        assert_eq!(
            found
                .problem
                .mapping_oldlabel_text
                .as_ref()
                .unwrap()
                .iter()
                .cloned()
                .collect::<HashMap<_, _>>(),
            original_names
        );
    }
}

#[test]
fn automatic_names_avoid_original_and_generated_name_collisions() {
    let original = problem("A B (A=B) (FP0) (A=B_FP1_0)\n\nA B (A=B) (FP0) (A=B_FP1_0)");
    let by_name: HashMap<_, _> = original
        .mapping_label_text
        .iter()
        .map(|(l, s)| (s.as_str(), *l))
        .collect();
    let candidate = Candidate {
        order: vec![vec![false; 6]; 6],
        join: vec![],
        meet: vec![],
        mapping: [
            ("A", 1),
            ("B", 1),
            ("(A=B)", 2),
            ("(FP0)", 3),
            ("(A=B_FP1_0)", 4),
        ]
        .into_iter()
        .map(|(name, node)| (by_name[name], node))
        .collect(),
    };
    let mut result = original.clone();
    name_fixed_point(&original, &candidate, &mut result);
    let names: HashMap<_, _> = result.mapping_label_text.iter().cloned().collect();
    assert_eq!(names[&0], "(FP0_FP0_0)");
    assert_eq!(names[&1], "(A=B_FP1_1)");
    assert_eq!(names[&2], "(A=B)");
    assert_eq!(names[&3], "(FP0)");
    assert_eq!(names[&4], "(A=B_FP1_0)");
    assert_eq!(names[&5], "(FP5)");
    assert_eq!(names.values().collect::<HashSet<_>>().len(), names.len());
    let mut again = original.clone();
    name_fixed_point(&original, &candidate, &mut again);
    assert_eq!(result.mapping_label_text, again.mapping_label_text);
}

#[test]
fn readable_diagram_names_preserve_grouped_and_reverse_edges() {
    let (names, edges) = super::super::fixpoint::parse_diagram(
        "# readable mapping\nA = (X=Y)\nB = (Z<-W)\n\
         (bottom)->(X=Y) (Z<-W)\n(top)<-(X=Y) (Z<-W)\n\
         (bottom)->(bottom)\n(top)->(top)",
    );
    let names: HashMap<_, _> = names.into_iter().collect();
    assert_eq!(names.len(), 4);
    let edges: BTreeSet<_> = edges
        .into_iter()
        .map(|(a, b)| (names[&a].as_str(), names[&b].as_str()))
        .collect();
    assert_eq!(
        edges,
        BTreeSet::from([
            ("(bottom)", "(X=Y)"),
            ("(bottom)", "(Z<-W)"),
            ("(X=Y)", "(top)"),
            ("(Z<-W)", "(top)"),
            ("(bottom)", "(bottom)"),
            ("(top)", "(top)"),
        ])
    );
}

#[test]
fn validates_search_bounds_and_degrees() {
    let problem = problem("A A\n\nA A");
    let options = SatSearchOptions {
        min_nodes: 0,
        ..Default::default()
    };
    assert!(problem
        .fixpoint_sat(&options, &mut EventHandler::null())
        .is_err());
    let options = SatSearchOptions {
        min_nodes: 3,
        max_nodes: Some(2),
        ..Default::default()
    };
    assert!(problem
        .fixpoint_sat(&options, &mut EventHandler::null())
        .is_err());
    let problem = Problem::from_string("A A\n\nA A A").unwrap();
    assert!(problem
        .fixpoint_sat(&SatSearchOptions::default(), &mut EventHandler::null())
        .is_err());
}
