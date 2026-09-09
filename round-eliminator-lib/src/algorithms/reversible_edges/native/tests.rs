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
