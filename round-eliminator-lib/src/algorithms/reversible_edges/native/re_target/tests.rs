use super::*;

fn problem(s: &str) -> Problem {
    Problem::from_string(s).unwrap()
}
fn budget(options: &Options) -> Budget<'_> {
    Budget {
        options,
        deadline: Instant::now() + Duration::from_secs(20),
    }
}

#[test]
fn bounded_builder_matches_two_ordinary_unsimplified_speedups() {
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null().with_worker_limit(1);
    for text in [
        "A A\nB B\n\nA A\nA B",
        "A A\nB B\nC C\n\nA B\nA C\nB C",
        "M M M\nP U U\n\nM P\nM U\nU U",
        "B C C\nB B C\nA A C\nB B B\n\nA B\nC C",
    ] {
        let p = problem(text);
        let built = prepare(&p, &b, &mut eh).unwrap();
        let first = p.speedup(&mut eh);
        let second = first.speedup(&mut eh);
        for (actual, expected) in [(&built.first, &first), (&built.second, &second)] {
            assert_eq!(canonical(&actual.active), canonical(&expected.active));
            assert_eq!(canonical(&actual.passive), canonical(&expected.passive));
            assert_eq!(
                actual.mapping_label_oldlabels,
                expected.mapping_label_oldlabels
            );
        }
        verify(&p, &built, &b, &mut eh).unwrap();
    }
}

#[test]
fn re2_mapping_certificate_roundtrips_and_checks_the_decoder() {
    let p = problem("A A\nB B\n\nA A\nA B");
    let b_label = p
        .mapping_label_text
        .iter()
        .find(|(_, s)| s == "B")
        .unwrap()
        .0;
    let options = Options::default();
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let target = prepare(&p, &b, &mut eh).unwrap();
    let added = vec![[b_label, b_label]];
    let q = relaxation(&p, &added).unwrap();
    let c = attempt_target(&p, &q, &added, &[], Some(&target), &b, &mut eh)
        .unwrap()
        .expect("The RE² target is trivially solvable");
    assert!(c.target.is_some());
    let encoded = serde_json::to_string(&c).unwrap();
    let c: Certificate = serde_json::from_str(&encoded).unwrap();
    let relaxed = apply(&p, &c, &mut eh).unwrap();
    assert_eq!(relaxed.active, p.active);
    assert_eq!(relaxed.mapping_label_text, p.mapping_label_text);
    assert!(relaxed.passive.includes(&edge_line(added[0])));
    let request = serde_json::to_string(&crate::serial::Request::ApplyReversibleEdges(
        p.clone(),
        c.clone(),
    ))
    .unwrap();
    let returned = std::sync::Mutex::new(None);
    crate::serial::request_json(&request, |response, primary| {
        if primary {
            match serde_json::from_str::<crate::serial::Response>(&response).unwrap() {
                crate::serial::Response::P(p) => *returned.lock().unwrap() = Some(p),
                crate::serial::Response::E(e) => {
                    panic!("RE² replay API rejected the certificate: {e}")
                }
                _ => {}
            }
        }
    });
    let returned = returned.into_inner().unwrap().unwrap();
    assert_eq!(returned.active, relaxed.active);
    assert_eq!(returned.passive, relaxed.passive);
    assert_eq!(returned.mapping_label_text, relaxed.mapping_label_text);

    let mut bad = c.clone();
    bad.target.as_mut().unwrap().first.mapping_label_oldlabels = None;
    assert!(apply(&p, &bad, &mut eh)
        .unwrap_err()
        .contains("decoder sets"));
    let mut bad = c.clone();
    bad.target.as_mut().unwrap().second.passive.lines.clear();
    assert!(apply(&p, &bad, &mut eh).is_err());
    let mut bad = c.clone();
    bad.mapping[0].output.fill(99999);
    assert!(apply(&p, &bad, &mut eh)
        .unwrap_err()
        .contains("node configuration"));

    // Forge an existential-side relation without changing the dictionaries.
    let mut bad = c.clone();
    let first = &mut bad.target.as_mut().unwrap().first;
    let l = first.labels()[0];
    first.passive.lines = vec![edge_line([l, l])];
    assert!(apply(&p, &bad, &mut eh).is_err());

    // The maximal universal row AB x A is valid, AB x AB is not (BB forbidden).
    let mut bad = c.clone();
    let first = &mut bad.target.as_mut().unwrap().first;
    let both = first
        .mapping_label_oldlabels
        .as_ref()
        .unwrap()
        .iter()
        .find(|(_, set)| set.len() == 2)
        .unwrap()
        .0;
    first.active.lines = vec![edge_line([both, both])];
    assert!(apply(&p, &bad, &mut eh)
        .unwrap_err()
        .contains("universal constraint"));
}

#[test]
fn old_certificates_and_options_remain_compatible() {
    let options: Options = serde_json::from_str(r#"{"seconds":2,"threads":1}"#).unwrap();
    assert!(options.re2);
    let p = problem("A A\nB B\n\nA A\nA B");
    let b = budget(&options);
    let mut eh = EventHandler::null();
    let bb = p
        .mapping_label_text
        .iter()
        .find(|(_, s)| s == "B")
        .unwrap()
        .0;
    let added = vec![[bb, bb]];
    let c = attempt(
        &p,
        &relaxation(&p, &added).unwrap(),
        &added,
        &[],
        &b,
        &mut eh,
    )
    .unwrap()
    .unwrap();
    let encoded = serde_json::to_string(&c).unwrap();
    assert!(!encoded.contains("target"));
    let decoded: Certificate = serde_json::from_str(&encoded).unwrap();
    assert!(decoded.target.is_none());
    apply(&p, &decoded, &mut eh).unwrap();
}

#[test]
fn target_preparation_honors_expired_and_cancelled_budgets() {
    use std::sync::{atomic::AtomicBool, Arc};
    let p = problem("A A\nB B\n\nA A\nA B");
    let options = Options::default();
    let b = Budget {
        options: &options,
        deadline: Instant::now(),
    };
    assert_eq!(
        prepare(&p, &b, &mut EventHandler::null()).unwrap_err(),
        LIMIT
    );
    let b = budget(&options);
    let mut eh = EventHandler::null().with_cancellation(Arc::new(AtomicBool::new(true)));
    assert_eq!(prepare(&p, &b, &mut eh).unwrap_err(), CANCELLED);
}
