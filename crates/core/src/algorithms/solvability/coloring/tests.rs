use super::*;
use crate::{
    algorithms::{configurations::concrete_multisets, solvability::zero_round::ZeroRoundResult},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, Input, LabelId, OutputMapping, Part,
    },
    parser::parse_problem,
};

/// LLM: Parse a JSON problem definition for the independent algorithm fixtures. Used by tests
/// LLM: to exercise the same entry conversion as the UI.
pub(super) fn problem(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

/// LLM: Parse a plain active/passive fixture with inferred degrees. Used by algorithm tests
/// LLM: and benchmark preparation.
pub(super) fn plain(active: &str, passive: &str) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    problem(serde_json::json!({"kind":"plain", "constraints":{"active":active,"passive":passive}}))
}

/// LLM: Run one fixed-palette coloring decision and verify any returned witness. Used by
/// LLM: coloring regression tests to compare expected solvability.
pub(super) fn fixed(problem: &Problem, kind: ColoringKind, colors: u32) -> bool {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    match check(
        problem,
        kind,
        PaletteSearch::Fixed { colors },
        PassiveMode::General,
    )
    .unwrap()
    {
        ColoringResult::Solvable { witness, .. } => {
            assert!(verify_witness(problem, kind, &witness, false));
            true
        }
        ColoringResult::Unsolvable { .. } => false,
        result => panic!("unexpected {result:?}"),
    }
}

// LLM: Independent color enumeration, deliberately including all permutations.
// LLM: Construct a small explicit colored input promise and use the separately
// LLM: tested local-rule solver as a reference for the compact membership encoding.
/// LLM: Materialize the auxiliary coloring as ordinary input constraints for tiny fixtures.
/// LLM: Used to verify the compact coloring encoding against an independent explicit problem.
fn explicit_coloring(problem: &Problem, kind: ColoringKind, colors: u32) -> Problem {
    /// LLM: Enumerate small color placements for an explicit colored-input oracle. Used only
    /// LLM: in coloring equivalence tests.
    fn palettes(colors: u32, degree: u32) -> Vec<Vec<u32>> {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if degree == 0 {
            return vec![vec![]];
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        palettes(colors, degree - 1)
            .into_iter()
            .flat_map(|prefix| {
                (0..colors).map(move |color| {
                    let mut tuple = prefix.clone();
                    tuple.push(color);
                    tuple
                })
            })
            .collect()
    }
    // LLM: Prepare `original_inputs` for the following computation; it stores the original
    // LLM: inputs for this stage.
    let original_inputs = prepare::input_labels(problem);
    let constraints = |active: bool| {
        // LLM: Prepare `degrees` for the following computation; it stores the degrees for this
        // LLM: stage.
        let degrees = if active {
            problem.graph_class().active()
        } else {
            problem.graph_class().passive()
        };
        // LLM: Prepare `inputs` for the following computation; it stores the inputs for this
        // LLM: stage.
        let inputs: Vec<(u32, Vec<Option<LabelId>>)> = match problem.input() {
            Input::Trivial => degrees
                .iter()
                .map(|&degree| (degree, vec![None; degree as usize]))
                .collect(),
            Input::Explicit(input) => concrete_multisets(
                if active {
                    input.active()
                } else {
                    input.passive()
                },
                degrees,
            )
            .into_iter()
            .map(|input| (input.degree(), input.occurrences().map(Some).collect()))
            .collect(),
        };
        // LLM: Prepare `configurations` for the following computation; it stores the
        // LLM: configurations for this stage.
        let mut configurations = Vec::new();
        for (degree, input) in inputs {
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for palette in palettes(colors, degree) {
                // LLM: Prepare `equal` for the following computation; it stores the equal for
                // LLM: this stage.
                let equal = (active && kind == ColoringKind::Vertex)
                    || (!active && kind == ColoringKind::GraphEdges);
                if equal && palette.windows(2).any(|pair| pair[0] != pair[1]) {
                    continue;
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if !equal
                    && palette
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != palette.len()
                {
                    continue;
                }
                // LLM: Prepare `parts` for the following computation; it stores the parts for
                // LLM: this stage.
                let parts = input
                    .iter()
                    .zip(palette)
                    .map(|(&input, color)| {
                        let index = prepare::input_index(input) as u32 * colors + color;
                        Part::new(LabelSet::new(vec![LabelId(index)]), 1).unwrap()
                    })
                    .collect();
                // LLM: Retain the newly constructed choices in the result for this stage.
                configurations.push(CondensedConfiguration::new(parts).unwrap());
            }
        }
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Constraint::new(configurations)
    };
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let names = original_inputs
        .iter()
        .enumerate()
        .flat_map(|(input, _)| (0..colors).map(move |color| format!("{input}/{color}")))
        .collect();
    let input = ConstraintPair::new(names, constraints(true), constraints(false)).unwrap();
    // LLM: Preserve the incidence permission relation while constructing the transformed
    // LLM: problem.
    let mapping = original_inputs
        .iter()
        .flat_map(|&input| {
            // LLM: Update the current working state before continuing this operation.
            (0..colors).map(move |_| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                LabelSet::new(
                    (0..problem.output().labels().len())
                        .map(|id| LabelId(id as u32))
                        .filter(|&output| problem.output_allowed(input, output))
                        .collect(),
                )
            })
        })
        .collect();
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Problem::new(
        problem.graph_class().clone(),
        Input::Explicit(input),
        problem.output().clone(),
        OutputMapping::Restricted(mapping),
    )
    .unwrap()
}

/// LLM: Regression test: verify that compact encoding agrees with explicit colored inputs.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn compact_encoding_agrees_with_explicit_colored_inputs() {
    // LLM: Prepare `fixtures` for the following computation; it stores the fixtures for this
    // LLM: stage.
    let fixtures = [
        plain("A B", "A A\nB B"),
        plain("A^2\nB^2\nC^2", "A BC\nB C"),
        plain("A B\nA C\nB C", "A A\nB B\nC C"),
        plain("A B", "A^2\nA B\nB^2"),
        problem(
            serde_json::json!({"kind":"independent", "input":{"active":"a b","passive":"ab^2"},"output":{"active":"A B","passive":"A A\nB B"}}),
        ),
        problem(
            serde_json::json!({"kind":"mapped", "input":{"active":"a b\nb^2","passive":"ab^2"},"output":{"active":"A B\nB^2","passive":"AB^2"},"mapping":"a -> A\nb -> AB"}),
        ),
        problem(
            serde_json::json!({"kind":"paired", "constraints":{"active":"(a,A) (b,B)\n(a,B) (b,A)","passive":"(a,A)(a,B) (b,A)(b,B)"}}),
        ),
        problem(
            serde_json::json!({"kind":"plain", "constraints":{"active":"A\nA B","passive":"AB^2"}}),
        ),
    ];
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for fixture in &fixtures {
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for kind in [
            ColoringKind::Vertex,
            ColoringKind::GraphEdges,
            ColoringKind::IncidenceEdges,
        ] {
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for colors in prepare::minimum(fixture, kind).unwrap()..=3 {
                // LLM: Prepare `reference` for the following computation; it stores the
                // LLM: reference for this stage.
                let reference = explicit_coloring(fixture, kind, colors);
                let expected = matches!(
                    zero_round::check(&reference, PassiveMode::General).unwrap(),
                    ZeroRoundResult::Solvable { .. }
                );
                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
                assert_eq!(
                    fixed(fixture, kind, colors),
                    expected,
                    "{kind:?}/{colors}: {fixture:?}"
                );
            }
        }
    }
}

/// LLM: Regression test: verify that original inputs and colors are both available and mapping
/// LLM: is preserved. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn original_inputs_and_colors_are_both_available_and_mapping_is_preserved() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = problem(serde_json::json!({"kind":"mapped",
        "input":{"active":"a^2\nb^2","passive":"ab^2"},
        "output":{"active":"A^2\nB^2\nC^2\nD^2","passive":"A BCD\nB CD\nC D"},
        "mapping":"a -> AB\nb -> CD"}));
    assert!(matches!(
        zero_round::check(&p, PassiveMode::General).unwrap(),
        ZeroRoundResult::Unsolvable
    ));
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(fixed(&p, ColoringKind::Vertex, 2));
    assert!(!fixed(&p, ColoringKind::Vertex, 3));
    // LLM: Using only the color would require one output allowed for both a and b;
    // LLM: their permitted output sets are disjoint. The combined strategy works.
}

/// LLM: Regression test: verify that maximum search streams proved bounds for consecutive
/// LLM: palettes. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn maximum_search_streams_proved_bounds_for_consecutive_palettes() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let mut updates = Vec::new();
    let mut progress = |_| {};
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check_with(
        &p,
        ColoringKind::Vertex,
        PaletteSearch::Maximum,
        PassiveMode::General,
        &mut Computation::new(Cancellation::default(), &mut progress),
        &mut |bounds| updates.push(bounds),
    )
    .unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(result, ColoringResult::Maximum { colors: 3, .. }));
    assert_eq!(
        updates
            .iter()
            .filter_map(|bounds| bounds.testing)
            .collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        updates
            .iter()
            .any(|bounds| bounds.lower == Some(3) && bounds.upper == Some(3))
    );
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for bounds in updates {
        if let Some(witness) = bounds.witness {
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}

/// LLM: Regression test: verify that maximum search stops at the first unsolvable palette.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn maximum_search_stops_at_the_first_unsolvable_palette() {
    // LLM: Prepare `updates` for the following computation; it stores the updates for this
    // LLM: stage.
    let mut updates = Vec::new();
    let mut queries = Vec::new();
    let mut progress = |_| {};
    // LLM: A decision oracle isolates the search policy from SAT heuristics. This
    // LLM: models the reported 10/11 boundary and catches accidental doubling.

    let result = search_maximum(
        ColoringBounds {
            minimum: 3,
            lower: None,
            upper: None,
            testing: None,
            witness: None,
        },
        &mut Computation::new(Cancellation::default(), &mut progress),
        &mut |bounds| updates.push(bounds),
        &mut |colors, _| {
            // LLM: Retain the newly constructed choices in the result for this stage.
            queries.push(colors);
            Ok((colors <= 10).then(|| ColoringWitness::LocalColors {
                behaviors: vec![
                    ColorBehavior {
                        outputs_by_input: vec![]
                    };
                    colors as usize
                ],
            }))
        },
    )
    .unwrap();
    // LLM: Compare the implementation result with the independently prepared reference for
    // LLM: this fixture.
    assert_eq!(queries, (3..=11).collect::<Vec<_>>());
    let ColoringResult::Maximum { colors, witness } = result else {
        panic!("expected finite maximum");
    };
    assert_eq!(colors, 10);
    let ColoringWitness::LocalColors { behaviors } = witness else {
        panic!("expected the last solvable witness");
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(behaviors.len(), 10);
    let last = updates.last().unwrap();
    assert_eq!(
        (last.lower, last.upper, last.testing),
        (Some(10), Some(10), None)
    );
}

/// LLM: Regression test: verify that bipartite maximal matching incidence maximum. Used by the
/// LLM: test suite to protect this behavior while implementations change.
#[test]
fn bipartite_maximal_matching_incidence_maximum() {
    // LLM: The degree-ten version has the reported 10/11 boundary; the preset's
    // LLM: degree-three version instead has maximum three.

    let p = plain("M U^9\nP^10", "M UP^9\nU^10");
    let mut queries = Vec::new();
    let mut progress = |_| {};
    let started = std::time::Instant::now();
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check_with(
        &p,
        ColoringKind::IncidenceEdges,
        PaletteSearch::Maximum,
        PassiveMode::General,
        &mut Computation::new(Cancellation::default(), &mut progress),
        &mut |bounds| {
            if let Some(colors) = bounds.testing {
                queries.push(colors);
                eprintln!("Checking {colors} colors after {:?}.", started.elapsed());
            }
        },
    )
    .unwrap();
    // LLM: Compare the implementation result with the independently prepared reference for
    // LLM: this fixture.
    assert_eq!(queries, vec![10, 11]);
    let ColoringResult::Maximum { colors, witness } = result else {
        panic!("expected finite matching maximum");
    };
    assert_eq!(colors, 10);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(verify_witness(
        &p,
        ColoringKind::IncidenceEdges,
        &witness,
        false
    ));
    eprintln!("Proved maximum 10 in {:?}.", started.elapsed());
}

/// LLM: Regression test: verify that edge palette maxima and shared vs distinct passive
/// LLM: colors. Used by the test suite to protect this behavior while implementations change.
#[test]
fn edge_palette_maxima_and_shared_vs_distinct_passive_colors() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("A B\nA C\nB C", "A A\nB B\nC C");
    assert!(matches!(
        check(
            &p,
            ColoringKind::GraphEdges,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .unwrap(),
        ColoringResult::Maximum { colors: 3, .. }
    ));
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("A\nB\nC", "A B C");
    assert!(matches!(
        check(
            &p,
            ColoringKind::IncidenceEdges,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .unwrap(),
        ColoringResult::Maximum { colors: 3, .. }
    ));
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        check(
            &p,
            ColoringKind::GraphEdges,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .is_err()
    );
}

/// LLM: Regression test: verify that both passive modes agree on manually maximized
/// LLM: constraints. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn both_passive_modes_agree_on_manually_maximized_constraints() {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for p in [
        plain("A^2\nB^2", "A B"),
        plain("A B", "AB^2"),
        plain("A\nB\nC", "A B C"),
        plain("A B\nA C\nB C", "A A\nB B\nC C"),
    ] {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for kind in [
            ColoringKind::Vertex,
            ColoringKind::GraphEdges,
            ColoringKind::IncidenceEdges,
        ] {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if prepare::minimum(&p, kind).is_err() {
                continue;
            }
            let general = check(&p, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap();
            let maximized =
                check(&p, kind, PaletteSearch::Maximum, PassiveMode::Maximized).unwrap();
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                std::mem::discriminant(&general),
                std::mem::discriminant(&maximized)
            );
            // LLM: Validate the returned color behavior tables with the independent
            // LLM: certificate checker.
            match maximized {
                ColoringResult::Maximum { colors, witness } => {
                    assert!(
                        matches!(general, ColoringResult::Maximum { colors: expected, .. } if colors == expected)
                    );
                    assert!(verify_witness(&p, kind, &witness, false));
                }
                ColoringResult::Unbounded { witness, .. } => {
                    assert!(verify_witness(&p, kind, &witness, true))
                }
                ColoringResult::NoPalette => {}
                result => panic!("unexpected {result:?}"),
            }
        }
    }
}

/// LLM: Regression test: verify that unbounded certificates use original inputs and cover
/// LLM: nonregular and zero degree. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn unbounded_certificates_use_original_inputs_and_cover_nonregular_and_zero_degree() {
    // LLM: Prepare `inputs` for the following computation; it stores the inputs for this
    // LLM: stage.
    let inputs = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"a^2\nb^2","passive":"a b"},
        "output":{"active":"A^2\nB^2","passive":"A B"},"mapping":"a -> A\nb -> B"}),
    );
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for p in [inputs, plain("()\nA\nA^3", "()\nA^2"), plain("A B", "AB^2")] {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for kind in [ColoringKind::Vertex, ColoringKind::IncidenceEdges] {
            // LLM: Prepare `ColoringResult` for the following computation; it stores the
            // LLM: coloring result for this stage.
            let ColoringResult::Unbounded { witness, .. } =
                check(&p, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap()
            else {
                panic!("expected unbounded");
            };
            assert!(verify_witness(&p, kind, &witness, true));
            let mut broken = witness;
            // LLM: Prepare `ColoringWitness` for the following computation; it stores the
            // LLM: coloring witness for this stage.
            let ColoringWitness::LocalColors { behaviors } = &mut broken else {
                panic!("expected local-color witness");
            };
            for behavior in behaviors {
                behavior.outputs_by_input.fill(LabelSet::default());
            }
            assert!(!verify_witness(&p, kind, &broken, true));
        }
    }
    // LLM: Prepare `empty` for the following computation; it stores the empty for this stage.
    let empty = plain("", "");
    assert!(matches!(
        check(
            &empty,
            ColoringKind::Vertex,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .unwrap(),
        ColoringResult::Unbounded { .. }
    ));
    // LLM: Prepare `impossible_zero` for the following computation; it stores the impossible
    // LLM: zero for this stage.
    let impossible_zero = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"","passive":"()"}, "degrees":{"active":"0","passive":"0"}}),
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(
        check(
            &impossible_zero,
            ColoringKind::Vertex,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .unwrap(),
        ColoringResult::NoPalette
    ));
}

/// LLM: Regression test: verify that partial results can cancel before the next query. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn partial_results_can_cancel_before_the_next_query() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let cancellation = Cancellation::default();
    let mut progress = |_| {};
    let mut last = None;
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check_with(
        &p,
        ColoringKind::Vertex,
        PaletteSearch::Maximum,
        PassiveMode::General,
        &mut Computation::new(cancellation.clone(), &mut progress),
        &mut |bounds| {
            if bounds.lower.is_some() {
                last = Some(bounds);
                cancellation.cancel();
            }
        },
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(result.is_err());
    let last = last.unwrap();
    assert_eq!(last.lower, Some(2));
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(verify_witness(
        &p,
        ColoringKind::Vertex,
        &last.witness.unwrap(),
        false
    ));
}

/// LLM: Regression test: verify that re matching preserves the incidence palette boundary.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn re_matching_preserves_the_incidence_palette_boundary() {
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for degree in [2, 3, 10] {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = plain(
            &format!("M U^{}\nP^{degree}", degree - 1),
            &format!("M UP^{}\nU^{degree}", degree - 1),
        );
        let p = crate::algorithms::round_elimination::apply(&p).unwrap();
        let mut report = |_| {};
        let mut queries = Vec::new();
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = check_with(
            &p,
            ColoringKind::IncidenceEdges,
            PaletteSearch::Maximum,
            PassiveMode::General,
            &mut Computation::new(Cancellation::default(), &mut report),
            &mut |bounds| {
                if let Some(colors) = bounds.testing {
                    queries.push(colors);
                }
            },
        )
        .unwrap();
        // LLM: Compare the implementation result with the independently prepared reference for
        // LLM: this fixture.
        assert_eq!(queries, [degree, degree + 1]);
        let ColoringResult::Maximum { colors, witness } = result else {
            panic!("finite maximum expected")
        };
        assert_eq!(colors, degree);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(verify_witness(
            &p,
            ColoringKind::IncidenceEdges,
            &witness,
            false
        ));
    }
}

/// LLM: Regression test: verify that incremental solver can disable and reenable colors. Used
/// LLM: by the test suite to protect this behavior while implementations change.
#[test]
fn incremental_solver_can_disable_and_reenable_colors() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("A^2\nB^2\nC^2", "A BC\nB C");
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    // LLM: Prepare `solver` for the following computation; it stores the solver for this
    // LLM: stage.
    let mut solver = encoding::PaletteSolver::new(
        &p,
        ColoringKind::Vertex,
        crate::algorithms::sat::backend(&computation.cancellation),
        &mut computation,
    )
    .unwrap();
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    for colors in [2, 3, 4, 2, 3, 5, 2] {
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        let witness = solver
            .solve(colors, PassiveMode::General, &mut computation)
            .unwrap();
        assert_eq!(witness.is_some(), colors <= 3);
        if let Some(witness) = witness {
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}
