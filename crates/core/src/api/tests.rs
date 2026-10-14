use super::*;

#[test]
fn conversion_progress_reaches_its_total_when_an_impossible_part_skips_the_rest_of_a_row() {
    let problem = crate::parse_problem(
        serde_json::from_value(serde_json::json!({
            "kind":"mapped","input":{"active":"i^2","passive":"i^2"},
            "output":{"active":"A B","passive":"B^2"},"mapping":"i -> A"
        }))
        .unwrap(),
    )
    .unwrap();
    let body = serde_json::json!({"type":"apply_re","data":{"problem":problem}}).to_string();
    let mut updates = Vec::new();
    let response = execute_text_with_events(&body, Cancellation::default(), &mut |event| {
        if let ComputationEvent::Progress(progress) = event {
            if progress.phase == Phase::Transforming {
                if let Some(work) = progress.work {
                    updates.push(work);
                }
            }
        }
    });
    assert!(matches!(response, Response::Problem(_)));
    assert_eq!(updates[0].completed, 0);
    assert_eq!(updates[0].total, 2);
    assert_eq!(updates.last().unwrap().completed, 2);
}

#[test]
fn basic_fixed_point_returns_a_new_problem_and_cancellation_in_either_closure_is_distinct() {
    let problem = crate::parse_problem(
        serde_json::from_value(serde_json::json!({
            "kind":"plain","constraints":{"active":"A^2","passive":"A^2"}
        }))
        .unwrap(),
    )
    .unwrap();
    let body =
        serde_json::json!({"type":"basic_fixed_point","data":{"problem":problem}}).to_string();
    let response = execute_text_with_events(&body, Cancellation::default(), &mut |event| {
        if let ComputationEvent::Progress(p) = event {
            assert_ne!(p.phase, Phase::Parsing);
        }
    });
    let Response::Problem(result) = response else {
        panic!()
    };
    assert_eq!(result.graph_class(), problem.graph_class());
    for phase in [
        Phase::ClosingActive,
        Phase::ClosingPassive,
        Phase::Transforming,
    ] {
        let cancellation = Cancellation::default();
        let response = execute_text_with_events(&body, cancellation.clone(), &mut |event| {
            if let ComputationEvent::Progress(p) = event {
                if p.phase == phase {
                    cancellation.cancel();
                }
            }
        });
        assert!(matches!(response, Response::Cancelled));
    }
}

#[test]
fn apply_re_reuses_maxima_streams_complete_cache_and_cancels_without_a_new_problem() {
    let problem = crate::parse_problem(
        serde_json::from_value(serde_json::json!({
            "kind":"plain", "constraints":{"active":"A B", "passive":"A^2\nA B\nB^2"}
        }))
        .unwrap(),
    )
    .unwrap();
    // Omitting the optional cache works for direct API callers as well.
    let body = serde_json::json!({"type":"apply_re", "data":{"problem":problem}}).to_string();
    let mut events = Vec::new();
    let response = execute_text_with_events(&body, Cancellation::default(), &mut |event| {
        events.push(event)
    });
    let Response::Problem(transformed) = response else {
        panic!()
    };
    let maximum = events
        .iter()
        .find_map(|event| match event {
            ComputationEvent::PartialResult(response) => match response.as_ref() {
                Response::Maximization(maximum) => Some(maximum.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("complete source maximum is available for caching");
    assert!(events.iter().all(|event| !matches!(event,
        ComputationEvent::Progress(p) if p.phase == Phase::Parsing)));
    let cached =
        serde_json::json!({"type":"apply_re", "data":{"problem":problem, "maximum":maximum}})
            .to_string();
    let response = execute_text_with_events(&cached, Cancellation::default(), &mut |event| {
        assert!(!matches!(event, ComputationEvent::PartialResult(_)));
        if let ComputationEvent::Progress(p) = event {
            assert_eq!(p.phase, Phase::Transforming);
            if let Some(work) = p.work {
                assert!(work.completed <= work.total);
            }
        }
    });
    let Response::Problem(reused) = response else {
        panic!()
    };
    assert_eq!(reused, transformed);
    for stop in [Phase::Maximizing, Phase::Transforming] {
        let cancellation = Cancellation::default();
        let mut complete_maximum = false;
        let response =
            execute_text_with_events(&body, cancellation.clone(), &mut |event| match event {
                ComputationEvent::Progress(p) if p.phase == stop => cancellation.cancel(),
                ComputationEvent::PartialResult(response) => {
                    assert!(matches!(*response, Response::Maximization(_)));
                    complete_maximum = true;
                }
                _ => {}
            });
        assert!(matches!(response, Response::Cancelled));
        assert_eq!(complete_maximum, stop == Phase::Transforming);
    }
}

#[test]
fn maximization_reuses_snapshots_and_cancels_without_partial_certificates() {
    let problem = crate::parse_problem(
        serde_json::from_value(serde_json::json!({
            "kind":"plain","constraints":{"active":"A B", "passive":"A^3\nA^2 B\nA B^2\nB^3"}
        }))
        .unwrap(),
    )
    .unwrap();
    let body = serde_json::json!({"type":"maximize","data":{"problem":problem}}).to_string();
    let result: serde_json::Value = serde_json::from_str(&execute_json(&body)).unwrap();
    assert_eq!(result["type"], "maximization");
    assert_eq!(result["data"]["kind"], "output_only");
    assert_eq!(
        result["data"]["constraint"]["degrees"][0]["configurations"],
        serde_json::json!([{"parts":[{"labels":[0,1],"multiplicity":3}]}])
    );
    assert!(result["data"].get("problem").is_none());
    for stop in [Phase::Preparing, Phase::Maximizing] {
        let cancellation = Cancellation::default();
        let response = execute_text_with_events(&body, cancellation.clone(), &mut |event| {
            if let ComputationEvent::Progress(p) = event {
                assert_ne!(p.phase, Phase::Parsing);
                if p.phase == stop {
                    cancellation.cancel();
                }
            }
        });
        assert!(matches!(response, Response::Cancelled));
    }
}

#[test]
fn diagram_protocol_reuses_the_problem_and_supports_cancellation_in_every_phase() {
    let parsed = crate::parse_problem(
        serde_json::from_value(serde_json::json!({
            "kind":"plain", "constraints":{"active":"A B", "passive":"A^3\nA^2 B\nA B^2\nB^3"}
        }))
        .unwrap(),
    )
    .unwrap();
    let body = serde_json::json!({"type":"compute_diagram","data":{"problem":parsed,
        "options":{"kind":"output_only","output":"exact","input_output":"exact"}}})
    .to_string();
    let response: serde_json::Value = serde_json::from_str(&execute_json(&body)).unwrap();
    assert_eq!(response["type"], "diagram");
    assert!(response["data"].get("problem").is_none());
    for unrequested in ["output_only", "input_output", "input_aware"] {
        assert!(response["data"].get(unrequested).is_none());
    }
    assert_eq!(
        response["data"]["diagram"]["components"],
        serde_json::json!([[0, 1]])
    );
    for phase in [Phase::Preparing, Phase::Comparing, Phase::Reducing] {
        let cancellation = Cancellation::default();
        let result = execute_text_with_events(&body, cancellation.clone(), &mut |event| {
            if let ComputationEvent::Progress(progress) = event {
                assert_ne!(progress.phase, Phase::Parsing);
                if progress.phase == phase {
                    cancellation.cancel()
                }
            }
        });
        assert!(matches!(result, Response::Cancelled));
    }
}
use crate::computation::Progress;

fn analysis_request(active: &str, passive: &str) -> serde_json::Value {
    let text = serde_json::from_value(serde_json::json!({
        "kind":"plain", "constraints":{"active":active, "passive":passive}
    }))
    .unwrap();
    let problem = crate::parser::parse_problem(text).unwrap();
    serde_json::json!({"type":"check_zero_round", "data":{"problem":problem}})
}

#[test]
fn cancellation_at_each_phase_is_never_reported_as_unsolvability() {
    for mode in ["general", "maximized"] {
        for phase in [
            Phase::Enumerating,
            Phase::Encoding,
            Phase::Solving,
            Phase::Verifying,
        ] {
            let cancellation = Cancellation::default();
            let mut seen = Vec::new();
            let mut report = |progress: Progress| {
                seen.push(progress.phase);
                if progress.phase == phase {
                    cancellation.cancel();
                }
            };
            let mut request = analysis_request("A B", "AB^3");
            request["data"]["passive_mode"] = mode.into();
            let request = request.to_string();
            let response = execute_text(
                &request,
                &mut Computation::new(cancellation.clone(), &mut report),
            );
            assert!(matches!(response, Response::Cancelled));
            assert_eq!(seen.last(), Some(&phase));
        }
    }
    let cancellation = Cancellation::default();
    let mut report = |progress: Progress| {
        if progress.phase == Phase::Parsing {
            cancellation.cancel();
        }
    };
    let response = execute_text(
        r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A","passive":"A"}}}"#,
        &mut Computation::new(cancellation.clone(), &mut report),
    );
    assert!(matches!(response, Response::Cancelled));
}

#[test]
fn analysis_reuses_the_start_result_without_a_parsing_phase() {
    let parsed = execute_json(
        r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"AB^0..2","passive":"AB^2"}}}"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&parsed).unwrap();
    assert_eq!(parsed["type"], "problem");
    let request = serde_json::json!({"type":"check_zero_round", "data":{"problem":parsed["data"]}});
    let mut phases = Vec::new();
    let mut report = |progress: Progress| phases.push(progress.phase);
    let response = execute_text(
        &request.to_string(),
        &mut Computation::new(Cancellation::default(), &mut report),
    );
    let response = serde_json::to_value(response).unwrap();
    assert_eq!(response["type"], "zero_round");
    assert_eq!(response["data"]["status"], "solvable");
    assert!(response["data"].get("problem").is_none());
    assert!(response["data"].get("result").is_none());
    assert_eq!(phases.first(), Some(&Phase::Enumerating));
    assert!(!phases.contains(&Phase::Parsing));
}

#[test]
fn zero_round_protocol_returns_results_for_structured_problems() {
    for (active, passive, status) in [("A B", "AB^3", "solvable"), ("A B", "A B", "unsolvable")] {
        let request = analysis_request(active, passive);
        let result: serde_json::Value =
            serde_json::from_str(&execute_json(&request.to_string())).unwrap();
        assert_eq!(result["type"], "zero_round");
        assert_eq!(result["data"]["status"], status);
        if status == "solvable" {
            assert_eq!(
                result["data"]["strategy"][0]["output"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        } else {
            assert_eq!(
                result,
                serde_json::json!({"type":"zero_round", "data":{"status":"unsolvable"}})
            );
        }
    }
}

#[test]
fn passive_mode_defaults_to_general_and_can_be_selected_explicitly() {
    let request = analysis_request("A B", "AB^3");
    let parsed: Request = serde_json::from_value(request.clone()).unwrap();
    assert!(matches!(
        parsed,
        Request::CheckZeroRound {
            passive_mode: zero_round::PassiveMode::General,
            ..
        }
    ));
    for mode in ["general", "maximized"] {
        let mut request = request.clone();
        request["data"]["passive_mode"] = mode.into();
        let response: serde_json::Value =
            serde_json::from_str(&execute_json(&request.to_string())).unwrap();
        assert_eq!(response["data"]["status"], "solvable");
    }
}

#[test]
fn json_protocol_returns_problem_and_structured_error() {
    let request = r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A B","passive":"AB"}}}"#;
    let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
    assert_eq!(result["type"], "problem");
    assert_eq!(
        result["data"]["output"]["labels"],
        serde_json::json!(["A", "B"])
    );

    let request = r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A","passive":"B^2..1"}}}"#;
    let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
    assert_eq!(result["type"], "error");
    assert_eq!(result["data"]["location"]["field"], "passive");
    assert_eq!(result["data"]["location"]["line"], 1);

    let result: serde_json::Value = serde_json::from_str(&execute_json("not json")).unwrap();
    assert_eq!(result["type"], "error");
    assert!(result["data"]["location"].is_null());
}

fn coloring_request(search: serde_json::Value) -> String {
    let mut request = analysis_request("A^3\nB^3\nC^3", "A BC\nB C");
    request["type"] = "check_coloring".into();
    request["data"]["kind"] = "vertex".into();
    request["data"]["search"] = search;
    request.to_string()
}

#[test]
fn coloring_protocol_streams_bounds_without_parsing_or_echoing_the_problem() {
    let mut events = Vec::new();
    let response = execute_text_with_events(
        &coloring_request(serde_json::json!({"kind":"maximum"})),
        Cancellation::default(),
        &mut |event| events.push(event),
    );
    let response = serde_json::to_value(response).unwrap();
    assert_eq!(response["type"], "coloring");
    assert_eq!(response["data"]["status"], "maximum");
    assert_eq!(response["data"]["colors"], 3);
    assert!(response["data"].get("problem").is_none());
    let mut saw_lower = false;
    let mut saw_upper = false;
    for event in events {
        match event {
            ComputationEvent::Progress(progress) => assert_ne!(progress.phase, Phase::Parsing),
            ComputationEvent::PartialResult(response) => {
                let Response::ColoringBounds(bounds) = *response else {
                    panic!("unexpected partial result")
                };
                saw_lower |= bounds.lower.is_some();
                saw_upper |= bounds.upper.is_some();
                assert_eq!(bounds.witness.is_some(), bounds.lower.is_some());
            }
            ComputationEvent::Response(_) => panic!("the final response is returned separately"),
        }
    }
    assert!(saw_lower && saw_upper);
    for (colors, status) in [(3, "solvable"), (4, "unsolvable")] {
        let response: serde_json::Value = serde_json::from_str(&execute_json(&coloring_request(
            serde_json::json!({"kind":"fixed", "colors":colors}),
        )))
        .unwrap();
        assert_eq!(response["data"]["status"], status);
        assert_eq!(response["data"]["colors"], colors);
    }
}

#[test]
fn coloring_cancellation_keeps_partial_proofs_and_never_implies_unsatisfiability() {
    let cancellation = Cancellation::default();
    let mut lower = None;
    let response = execute_text_with_events(
        &coloring_request(serde_json::json!({"kind":"maximum"})),
        cancellation.clone(),
        &mut |event| {
            if let ComputationEvent::PartialResult(response) = event
                && let Response::ColoringBounds(bounds) = *response
                && bounds.lower.is_some()
            {
                lower = bounds.lower;
                cancellation.cancel();
            }
        },
    );
    assert!(matches!(response, Response::Cancelled));
    assert_eq!(lower, Some(2));

    // This passive constraint is already maximized, so both modes are valid.
    for mode in ["general", "maximized"] {
        for phase in [
            Phase::Enumerating,
            Phase::Encoding,
            Phase::Solving,
            Phase::Verifying,
        ] {
            let mut request = analysis_request("A B", "AB^3");
            request["type"] = "check_coloring".into();
            request["data"]["kind"] = "incidence_edges".into();
            request["data"]["search"] = serde_json::json!({"kind":"fixed", "colors":3});
            request["data"]["passive_mode"] = mode.into();
            let cancellation = Cancellation::default();
            let response = execute_text_with_events(
                &request.to_string(),
                cancellation.clone(),
                &mut |event| {
                    if let ComputationEvent::Progress(progress) = event
                        && progress.phase == phase
                    {
                        cancellation.cancel();
                    }
                },
            );
            assert!(matches!(response, Response::Cancelled), "{mode}: {phase:?}");
        }
    }
}
