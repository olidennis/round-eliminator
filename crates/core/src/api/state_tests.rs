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

/// LLM: Parse a plain active/passive fixture with inferred degrees. Used by algorithm tests
/// LLM: and benchmark preparation.
fn plain(active: &str, passive: &str) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse(serde_json::json!({"kind":"plain","constraints":{"active":active,"passive":passive}}))
}
/// LLM: Deserialize the JSON fixture as text entry and parse it into the canonical model. Used
/// LLM: by this module's tests or benchmark preparation.
fn parse(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    crate::parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}
/// LLM: Evaluate the prepared benchmark or operation fixture and return its result data. Used
/// LLM: by this module's harness rather than the application protocol.
fn run(state: &ProblemState, operation: Operation) -> (ResponsePacket, Vec<Phase>) {
    // LLM: Prepare `phases` for the following computation; it stores the phases for this
    // LLM: stage.
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
    // LLM: Update the current working state before continuing this operation.
    (packet, phases)
}
/// LLM: Construct exact diagram options for the requested relation kind. Used by cache and
/// LLM: relation tests to avoid incidental method differences.
fn exact(kind: DiagramKind) -> DiagramOptions {
    // LLM: Update the current working state before continuing this operation.
    DiagramOptions {
        kind,
        output: OutputDiagramMethod::Exact,
        input_output: InputOutputDiagramMethod::Exact,
    }
}

/// LLM: Regression test: verify that source cache survives re cancellation and is not
/// LLM: inherited by the new problem. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn source_cache_survives_re_cancellation_and_is_not_inherited_by_the_new_problem() {
    // LLM: Prepare `state` for the following computation; it stores the state for this stage.
    let state = ProblemState::from(plain("A B", "A^2\nA B\nB^2"));
    let request = serde_json::to_string(&Request::Compute {
        state: state.clone(),
        operation: Operation::ApplyRe,
    })
    .unwrap();
    let cancellation = Cancellation::default();
    let mut completed = None;
    // LLM: Run the common Rust protocol, forwarding progress and cumulative proof snapshots.
    let packet =
        api::execute_text_with_events(&request, cancellation.clone(), &mut |event| match event {
            ComputationEvent::CacheUpdate(cache) => completed = cache.maximization.clone(),
            ComputationEvent::Progress(p) if p.phase == Phase::Transforming => {
                cancellation.cancel()
            }
            _ => {}
        });
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(packet.response, Response::Cancelled));
    assert_eq!(packet.cache.maximization, completed);
    assert!(completed.is_some());
    let source = ProblemState {
        analyses: packet.cache,
        ..state.clone()
    };
    // LLM: Create the source capacity side of the matching network.
    let source: ProblemState =
        serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    let (packet, phases) = run(&source, Operation::ApplyRe);
    assert!(!phases.contains(&Phase::Maximizing));
    assert_eq!(packet.cache, AnalysisCache::default());
    let Response::Problem(next) = packet.response else {
        panic!("RE problem expected")
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(ProblemState::from(next).analyses, AnalysisCache::default());
    assert_eq!(source.problem, state.problem);
}

/// LLM: Regression test: verify that cached output and joint maxima preserve every diagram
/// LLM: relation. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn cached_output_and_joint_maxima_preserve_every_diagram_relation() {
    // LLM: Check each relevant row independently, preserving its exact degree and condensed
    // LLM: choices.
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
        // LLM: Establish the feasible bound for this local search before exploring or
        // LLM: allocating alternatives.
        let maximum = maximize::compute(&problem).unwrap();
        for kind in [
            DiagramKind::OutputOnly,
            DiagramKind::InputOutput,
            DiagramKind::InputAware,
        ] {
            // LLM: Construct the independent reference result for this fixture.
            let expected = diagram::compute(&problem, exact(kind)).unwrap();
            let actual = diagram::compute_with_maximum(
                &problem,
                exact(kind),
                Some(&maximum),
                &mut Computation::new(Cancellation::default(), &mut |_| {}),
            )
            .unwrap();
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(actual, expected);
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if matches!(maximum, maximize::MaximizationResult::InputOutput(_)) {
            // LLM: Prepare `options` for the following computation; it stores the options for
            // LLM: this stage.
            let options = DiagramOptions {
                output: OutputDiagramMethod::Partial,
                ..exact(DiagramKind::OutputOnly)
            };
            // LLM: Evaluate the implementation under test on the same fixture.
            let actual = diagram::compute_with_maximum(
                &problem,
                options,
                Some(&maximum),
                &mut Computation::new(Cancellation::default(), &mut |_| {}),
            )
            .unwrap();
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(
                actual,
                diagram::compute(&problem, options).unwrap(),
                "joint maximum cannot certify output-only arrows"
            );
        }
    }
}

/// LLM: Regression test: verify that cached output maximum upgrades partial without reusing an
/// LLM: incomplete diagram. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn cached_output_maximum_upgrades_partial_without_reusing_an_incomplete_diagram() {
    // LLM: Prepare `state` for the following computation; it stores the state for this stage.
    let mut state = ProblemState::from(plain("A B", "A^3\nA^2 B\nA B^2\nB^3"));
    let options = DiagramOptions {
        output: OutputDiagramMethod::Partial,
        ..exact(DiagramKind::OutputOnly)
    };
    let (partial, _) = run(&state, Operation::ComputeDiagram { options });
    // LLM: Prepare `Response` for the following computation; it stores the response for this
    // LLM: stage.
    let Response::Diagram(partial_result) = &partial.response else {
        panic!()
    };
    assert!(!partial_result.diagram.complete);
    state.analyses.diagrams = partial.cache.diagrams;
    state.analyses.maximization = Some(maximize::compute(&state.problem).unwrap());
    let (upgraded, phases) = run(&state, Operation::ComputeDiagram { options });
    assert!(phases.contains(&Phase::Comparing));
    // LLM: Prepare `Response` for the following computation; it stores the response for this
    // LLM: stage.
    let Response::Diagram(upgraded_result) = &upgraded.response else {
        panic!()
    };
    assert!(upgraded_result.diagram.complete);
    assert_eq!(upgraded_result.options.output, OutputDiagramMethod::Exact);
    assert_eq!(upgraded_result.diagram.components, vec![vec![0, 1]]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_ne!(
        upgraded_result.diagram.successors,
        partial_result.diagram.successors
    );
    state.analyses.diagrams.extend(upgraded.cache.diagrams);
    // LLM: Update the current working state before continuing this operation.
    let (reused, phases) = run(
        &state,
        Operation::ComputeDiagram {
            options: exact(DiagramKind::OutputOnly),
        },
    );
    assert!(phases.is_empty());
    // LLM: Prepare `Response` for the following computation; it stores the response for this
    // LLM: stage.
    let Response::Diagram(reused_result) = reused.response else {
        panic!()
    };
    assert_eq!(&reused_result, upgraded_result);
}

/// LLM: Regression test: verify that cached maximum removes exact diagram context work and
/// LLM: complete diagrams are reused. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn cached_maximum_removes_exact_diagram_context_work_and_complete_diagrams_are_reused() {
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = plain("A B", "A^3\nA^2 B\nA B^2\nB^3");
    let mut state = ProblemState::from(problem);
    let options = exact(DiagramKind::OutputOnly);
    let (uncached, slow) = run(&state, Operation::ComputeDiagram { options });
    state.analyses.maximization = Some(maximize::compute(&state.problem).unwrap());
    let (cached, fast) = run(&state, Operation::ComputeDiagram { options });
    // LLM: Update the current working state before continuing this operation.
    let (Response::Diagram(a), Response::Diagram(b)) = (uncached.response, cached.response) else {
        panic!()
    };
    assert_eq!(a, b);
    assert!(
        fast.iter().filter(|p| **p == Phase::Preparing).count()
            < slow.iter().filter(|p| **p == Phase::Preparing).count()
    );
    // LLM: Update the current working state before continuing this operation.
    state.analyses.diagrams = cached.cache.diagrams;
    let (reused, phases) = run(&state, Operation::ComputeDiagram { options });
    assert!(phases.is_empty());
    assert_eq!(reused.cache, AnalysisCache::default());
    assert!(matches!(reused.response, Response::Diagram(_)));
}

/// LLM: Regression test: verify that cached solving agrees with original constraints and
/// LLM: preserves input witnesses. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn cached_solving_agrees_with_original_constraints_and_preserves_input_witnesses() {
    // LLM: Check the returned local rules independently of the solver encoding.
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
        // LLM: Establish the feasible bound for this local search before exploring or
        // LLM: allocating alternatives.
        let maximum = maximize::compute(&problem).unwrap();
        let expected = zero_round::check(&problem, PassiveMode::General).unwrap();
        // LLM: Evaluate the implementation under test on the same fixture.
        let actual = zero_round::check_with_maximum(
            &problem,
            PassiveMode::General,
            Some(&maximum),
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        // LLM: Compare the implementation result with the independently prepared reference for
        // LLM: this fixture.
        assert_eq!(
            matches!(actual, ZeroRoundResult::Solvable { .. }),
            matches!(expected, ZeroRoundResult::Solvable { .. })
        );
        if let ZeroRoundResult::Solvable { strategy } = actual {
            assert!(zero_round::verify_strategy(&problem, &strategy));
        }
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for colors in [3, 4] {
            // LLM: Construct the independent reference result for this fixture.
            let expected = coloring::check(
                &problem,
                ColoringKind::Vertex,
                PaletteSearch::Fixed { colors },
                PassiveMode::General,
            )
            .unwrap();
            // LLM: Evaluate the implementation under test on the same fixture.
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
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(
                matches!(actual, ColoringResult::Solvable { .. }),
                matches!(expected, ColoringResult::Solvable { .. })
            );
            // LLM: Validate the returned color behavior tables with the independent
            // LLM: certificate checker.
            if let ColoringResult::Solvable { witness, .. } = actual {
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
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

/// LLM: Regression test: verify that completed palette proofs restrict both color indices and
/// LLM: do not cross kinds. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn completed_palette_proofs_restrict_both_color_indices_and_do_not_cross_kinds() {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for (problem, kind) in [
        (plain("A^3\nB^3\nC^3", "A BC\nB C"), ColoringKind::Vertex),
        (plain("O I", "O I"), ColoringKind::DistanceTwo),
        (plain("OI^2", "O I"), ColoringKind::DistanceTwo),
    ] {
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result =
            coloring::check(&problem, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap();
        let mut state = ProblemState::from(problem);
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Maximum,
            result,
        });
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for colors in [3, 4] {
            // LLM: Update the current working state before continuing this operation.
            let (packet, phases) = run(
                &state,
                Operation::CheckColoring {
                    kind,
                    search: PaletteSearch::Fixed { colors },
                    passive_mode: PassiveMode::General,
                },
            );
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(!phases.contains(&Phase::Solving));
            match packet.response {
                Response::Coloring(ColoringResult::Solvable { witness, .. }) => {
                    assert!(verify_witness(&state.problem, kind, &witness, false))
                }
                Response::Coloring(ColoringResult::Unsolvable { .. }) => {}
                other => panic!("{other:?}"),
            }
        }
        // LLM: Update the current working state before continuing this operation.
        let (packet, _) = run(
            &state,
            Operation::CheckColoring {
                kind,
                search: PaletteSearch::Fixed { colors: 1 },
                passive_mode: PassiveMode::General,
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(matches!(packet.response, Response::Error(_)));
    }
    // LLM: Prepare `state` for the following computation; it stores the state for this stage.
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
    // LLM: An unrelated kind must never consume the deliberately unusable witness.

    let (packet, phases) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::IncidenceEdges,
            search: PaletteSearch::Fixed { colors: 3 },
            passive_mode: PassiveMode::General,
        },
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(phases.contains(&Phase::Solving));
    assert!(matches!(packet.response, Response::Coloring(_)));
}

/// LLM: Regression test: verify that zero round preflight is cached and errors never create
/// LLM: analysis entries. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn zero_round_preflight_is_cached_and_errors_never_create_analysis_entries() {
    // LLM: Prepare `state` for the following computation; it stores the state for this stage.
    let state = ProblemState::from(plain("A^2\nB^2", "A B"));
    let (packet, _) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Maximum,
            passive_mode: PassiveMode::General,
        },
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(packet.cache.zero_round.is_some());
    let state = ProblemState {
        analyses: packet.cache,
        ..state
    };
    // LLM: Update the current working state before continuing this operation.
    let (packet, phases) = run(
        &state,
        Operation::CheckZeroRound {
            passive_mode: PassiveMode::General,
        },
    );
    assert!(phases.is_empty());
    assert_eq!(packet.cache, AnalysisCache::default());
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(packet.response, Response::ZeroRound(_)));
    let (packet, _) = run(
        &state,
        Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Fixed { colors: 0 },
            passive_mode: PassiveMode::General,
        },
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(packet.cache, AnalysisCache::default());
    assert!(matches!(packet.response, Response::Error(_)));
}

/// LLM: Regression test: verify that a cached zero round strategy certifies every coloring
/// LLM: kind without sat. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn a_cached_zero_round_strategy_certifies_every_coloring_kind_without_sat() {
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"a b","passive":"ab^2"},
        "output":{"active":"A B","passive":"AB^2"},"mapping":"a -> A\nb -> B"}));
    let mut state = ProblemState::from(problem);
    state.analyses.zero_round =
        Some(zero_round::check(&state.problem, PassiveMode::General).unwrap());
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(
        state.analyses.zero_round,
        Some(ZeroRoundResult::Solvable { .. })
    ));
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for kind in [
        ColoringKind::Vertex,
        ColoringKind::GraphEdges,
        ColoringKind::IncidenceEdges,
        ColoringKind::DistanceTwo,
    ] {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for search in [PaletteSearch::Maximum, PaletteSearch::Fixed { colors: 3 }] {
            // LLM: Update the current working state before continuing this operation.
            let (packet, phases) = run(
                &state,
                Operation::CheckColoring {
                    kind,
                    search,
                    passive_mode: PassiveMode::General,
                },
            );
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
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

/// LLM: Regression test: verify that maximum search reuses fixed palette bounds and negative
/// LLM: proofs. Used by the test suite to protect this behavior while implementations change.
#[test]
fn maximum_search_reuses_fixed_palette_bounds_and_negative_proofs() {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for (problem, kind) in [
        (plain("A^3\nB^3\nC^3", "A BC\nB C"), ColoringKind::Vertex),
        (plain("O I", "O I"), ColoringKind::DistanceTwo),
    ] {
        // LLM: Prepare `state` for the following computation; it stores the state for this
        // LLM: stage.
        let mut state = ProblemState::from(problem);
        let at_three = coloring::check(
            &state.problem,
            kind,
            PaletteSearch::Fixed { colors: 3 },
            PassiveMode::General,
        )
        .unwrap();
        // LLM: Retain the newly constructed choices in the result for this stage.
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Fixed { colors: 3 },
            result: at_three,
        });
        // LLM: Prepare `request` for the following computation; it stores the request for this
        // LLM: stage.
        let request = serde_json::to_string(&Request::Compute {
            state: state.clone(),
            operation: Operation::CheckColoring {
                kind,
                search: PaletteSearch::Maximum,
                passive_mode: PassiveMode::General,
            },
        })
        .unwrap();
        // LLM: Prepare `palettes` for the following computation; it stores the palettes for
        // LLM: this stage.
        let mut palettes = Vec::new();
        let packet =
            api::execute_text_with_events(&request, Cancellation::default(), &mut |event| {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if let ComputationEvent::PartialResult(result) = event
                    && let Response::ColoringBounds(bounds) = result.as_ref()
                    && let Some(colors) = bounds.testing
                {
                    palettes.push(colors);
                }
            });
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            palettes,
            [4],
            "already proved palettes must not be retested"
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(matches!(
            packet.response,
            Response::Coloring(ColoringResult::Maximum { colors: 3, .. })
        ));
        // LLM: Retain the newly constructed choices in the result for this stage.
        state.analyses.coloring.push(CachedColoring {
            kind,
            search: PaletteSearch::Fixed { colors: 4 },
            result: ColoringResult::Unsolvable { colors: 4 },
        });
        // LLM: Update the current working state before continuing this operation.
        let (packet, phases) = run(
            &state,
            Operation::CheckColoring {
                kind,
                search: PaletteSearch::Maximum,
                passive_mode: PassiveMode::General,
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            phases.is_empty(),
            "adjacent known SAT/UNSAT palettes already determine the maximum"
        );
        let Response::Coloring(ColoringResult::Maximum { colors, witness }) = packet.response
        else {
            panic!()
        };
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(colors, 3);
        assert!(verify_witness(&state.problem, kind, &witness, false));
        let (packet, phases) = run(
            &state,
            Operation::CheckZeroRound {
                passive_mode: PassiveMode::General,
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(phases.is_empty());
        assert!(matches!(
            packet.response,
            Response::ZeroRound(ZeroRoundResult::Unsolvable)
        ));
    }
}

/// LLM: Regression test: verify that a known upper bound is not mistaken for a completed
/// LLM: maximum. Used by the test suite to protect this behavior while implementations change.
#[test]
fn a_known_upper_bound_is_not_mistaken_for_a_completed_maximum() {
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let mut state = ProblemState::from(problem);
    state.analyses.coloring.push(CachedColoring {
        kind: ColoringKind::Vertex,
        search: PaletteSearch::Fixed { colors: 4 },
        result: ColoringResult::Unsolvable { colors: 4 },
    });
    // LLM: Prepare `request` for the following computation; it stores the request for this
    // LLM: stage.
    let request = serde_json::to_string(&Request::Compute {
        state,
        operation: Operation::CheckColoring {
            kind: ColoringKind::Vertex,
            search: PaletteSearch::Maximum,
            passive_mode: PassiveMode::General,
        },
    })
    .unwrap();
    // LLM: Prepare `palettes` for the following computation; it stores the palettes for this
    // LLM: stage.
    let mut palettes = Vec::new();
    let packet = api::execute_text_with_events(&request, Cancellation::default(), &mut |event| {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if let ComputationEvent::PartialResult(result) = event
            && let Response::ColoringBounds(bounds) = result.as_ref()
            && let Some(colors) = bounds.testing
        {
            palettes.push(colors);
        }
    });
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(palettes, [2, 3]);
    assert!(matches!(
        packet.response,
        Response::Coloring(ColoringResult::Maximum { colors: 3, .. })
    ));
}

/// LLM: Regression test: verify that cancelled palette searches resume from proved bounds and
/// LLM: keep the witness. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn cancelled_palette_searches_resume_from_proved_bounds_and_keep_the_witness() {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for (problem, kind) in [
        (plain("A^3\nB^3\nC^3", "A BC\nB C"), ColoringKind::Vertex),
        (plain("O I", "O I"), ColoringKind::DistanceTwo),
    ] {
        // LLM: Prepare `state` for the following computation; it stores the state for this
        // LLM: stage.
        let state = ProblemState::from(problem);
        let request = Request::Compute {
            state: state.clone(),
            operation: Operation::CheckColoring {
                kind,
                search: PaletteSearch::Maximum,
                passive_mode: PassiveMode::General,
            },
        };
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let packet = api::execute_text_with_events(
            &serde_json::to_string(&request).unwrap(),
            cancellation.clone(),
            &mut |event| {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if let ComputationEvent::PartialResult(response) = event
                    && let Response::ColoringBounds(bounds) = response.as_ref()
                    && bounds.testing == Some(4)
                {
                    cancellation.cancel();
                }
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(matches!(packet.response, Response::Cancelled));
        let proof = packet
            .cache
            .coloring_bounds
            .iter()
            .find(|entry| entry.kind == kind)
            .unwrap();
        assert_eq!(proof.bounds.lower, Some(3));
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(proof.bounds.upper, None);
        assert_eq!(proof.bounds.testing, None);
        assert!(coloring::verify_witness(
            &state.problem,
            kind,
            proof.bounds.witness.as_ref().unwrap(),
            false
        ));
        // LLM: Prepare `resumed` for the following computation; it stores the resumed for this
        // LLM: stage.
        let resumed = ProblemState {
            analyses: packet.cache,
            ..state.clone()
        };
        let mut tested = Vec::new();
        // LLM: Run the common Rust protocol, forwarding progress and cumulative proof
        // LLM: snapshots.
        let packet = api::execute_text_with_events(
            &serde_json::to_string(&Request::Compute {
                state: resumed.clone(),
                operation: Operation::CheckColoring {
                    kind,
                    search: PaletteSearch::Maximum,
                    passive_mode: PassiveMode::General,
                },
            })
            .unwrap(),
            Cancellation::default(),
            &mut |event| {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if let ComputationEvent::PartialResult(response) = event
                    && let Response::ColoringBounds(bounds) = response.as_ref()
                    && let Some(colors) = bounds.testing
                {
                    tested.push(colors);
                }
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(tested, vec![4]);
        assert!(matches!(
            packet.response,
            Response::Coloring(ColoringResult::Maximum { colors: 3, .. })
        ));
        // LLM: Update the current working state before continuing this operation.
        let (fixed, phases) = run(
            &resumed,
            Operation::CheckColoring {
                kind,
                search: PaletteSearch::Fixed { colors: 3 },
                passive_mode: PassiveMode::General,
            },
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(matches!(
            fixed.response,
            Response::Coloring(ColoringResult::Solvable { colors: 3, .. })
        ));
        assert!(!phases.contains(&Phase::Solving));
    }
}
