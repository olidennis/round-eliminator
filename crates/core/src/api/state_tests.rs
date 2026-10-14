use crate::{
    algorithms::{
        diagram::{self, *},
        maximize,
        solvability::{
            coloring::{self, *},
            passive::PassiveMode,
            zero_round::{self, ZeroRoundResult},
        },
    },
    api::{self, *},
    computation::{Cancellation, Computation, Phase},
    model::Problem,
};

fn plain(active: &str, passive: &str) -> Problem {
    parse(serde_json::json!({"kind":"plain","constraints":{"active":active,"passive":passive}}))
}
fn parse(value: serde_json::Value) -> Problem {
    crate::parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}
fn run(state: &ProblemState, operation: Operation) -> (ResponsePacket, Vec<Phase>) {
    let mut phases = Vec::new();
    let packet = api::execute_with(
        Request::Compute {
            state: state.clone(),
            operation,
        },
        &mut Computation::new(Cancellation::default(), &mut |progress| {
            phases.push(progress.phase)
        }),
    );
    (packet, phases)
}
fn exact(kind: DiagramKind) -> DiagramOptions {
    DiagramOptions {
        kind,
        output: OutputDiagramMethod::Exact,
        input_output: InputOutputDiagramMethod::Exact,
    }
}

#[test]
fn source_cache_survives_re_cancellation_and_is_not_inherited_by_the_new_problem() {
    let state = ProblemState::from(plain("A B", "A^2\nA B\nB^2"));
    let request = serde_json::to_string(&Request::Compute {
        state: state.clone(),
        operation: Operation::ApplyRe,
    })
    .unwrap();
    let cancellation = Cancellation::default();
    let mut completed = None;
    let packet =
        api::execute_text_with_events(&request, cancellation.clone(), &mut |event| match event {
            ComputationEvent::CacheUpdate(cache) => completed = cache.maximization.clone(),
            ComputationEvent::Progress(p) if p.phase == Phase::Transforming => {
                cancellation.cancel()
            }
            _ => {}
        });
    assert!(matches!(packet.response, Response::Cancelled));
    assert_eq!(packet.cache.maximization, completed);
    assert!(completed.is_some());
    let source = ProblemState {
        analyses: packet.cache,
        ..state.clone()
    };
    let source: ProblemState =
        serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    let (packet, phases) = run(&source, Operation::ApplyRe);
    assert!(!phases.contains(&Phase::Maximizing));
    assert_eq!(packet.cache, AnalysisCache::default());
    let Response::Problem(next) = packet.response else {
        panic!("RE problem expected")
    };
    assert_eq!(ProblemState::from(next).analyses, AnalysisCache::default());
    assert_eq!(source.problem, state.problem);
}

#[test]
fn cached_output_and_joint_maxima_preserve_every_diagram_relation() {
    for problem in [
        plain("A B", "A^2\nA B\nB^2"),
        parse(
            serde_json::json!({"kind":"independent", "input":{"active":"i^2","passive":"i^2"},
            "output":{"active":"A B","passive":"A^2\nA B\nB^2"}}),
        ),
        parse(
            serde_json::json!({"kind":"mapped", "input":{"active":"i^3\nj^3","passive":"ij^2"},
            "output":{"active":"A^3\nB^3\nC^3","passive":"A C\nBC^2"}, "mapping":"i -> AB\nj -> C"}),
        ),
        parse(
            serde_json::json!({"kind":"paired", "constraints":{"active":"(i,A)(i,B)^2\n(j,C)^2", "passive":"(i,A) (j,C)\n(i,B)(j,C)^2"}}),
        ),
    ] {
        let maximum = maximize::compute(&problem).unwrap();
        for kind in [
            DiagramKind::OutputOnly,
            DiagramKind::InputOutput,
            DiagramKind::InputAware,
        ] {
            let expected = diagram::compute(&problem, exact(kind)).unwrap();
            let actual = diagram::compute_with_maximum(
                &problem,
                exact(kind),
                Some(&maximum),
                &mut Computation::new(Cancellation::default(), &mut |_| {}),
            )
            .unwrap();
            assert_eq!(actual, expected);
        }
        if matches!(maximum, maximize::MaximizationResult::InputOutput(_)) {
            let options = DiagramOptions {
                output: OutputDiagramMethod::Partial,
                ..exact(DiagramKind::OutputOnly)
            };
            let actual = diagram::compute_with_maximum(
                &problem,
                options,
                Some(&maximum),
                &mut Computation::new(Cancellation::default(), &mut |_| {}),
            )
            .unwrap();
            assert_eq!(
                actual,
                diagram::compute(&problem, options).unwrap(),
                "joint maximum cannot certify output-only arrows"
            );
        }
    }
}

#[test]
fn cached_maximum_removes_exact_diagram_context_work_and_complete_diagrams_are_reused() {
    let problem = plain("A B", "A^3\nA^2 B\nA B^2\nB^3");
    let mut state = ProblemState::from(problem);
    let options = exact(DiagramKind::OutputOnly);
    let (uncached, slow) = run(&state, Operation::ComputeDiagram { options });
    state.analyses.maximization = Some(maximize::compute(&state.problem).unwrap());
    let (cached, fast) = run(&state, Operation::ComputeDiagram { options });
    let (Response::Diagram(a), Response::Diagram(b)) = (uncached.response, cached.response) else {
        panic!()
    };
    assert_eq!(a, b);
    assert!(
        fast.iter().filter(|p| **p == Phase::Preparing).count()
            < slow.iter().filter(|p| **p == Phase::Preparing).count()
    );
    state.analyses.diagrams = cached.cache.diagrams;
    let (reused, phases) = run(&state, Operation::ComputeDiagram { options });
    assert!(phases.is_empty());
    assert_eq!(reused.cache, AnalysisCache::default());
    assert!(matches!(reused.response, Response::Diagram(_)));
}

#[test]
fn cached_solving_agrees_with_original_constraints_and_preserves_input_witnesses() {
    for problem in [
        plain("A B", "A^3\nB^3"),
        parse(
            serde_json::json!({"kind":"mapped","input":{"active":"a b^2","passive":"a b^2"},
            "output":{"active":"AB^3","passive":"A^3\nA^2 B\nB^3"},"mapping":"a -> AB\nb -> AB"}),
        ),
        parse(
            serde_json::json!({"kind":"paired","constraints":{"active":"(a,A)(a,B)^3","passive":"(a,A)^3\n(a,A)^2 (a,B)\n(a,B)^3"}}),
        ),
    ] {
        let maximum = maximize::compute(&problem).unwrap();
        let expected = zero_round::check(&problem, PassiveMode::General).unwrap();
        let actual = zero_round::check_with_maximum(
            &problem,
            PassiveMode::General,
            Some(&maximum),
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        assert_eq!(
            matches!(actual, ZeroRoundResult::Solvable { .. }),
            matches!(expected, ZeroRoundResult::Solvable { .. })
        );
        if let ZeroRoundResult::Solvable { strategy } = actual {
            assert!(zero_round::verify_strategy(&problem, &strategy));
        }
        for colors in [3, 4] {
            let expected = coloring::check(
                &problem,
                ColoringKind::Vertex,
                PaletteSearch::Fixed { colors },
                PassiveMode::General,
            )
            .unwrap();
            let actual = coloring::check_with_cached(
                &problem,
                ColoringKind::Vertex,
                PaletteSearch::Fixed { colors },
                PassiveMode::General,
                coloring::ColoringCache {
                    passive_maximum: Some(&maximum),
                    ..Default::default()
                },
                &mut Computation::new(Cancellation::default(), &mut |_| {}),
                &mut |_| {},
            )
            .unwrap();
            assert_eq!(
                matches!(actual, ColoringResult::Solvable { .. }),
                matches!(expected, ColoringResult::Solvable { .. })
            );
            if let ColoringResult::Solvable { witness, .. } = actual {
                assert!(verify_witness(
                    &problem,
                    ColoringKind::Vertex,
                    &witness,
                    false
                ));
            }
        }
    }
}

#[test]
fn completed_palette_proofs_restrict_both_color_indices_and_do_not_cross_kinds() {
    for (problem, kind) in [
        (plain("A^3\nB^3\nC^3", "A BC\nB C"), ColoringKind::Vertex),
        (plain("O I", "O I"), ColoringKind::DistanceTwo),
        (plain("OI^2", "O I"), ColoringKind::DistanceTwo),
    ] {
        let result =
            coloring::check(&problem, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap();
        let mut state = ProblemState::from(problem);
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Maximum,
            result,
        });
        for colors in [3, 4] {
            let (packet, phases) = run(
                &state,
                Operation::CheckColoring {
                    kind,
                    search: PaletteSearch::Fixed { colors },
                    passive_mode: PassiveMode::General,
                },
            );
            assert!(!phases.contains(&Phase::Solving));
            match packet.response {
                Response::Coloring(ColoringResult::Solvable { witness, .. }) => {
                    assert!(verify_witness(&state.problem, kind, &witness, false))
                }
                Response::Coloring(ColoringResult::Unsolvable { .. }) => {}
                other => panic!("{other:?}"),
            }
        }
        let (packet, _) = run(
            &state,
            Operation::CheckColoring {
                kind,
                search: PaletteSearch::Fixed { colors: 1 },
                passive_mode: PassiveMode::General,
            },
        );
        assert!(matches!(packet.response, Response::Error(_)));
    }
    let mut state = ProblemState::from(plain("A^3\nB^3\nC^3", "A BC\nB C"));
    state.analyses.coloring.push(CachedColoring {
        kind: ColoringKind::Vertex,
        search: PaletteSearch::Maximum,
        result: ColoringResult::Maximum {
            colors: 3,
            witness: ColoringWitness::LocalColors {
                behaviors: Vec::new(),
            },
        },
    });
    // An unrelated kind must never consume the deliberately unusable witness.
    let (packet, phases) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::IncidenceEdges,
            search: PaletteSearch::Fixed { colors: 3 },
            passive_mode: PassiveMode::General,
        },
    );
    assert!(phases.contains(&Phase::Solving));
    assert!(matches!(packet.response, Response::Coloring(_)));
}

#[test]
fn zero_round_preflight_is_cached_and_errors_never_create_analysis_entries() {
    let state = ProblemState::from(plain("A^2\nB^2", "A B"));
    let (packet, _) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Maximum,
            passive_mode: PassiveMode::General,
        },
    );
    assert!(packet.cache.zero_round.is_some());
    let state = ProblemState {
        analyses: packet.cache,
        ..state
    };
    let (packet, phases) = run(
        &state,
        Operation::CheckZeroRound {
            passive_mode: PassiveMode::General,
        },
    );
    assert!(phases.is_empty());
    assert_eq!(packet.cache, AnalysisCache::default());
    assert!(matches!(packet.response, Response::ZeroRound(_)));
    let (packet, _) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Fixed { colors: 0 },
            passive_mode: PassiveMode::General,
        },
    );
    assert_eq!(packet.cache, AnalysisCache::default());
    assert!(matches!(packet.response, Response::Error(_)));
}

#[test]
fn a_cached_zero_round_strategy_certifies_every_coloring_kind_without_sat() {
    let problem = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"a b","passive":"ab^2"},
        "output":{"active":"A B","passive":"AB^2"},"mapping":"a -> A\nb -> B"}));
    let mut state = ProblemState::from(problem);
    state.analyses.zero_round =
        Some(zero_round::check(&state.problem, PassiveMode::General).unwrap());
    assert!(matches!(
        state.analyses.zero_round,
        Some(ZeroRoundResult::Solvable { .. })
    ));
    for kind in [
        ColoringKind::Vertex,
        ColoringKind::GraphEdges,
        ColoringKind::IncidenceEdges,
        ColoringKind::DistanceTwo,
    ] {
        for search in [PaletteSearch::Maximum, PaletteSearch::Fixed { colors: 3 }] {
            let (packet, phases) = run(
                &state,
                Operation::CheckColoring {
                    kind,
                    search,
                    passive_mode: PassiveMode::General,
                },
            );
            assert!(!phases.contains(&Phase::Solving));
            let (witness, repeat) = match packet.response {
                Response::Coloring(ColoringResult::Unbounded { witness, .. }) => (witness, true),
                Response::Coloring(ColoringResult::Solvable { witness, .. }) => (witness, false),
                other => panic!("{other:?}"),
            };
            assert!(verify_witness(&state.problem, kind, &witness, repeat));
        }
    }
}

#[test]
fn maximum_search_reuses_fixed_palette_bounds_and_negative_proofs() {
    for (problem, kind) in [
        (plain("A^3\nB^3\nC^3", "A BC\nB C"), ColoringKind::Vertex),
        (plain("O I", "O I"), ColoringKind::DistanceTwo),
    ] {
        let mut state = ProblemState::from(problem);
        let at_three = coloring::check(
            &state.problem,
            kind,
            PaletteSearch::Fixed { colors: 3 },
            PassiveMode::General,
        )
        .unwrap();
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Fixed { colors: 3 },
            result: at_three,
        });
        let request = serde_json::to_string(&Request::Compute {
            state: state.clone(),
            operation: Operation::CheckColoring {
                kind,
                search: PaletteSearch::Maximum,
                passive_mode: PassiveMode::General,
            },
        })
        .unwrap();
        let mut palettes = Vec::new();
        let packet =
            api::execute_text_with_events(&request, Cancellation::default(), &mut |event| {
                if let ComputationEvent::PartialResult(result) = event
                    && let Response::ColoringBounds(bounds) = result.as_ref()
                    && let Some(colors) = bounds.testing
                {
                    palettes.push(colors);
                }
            });
        assert_eq!(
            palettes,
            [4],
            "already proved palettes must not be retested"
        );
        assert!(matches!(
            packet.response,
            Response::Coloring(ColoringResult::Maximum { colors: 3, .. })
        ));
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Fixed { colors: 4 },
            result: ColoringResult::Unsolvable { colors: 4 },
        });
        let (packet, phases) = run(
            &state,
            Operation::CheckColoring {
                kind,
                search: PaletteSearch::Maximum,
                passive_mode: PassiveMode::General,
            },
        );
        assert!(
            phases.is_empty(),
            "adjacent known SAT/UNSAT palettes already determine the maximum"
        );
        let Response::Coloring(ColoringResult::Maximum { colors, witness }) = packet.response
        else {
            panic!()
        };
        assert_eq!(colors, 3);
        assert!(verify_witness(&state.problem, kind, &witness, false));
        let (packet, phases) = run(
            &state,
            Operation::CheckZeroRound {
                passive_mode: PassiveMode::General,
            },
        );
        assert!(phases.is_empty());
        assert!(matches!(
            packet.response,
            Response::ZeroRound(ZeroRoundResult::Unsolvable)
        ));
    }
}

#[test]
fn a_known_upper_bound_is_not_mistaken_for_a_completed_maximum() {
    let problem = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let mut state = ProblemState::from(problem);
    state.analyses.coloring.push(CachedColoring {
        kind: ColoringKind::Vertex,
        search: PaletteSearch::Fixed { colors: 4 },
        result: ColoringResult::Unsolvable { colors: 4 },
    });
    let request = serde_json::to_string(&Request::Compute {
        state,
        operation: Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Maximum,
            passive_mode: PassiveMode::General,
        },
    })
    .unwrap();
    let mut palettes = Vec::new();
    let packet = api::execute_text_with_events(&request, Cancellation::default(), &mut |event| {
        if let ComputationEvent::PartialResult(result) = event
            && let Response::ColoringBounds(bounds) = result.as_ref()
            && let Some(colors) = bounds.testing
        {
            palettes.push(colors);
        }
    });
    assert_eq!(palettes, [2, 3]);
    assert!(matches!(
        packet.response,
        Response::Coloring(ColoringResult::Maximum { colors: 3, .. })
    ));
}
