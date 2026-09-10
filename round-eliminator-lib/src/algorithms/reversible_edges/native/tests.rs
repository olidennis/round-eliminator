use super::*;

fn p(s: &str) -> Problem {
    Problem::from_string(s).unwrap()
}
fn label(p: &Problem, name: &str) -> Label {
    p.mapping_label_text
        .iter()
        .find(|(_, s)| s == name)
        .unwrap()
        .0
}
fn budget(options: &Options) -> Budget<'_> {
    Budget {
        options,
        deadline: Instant::now() + Duration::from_secs(20),
    }
}

#[test]
fn attempt_panics_report_the_original_error_and_recipe() {
    let mut original = p("A A\nB B\n\nA B");
    let b = label(&original, "B");
    // Deliberately corrupt the private worker's input to exercise a real
    // indexing panic, rather than the outer scope's generic replacement panic.
    original.passive.lines[0].parts.clear();
    let options = Options {
        threads: 2,
        ..Default::default()
    };
    let error = portfolio::run(
        &original,
        None,
        &[vec![[b, b]]],
        &[vec![vec![]]],
        &options,
        Instant::now() + Duration::from_secs(5),
        &mut EventHandler::null(),
        |_, _, _| panic!("a failed attempt must not publish a certificate"),
    )
    .err()
    .expect("corrupt input must be reported");
    assert!(
        error.contains("Direct reversible-edge attempt panicked"),
        "{error}"
    );
    assert!(error.contains("B B (recipe 1: [])"), "{error}");
    assert!(error.contains("index out of bounds"), "{error}");
    assert!(!error.contains("a scoped thread panicked"), "{error}");
}

#[test]
fn two_gui_speedups_then_both_reversible_edge_targets() {
    let mut original = p("B C C\nB B C\nA A C\nB B B\n\nA B\nC C");
    let mut eh = EventHandler::null();
    crate::serial::fix_problem(&mut original, true, true, &mut eh);
    for _ in 0..2 {
        if original.diagram_indirect.is_none() {
            original.compute_partial_diagram(&mut eh);
        }
        original = original.speedup(&mut eh);
        crate::serial::fix_problem(&mut original, true, true, &mut eh);
    }
    assert_eq!(original.labels().len(), 4);
    let report = search(
        &original,
        &Options {
            seconds: 15,
            threads: 4,
            ..Default::default()
        },
        &mut eh,
        |_| {},
    )
    .unwrap();
    assert!(report.stats.mapping_attempts > report.stats.re2_mapping_attempts);
    assert!(report.stats.re2_mapping_attempts > 0);
    for certificate in &report.certificates {
        apply(&original, certificate, &mut eh).unwrap();
    }
}

#[test]
fn mis_witnesses_require_independence_maximality_and_the_right_subgraph() {
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let original = p("A B\n\nA A\nA B\nB B");
    let input = Input::new(&original, &b, &eh).unwrap();
    let a = label(&original, "A");
    let bb = label(&original, "B");
    let mis = input
        .step(&Step::Mis(Subgraph::Pairs(vec![[a, a]])), 1, &b, &eh)
        .unwrap();
    let ai = input.base.iter().position(|&x| x == a).unwrap();
    let bi = input.base.iter().position(|&x| x == bb).unwrap();
    assert!(!mis.edges.contains(&annotations::edge(3 * ai, 3 * ai)));
    assert!(mis.edges.contains(&annotations::edge(3 * ai, 3 * ai + 2)));
    assert!(mis.edges.contains(&annotations::edge(3 * ai, 3 * bi)));
    assert!(!mis.edges.iter().any(|e| e.contains(&(3 * bi + 2))));
    assert!(mis.nodes.iter().all(|r| r.iter().all(|s| s % 3 == 0)
        || (r.iter().all(|s| s % 3 != 0) && r.iter().filter(|s| *s % 3 == 2).count() == 1)));
}

#[test]
fn independent_label_mis_results_can_be_used_jointly_without_merging_labels() {
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let original = p("A A B B\n\nA A\nB B");
    let target = p("(AM) (AM) (BM) (BM)\n(AM) (AM) (BP) (BU)\n(AP) (AU) (BM) (BM)\n(AP) (AU) (BP) (BU)\n\n(AM) (AP)\n(AM) (AU)\n(AU) (AU)\n(BM) (BP)\n(BM) (BU)\n(BU) (BU)");
    let aa = label(&original, "A");
    let bb = label(&original, "B");
    let input = Input::new(&original, &b, &eh).unwrap();
    let target = Input::new(&target, &b, &eh).unwrap();
    let one = input
        .step(&Step::Mis(Subgraph::Pairs(vec![[aa, aa]])), 1, &b, &eh)
        .unwrap();
    assert!(mapping::find(&one, &target, &b, &mut eh).unwrap().is_none());
    let both = one
        .step(&Step::Mis(Subgraph::Pairs(vec![[bb, bb]])), 2, &b, &eh)
        .unwrap();
    assert!(both.base.iter().all(|&l| l == aa || l == bb));
    let found = mapping::find(&both, &target, &b, &mut eh)
        .unwrap()
        .expect("Joint MIS results should solve both independent subgraphs");
    mapping::verify(&both, &target, &found, &b, &eh).unwrap();
}

#[test]
fn mis_reverse_certificate_roundtrips_and_tampering_is_rejected() {
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let original = p("M M M\nP U U\n\nM P\nM U\nU U");
    let m = label(&original, "M");
    let added = vec![[m, m]];
    let q = relaxation(&original, &added).unwrap();
    assert!(attempt(&original, &q, &added, &[], &b, &mut eh)
        .unwrap()
        .is_none());
    let c = attempt(
        &original,
        &q,
        &added,
        &[Step::Mis(Subgraph::All)],
        &b,
        &mut eh,
    )
    .unwrap()
    .unwrap();
    let encoded = serde_json::to_string(&c).unwrap();
    let c: Certificate = serde_json::from_str(&encoded).unwrap();
    let applied = apply(&original, &c, &mut eh).unwrap();
    assert_eq!(applied.active, original.active);
    assert_eq!(applied.mapping_label_text, original.mapping_label_text);
    assert!(applied.passive.includes(&edge_line([m, m])));
    assert!(applied.diagram_indirect.is_none());
    let mut bad = c.clone();
    bad.mapping[0].output.fill(m);
    for row in &mut bad.mapping {
        row.output.fill(m);
    }
    assert!(apply(&original, &bad, &mut eh).is_err());
    let mut bad = c;
    bad.mapping.pop();
    assert!(apply(&original, &bad, &mut eh).is_err());
}

#[test]
fn search_returns_verified_singles_and_rechecks_joint_sets() {
    let original = p("M M M\nP U U\n\nM P\nM U\nU U");
    let options = Options {
        seconds: 10,
        ..Default::default()
    };
    let mut updates = 0;
    let report = search(&original, &options, &mut EventHandler::null(), |_| {
        updates += 1
    })
    .unwrap();
    assert!(updates > 1);
    assert!(report.certificates.iter().any(|c| c.added.len() > 1));
    for c in &report.certificates {
        apply(&original, c, &mut EventHandler::null()).unwrap();
    }
    assert!(report.message.contains("NOT proved irreversible"));
}

#[test]
fn individually_reversible_additions_need_not_be_jointly_reversible() {
    // AB and CD are disconnected bipartite components. AC alone or AD alone
    // can be reversed by choosing how to flip the second component. Together
    // they admit an A-C-D triangle, which cannot map to a bipartite problem.
    let original = p("A A\nB B\nC C\nD D\n\nA B\nC D");
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let a = label(&original, "A");
    let c = label(&original, "C");
    let d = label(&original, "D");
    let ac = pair(a, c);
    let ad = pair(a, d);
    let first = attempt(
        &original,
        &relaxation(&original, &[ac]).unwrap(),
        &[ac],
        &[],
        &b,
        &mut eh,
    )
    .unwrap()
    .unwrap();
    assert!(attempt(
        &original,
        &relaxation(&original, &[ad]).unwrap(),
        &[ad],
        &[],
        &b,
        &mut eh
    )
    .unwrap()
    .is_some());
    let mut forged = first;
    forged.added.push(ad);
    forged.added.sort();
    assert!(apply(&original, &forged, &mut eh)
        .unwrap_err()
        .contains("edge configuration"));
    let report = search(
        &original,
        &Options {
            seconds: 3,
            ..Default::default()
        },
        &mut eh,
        |_| {},
    )
    .unwrap();
    assert!(!report
        .certificates
        .iter()
        .any(|c| c.added.contains(&ac) && c.added.contains(&ad)));
}

#[test]
fn proper_coloring_and_exchange_have_verified_mapping_paths() {
    let original = p("A A\nB B\nC C\n\nA B\nA C\nB C");
    let a = label(&original, "A");
    let added = vec![[a, a]];
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let q = relaxation(&original, &added).unwrap();
    for recipe in [vec![Step::Coloring], vec![Step::Coloring, Step::Exchange]] {
        let c = attempt(&original, &q, &added, &recipe, &b, &mut eh)
            .unwrap()
            .unwrap();
        apply(&original, &c, &mut eh).unwrap();
    }
}

#[test]
fn serialized_gui_search_and_apply_preserve_exact_node_constraint() {
    use crate::serial::{request_json, Request, Response};
    use std::sync::Mutex;
    let original = p("M M M\nP U U\n\nM P\nM U\nU U");
    let request = serde_json::to_string(&Request::ReversibleEdges(
        original.clone(),
        Options::default(),
    ))
    .unwrap();
    let saved = Mutex::new(None);
    request_json(&request, |s, primary| {
        if primary {
            match serde_json::from_str::<Response>(&s).unwrap() {
                Response::ReversibleEdges(r) => *saved.lock().unwrap() = Some(r),
                Response::E(e) => panic!("Search failed: {e}"),
                _ => {}
            }
        }
    });
    let report = saved.into_inner().unwrap().unwrap();
    let c = report
        .certificates
        .into_iter()
        .find(|c| c.added.len() > 1)
        .unwrap();
    let request =
        serde_json::to_string(&Request::ApplyReversibleEdges(original.clone(), c)).unwrap();
    let saved = Mutex::new(None);
    request_json(&request, |s, primary| {
        if primary {
            match serde_json::from_str::<Response>(&s).unwrap() {
                Response::P(p) => *saved.lock().unwrap() = Some(p),
                Response::E(e) => panic!("Apply failed: {e}"),
                _ => {}
            }
        }
    });
    let applied = saved.into_inner().unwrap().unwrap();
    assert_eq!(applied.active, original.active);
    assert_eq!(applied.mapping_label_text, original.mapping_label_text);
    assert!(applied.diagram_direct.is_some());
    assert!(applied.diagram_indirect.is_some());
}

#[test]
fn recursive_search_applies_results_until_no_pair_is_missing() {
    let original = p("A A\nB B\n\nA A\nA B");
    let mut steps = vec![];
    let result = recursive(
        original.clone(),
        &Options {
            seconds: 2,
            threads: 1,
            ..Default::default()
        },
        &mut EventHandler::null(),
        |step, certificate| steps.push((step, certificate.added.clone())),
    )
    .unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].0, 1);
    assert!(steps[0]
        .1
        .iter()
        .all(|&edge| result.passive.includes(&edge_line(edge))));
    assert_eq!(result.active, original.active);
    assert_eq!(result.mapping_label_text, original.mapping_label_text);
}

#[test]
fn invalid_inputs_and_budgets_are_not_negative_proofs() {
    let original = p("A A\nB B\n\nA B");
    let options = Options {
        max_variables: 1,
        attempt_ms: 10,
        seconds: 1,
        ..Default::default()
    };
    let report = search(&original, &options, &mut EventHandler::null(), |_| {}).unwrap();
    assert!(report.certificates.is_empty());
    assert!(!report.complete);
    assert!(report.stats.bounded_attempts > 0);
    assert!(search(
        &original,
        &Options {
            seconds: 0,
            ..Default::default()
        },
        &mut EventHandler::null(),
        |_| {}
    )
    .is_err());
    assert!(relaxation(&original, &[[999, 999]]).is_err());
}

// Every pair is allowed except the highest label paired with itself.
// Each label also occurs in the degree-one node constraint.
fn wide_alphabet_problem(count: usize) -> Problem {
    let names: Vec<_> = (0..count).map(|i| format!("(L{i})")).collect();
    let all = names.concat();
    let without_last = names[..count - 1].concat();
    p(&format!("{all}\n\n{all} {without_last}"))
}

#[test]
fn label_limit_accepts_64_and_rejects_65_or_empty_nodes() {
    let options = Options::default();
    for count in [32, 33, 64] {
        let original = wide_alphabet_problem(count);
        assert_eq!(original.labels().len(), count);
        validate(&original, &options).unwrap();
    }
    assert!(validate(&wide_alphabet_problem(65), &options)
        .unwrap_err()
        .contains("1–64 labels"));
    let mut empty = wide_alphabet_problem(64);
    empty.active.lines.clear();
    assert!(validate(&empty, &options)
        .unwrap_err()
        .contains("nonempty node constraint"));
}

#[test]
fn search_and_certificate_replay_support_64_labels() {
    let original = wide_alphabet_problem(64);
    let last = label(&original, "(L63)");
    let report = search(
        &original,
        &Options {
            seconds: 10,
            max_candidates: 1,
            threads: 1,
            ..Default::default()
        },
        &mut EventHandler::null(),
        |_| {},
    )
    .unwrap();
    assert_eq!(report.certificates.len(), 1);
    assert_eq!(report.certificates[0].added, vec![[last, last]]);
    let result = apply(
        &original,
        &report.certificates[0],
        &mut EventHandler::null(),
    )
    .unwrap();
    assert_eq!(result.active, original.active);
    assert_eq!(result.mapping_label_text, original.mapping_label_text);
    assert_eq!(result.labels().len(), 64);
    assert!(result.passive.includes(&edge_line([last, last])));
}

#[test]
fn certificate_pair_limits_cover_the_64_label_alphabet() {
    let original = wide_alphabet_problem(64);
    let labels = original.labels();
    let first = labels[0];
    let last = label(&original, "(L63)");
    let added = vec![[last, last]];
    let q = relaxation(&original, &added).unwrap();
    let pairs: Vec<_> = labels
        .iter()
        .enumerate()
        .flat_map(|(i, &a)| labels[i..].iter().map(move |&b| [a, b]))
        .collect();
    assert_eq!(pairs.len(), 2080);
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    for step in [
        Step::Mis(Subgraph::Pairs(pairs.clone())),
        // Keep the first self-pair good: every bad degree-one edge can be
        // repaired by changing both its endpoints to this first label.
        Step::RepairPairs(pairs[1..].to_vec()),
    ] {
        let input = transformed(&q, &[step.clone()], &b, &mut eh).unwrap();
        let mut certificate = Certificate {
            target: None,
            added: added.clone(),
            recipe: vec![step],
            mapping: input
                .nodes
                .iter()
                .map(|row| MappingRow {
                    input: row.iter().map(|&s| input.names[s].clone()).collect(),
                    output: vec![first; row.len()],
                })
                .collect(),
        };
        let result = apply(&original, &certificate, &mut eh).unwrap();
        assert_eq!(result.active, original.active);
        assert!(result.passive.includes(&edge_line([last, last])));
        let recipe_pairs = match &mut certificate.recipe[0] {
            Step::Mis(Subgraph::Pairs(pairs)) | Step::RepairPairs(pairs) => pairs,
            _ => unreachable!(),
        };
        recipe_pairs.resize(2081, [first, first]);
        assert!(apply(&original, &certificate, &mut eh)
            .unwrap_err()
            .contains("Invalid"));
    }
}

#[test]
fn stop_callback_cancels_and_joins_the_sat_worker() {
    let original = p("M M M\nP U U\n\nM P\nM U\nU U");
    let options = Options::default();
    let stopped = std::sync::atomic::AtomicBool::new(false);
    let mut eh = EventHandler::with(|(message, _, _)| {
        if message == "Reversible edges: solving reverse mapping" {
            stopped.store(true, std::sync::atomic::Ordering::Relaxed);
            panic!("simulated STOP");
        }
    });
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search(&original, &options, &mut eh, |_| {})
    }))
    .is_err());
    assert!(stopped.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn stronger_recipes_roundtrip_and_reject_invalid_priorities() {
    let original = p("A A\nB B\n\nA A\nA B");
    let a = label(&original, "A");
    let bb = label(&original, "B");
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let q = relaxation(&original, &[[bb, bb]]).unwrap();
    let order = vec![vec![a, a], vec![bb, bb]];
    let recipes = vec![
        vec![Step::NodeContext, Step::Exchange, Step::Prune],
        vec![Step::Matching(Subgraph::All), Step::Prune],
        vec![Step::Matching(Subgraph::Pairs(vec![[a, a]])), Step::Prune],
        vec![Step::GreedyColoring(Subgraph::All), Step::Prune],
        vec![
            Step::GreedyColoring(Subgraph::Pairs(vec![[a, a]])),
            Step::Prune,
        ],
        vec![Step::RulingSet(Subgraph::All), Step::Prune],
        vec![Step::RulingSet(Subgraph::Pairs(vec![[a, a]])), Step::Prune],
        vec![
            Step::PriorityMis {
                graph: Subgraph::All,
                order: order.clone(),
            },
            Step::Prune,
        ],
    ];
    for recipe in recipes {
        let c = attempt(&original, &q, &[[bb, bb]], &recipe, &b, &mut eh)
            .unwrap()
            .unwrap();
        let c: Certificate = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        let result = apply(&original, &c, &mut eh).unwrap();
        assert_eq!(result.active, original.active);
        assert_eq!(result.mapping_label_text, original.mapping_label_text);
    }
    let mut c = attempt(
        &original,
        &q,
        &[[bb, bb]],
        &[Step::PriorityMis {
            graph: Subgraph::All,
            order,
        }],
        &b,
        &mut eh,
    )
    .unwrap()
    .unwrap();
    if let Step::PriorityMis { order, .. } = &mut c.recipe[0] {
        order.push(order[0].clone());
    }
    assert!(apply(&original, &c, &mut eh)
        .unwrap_err()
        .contains("priority order"));
}

fn permits_cycle(input: &Input, states: &[[usize; 2]]) -> bool {
    states.iter().enumerate().all(|(v, &[left, right])| {
        input.nodes.contains(&{
            let mut r = vec![left, right];
            r.sort();
            r
        }) && input
            .edges
            .contains(&annotations::edge(right, states[(v + 1) % states.len()][0]))
    })
}

#[test]
fn ruling_set_represents_all_square_mis_outcomes_on_short_cycles() {
    let original = p("A A\n\nA A");
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let input = Input::new(&original, &b, &eh)
        .unwrap()
        .step(&Step::RulingSet(Subgraph::All), 1, &b, &eh)
        .unwrap();
    let state = |c, d| {
        input
            .names
            .iter()
            .position(|s| s.ends_with(&format!("ruling1={c}, neighbor={d}")))
            .unwrap()
    };
    for n in 3..=8 {
        let mut realizable = 0;
        for mask in 1usize..(1 << n) {
            let centers: Vec<_> = (0..n).filter(|&v| mask & (1 << v) != 0).collect();
            let distance = |v: usize, w: usize| {
                let d = v.abs_diff(w);
                d.min(n - d)
            };
            if centers
                .iter()
                .any(|&v| centers.iter().any(|&w| v != w && distance(v, w) <= 2))
            {
                continue;
            }
            let dist: Vec<_> = (0..n)
                .map(|v| centers.iter().map(|&w| distance(v, w)).min().unwrap())
                .collect();
            if dist.iter().any(|&d| d > 2) {
                continue;
            }
            let states: Vec<_> = (0..n)
                .map(|v| {
                    [
                        state(dist[v], dist[(v + n - 1) % n]),
                        state(dist[v], dist[(v + 1) % n]),
                    ]
                })
                .collect();
            assert!(permits_cycle(&input, &states), "n={n} centers={centers:?}");
            realizable += 1;
        }
        assert!(realizable > 0);
    }
    // Two centers at distance two share a distance-one vertex, which is illegal.
    let invalid = [0, 1, 0, 1];
    let states: Vec<_> = (0..4)
        .map(|v| {
            [
                state(invalid[v], invalid[(v + 3) % 4]),
                state(invalid[v], invalid[(v + 1) % 4]),
            ]
        })
        .collect();
    assert!(!permits_cycle(&input, &states));
}

#[test]
fn matching_represents_oriented_maximal_matchings_and_rejects_unmatched_edges() {
    let original = p("A A\n\nA A");
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let input = Input::new(&original, &b, &eh)
        .unwrap()
        .step(&Step::Matching(Subgraph::All), 1, &b, &eh)
        .unwrap();
    let state = |r| {
        input
            .names
            .iter()
            .position(|s| s.ends_with(&format!("match1={r}")))
            .unwrap()
    };
    for n in 3..=7 {
        for mask in 0usize..1 << n {
            let selected: Vec<_> = (0..n).filter(|&v| mask & (1 << v) != 0).collect();
            if selected.iter().any(|&v| selected.contains(&((v + 1) % n))) {
                continue;
            }
            let matched = |v| selected.contains(&v) || selected.contains(&((v + n - 1) % n));
            if (0..n).any(|v| !matched(v) && !matched((v + 1) % n)) {
                continue;
            }
            for orientation in 0usize..1 << selected.len() {
                let mut states = vec![[state("U"); 2]; n];
                for (i, &v) in selected.iter().enumerate() {
                    let [v_role, w_role] = if orientation & (1 << i) == 0 {
                        [("H", "h"), ("T", "t")]
                    } else {
                        [("T", "t"), ("H", "h")]
                    };
                    states[v] = [state(v_role.1), state(v_role.0)];
                    states[(v + 1) % n] = [state(w_role.0), state(w_role.1)];
                }
                assert!(permits_cycle(&input, &states));
            }
        }
    }
    assert!(!permits_cycle(&input, &vec![[state("U"); 2]; 5]));
}

#[test]
fn parallel_and_sequential_searches_publish_the_same_verified_sets_on_small_inputs() {
    let original = p("A A\nB B\nC C\n\nA A\nA B");
    let mut sets = vec![];
    for threads in [1, 4] {
        let options = Options {
            seconds: 10,
            threads,
            ..Default::default()
        };
        let report = search(&original, &options, &mut EventHandler::null(), |_| {}).unwrap();
        for c in &report.certificates {
            apply(&original, c, &mut EventHandler::null()).unwrap();
        }
        sets.push(
            report
                .certificates
                .iter()
                .map(|c| c.added.clone())
                .collect::<BTreeSet<_>>(),
        );
    }
    assert!(!sets[0].is_empty());
    assert_eq!(sets[0], sets[1]);
}

#[test]
fn portfolio_includes_all_generic_families_and_combined_rules() {
    let original = p(include_str!(
        "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let a = label(&original, "A");
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let q = relaxation(&original, &[[a, a]]).unwrap();
    let recipes = schedule::recipes(&q, &[[a, a]], &b, &eh).unwrap();
    for wanted in [
        "Mis",
        "Matching",
        "GreedyColoring",
        "RulingSet",
        "PriorityMis",
        "NodeContext",
        "Exchange",
        "Prune",
        "FindMis",
        "RepairPairs",
    ] {
        assert!(
            recipes
                .iter()
                .flatten()
                .any(|s| format!("{s:?}").starts_with(wanted)),
            "{wanted}"
        );
    }
    assert!(recipes.iter().any(|r| r
        .iter()
        .filter(|s| matches!(s, Step::GreedyColoring(_)))
        .count()
        == 2));
    assert!(recipes
        .iter()
        .any(|r| r.iter().filter(|s| matches!(s, Step::Mis(_))).count() >= 2));
    assert!(recipes.iter().any(
        |r| matches!(r.first(), Some(Step::PriorityMis { .. })) && r.contains(&Step::Exchange)
    ));
}

#[test]
fn priority_portfolio_skips_only_irrelevant_order_comparisons() {
    let original = p("A A\nB B\nC C\n\nA A\nB B\nC C");
    let a = label(&original, "A");
    let bb = label(&original, "B");
    let added = vec![pair(a, bb)];
    let q = relaxation(&original, &added).unwrap();
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let recipes = schedule::recipes(&q, &added, &b, &eh).unwrap();
    let orders: BTreeSet<_> = recipes
        .iter()
        .filter_map(|r| match r.first() {
            Some(Step::PriorityMis {
                graph: Subgraph::All,
                order,
            }) => Some(order.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(orders.len(), 2);
    let signs: BTreeSet<_> = orders
        .iter()
        .map(|order| {
            order.iter().position(|r| r == &vec![a, a]).unwrap()
                < order.iter().position(|r| r == &vec![bb, bb]).unwrap()
        })
        .collect();
    assert_eq!(signs, BTreeSet::from([false, true]));
}

#[test]
fn symbolic_mis_context_activation_matches_every_small_concrete_predicate() {
    let q = p("A B\n\nAB AB");
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    for stages in 1..=2 {
        let symbolic = synthesis::symbolic(&q, stages, &b, &eh).unwrap();
        let parameters = stages * symbolic.pairs.len();
        for mask in 0usize..1 << parameters {
            let chosen: Vec<_> = (0..parameters).map(|i| mask & (1 << i) != 0).collect();
            let mut values = vec![];
            for condition in &symbolic.activation.conditions {
                values.push(match condition {
                    mapping::Condition::Parameter(i, positive) => chosen[*i] == *positive,
                    mapping::Condition::All(children) => children.iter().all(|&c| values[c]),
                    mapping::Condition::Any(children) => children.iter().any(|&c| values[c]),
                });
            }
            let mut concrete = Input::new(&q, &b, &eh).unwrap();
            for stage in 0..stages {
                let graph = Subgraph::Pairs(
                    symbolic
                        .pairs
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &p)| chosen[stage * symbolic.pairs.len() + i].then_some(p))
                        .collect(),
                );
                concrete = concrete
                    .step(&Step::Mis(graph), stage + 1, &b, &eh)
                    .unwrap();
            }
            let nodes: Vec<_> = symbolic
                .input
                .nodes
                .iter()
                .zip(&symbolic.activation.roots)
                .filter(|(_, root)| root.map_or(true, |r| values[r]))
                .map(|(r, _)| r.clone())
                .collect();
            let edges: BTreeSet<_> = symbolic
                .input
                .edges
                .iter()
                .filter(|e| {
                    symbolic.guards.get(*e).map_or(true, |conditions| {
                        conditions
                            .iter()
                            .all(|&(i, positive)| chosen[i] == positive)
                    })
                })
                .copied()
                .collect();
            assert_eq!(nodes, concrete.nodes, "stages={stages} mask={mask}");
            assert_eq!(edges, concrete.edges, "stages={stages} mask={mask}");
        }
    }
}

#[test]
fn synthesized_subgraphs_produce_concrete_independently_verified_certificates() {
    let original = p("M M M\nP U U\n\nM P\nM U\nU U");
    let m = label(&original, "M");
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let q = relaxation(&original, &[[m, m]]).unwrap();
    for stages in 1..=3 {
        let c = attempt(
            &original,
            &q,
            &[[m, m]],
            &[Step::FindMis(stages)],
            &b,
            &mut eh,
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.recipe.len(), stages);
        assert!(c
            .recipe
            .iter()
            .all(|s| matches!(s, Step::Mis(Subgraph::Pairs(_)))));
        apply(&original, &c, &mut eh).unwrap();
    }
    let mut c = attempt(
        &original,
        &q,
        &[[m, m]],
        &[Step::Mis(Subgraph::All)],
        &b,
        &mut eh,
    )
    .unwrap()
    .unwrap();
    c.recipe = vec![Step::FindMis(1)];
    assert!(apply(&original, &c, &mut eh)
        .unwrap_err()
        .contains("Unresolved"));
}

#[test]
fn two_endpoint_repair_proves_three_coloring_but_not_two_coloring() {
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    for colors in [2, 3] {
        let original = if colors == 2 {
            p("A A\nB B\n\nA B")
        } else {
            p("A A\nB B\nC C\n\nA B\nA C\nB C")
        };
        let a = label(&original, "A");
        let q = relaxation(&original, &[[a, a]]).unwrap();
        let c = attempt(
            &original,
            &q,
            &[[a, a]],
            &[Step::RepairPairs(vec![[a, a]])],
            &b,
            &mut eh,
        )
        .unwrap();
        if colors == 2 {
            assert!(c.is_none());
        } else {
            let c = c.unwrap();
            assert_eq!(c.mapping.len(), 3);
            apply(&original, &c, &mut eh).unwrap();
        }
    }
}

#[test]
fn greedy_coloring_requires_lower_neighbor_colors_and_priority_mis_requires_earlier_parents() {
    let options = Options::default();
    let b = budget(&options);
    let eh = EventHandler::null();
    let p1 = p("A A\n\nA A");
    let colored = Input::new(&p1, &b, &eh)
        .unwrap()
        .step(&Step::GreedyColoring(Subgraph::All), 1, &b, &eh)
        .unwrap();
    let state = |c, d| {
        colored
            .names
            .iter()
            .position(|s| s.ends_with(&format!("greedy1={c}, neighbor={d}")))
            .unwrap()
    };
    assert!(permits_cycle(
        &colored,
        &[
            [state(0, 1); 2],
            [state(1, 0); 2],
            [state(0, 1); 2],
            [state(1, 0); 2]
        ]
    ));
    assert!(!permits_cycle(
        &colored,
        &[
            [state(0, 2); 2],
            [state(2, 0); 2],
            [state(0, 2); 2],
            [state(2, 0); 2]
        ]
    ));
    let p2 = p("A A\nB B\n\nAB AB");
    let a = label(&p2, "A");
    let bb = label(&p2, "B");
    let prioritized = Input::new(&p2, &b, &eh)
        .unwrap()
        .step(
            &Step::PriorityMis {
                graph: Subgraph::All,
                order: vec![vec![a, a], vec![bb, bb]],
            },
            1,
            &b,
            &eh,
        )
        .unwrap();
    let state = |rank, role| {
        prioritized
            .names
            .iter()
            .position(|s| s.ends_with(&format!("priorityMIS1[{rank}]={role}")))
            .unwrap()
    };
    assert!(!prioritized
        .edges
        .contains(&annotations::edge(state(0, "P"), state(1, "I"))));
    assert!(prioritized
        .edges
        .contains(&annotations::edge(state(0, "I"), state(1, "P"))));
}
