use super::*;
use crate::{
    algorithms::{configurations::concrete_multisets, solvability::zero_round::ZeroRoundResult},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, Input, LabelId, OutputMapping, Part,
    },
    parser::parse_problem,
};

pub(super) fn problem(value: serde_json::Value) -> Problem {
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

pub(super) fn plain(active: &str, passive: &str) -> Problem {
    problem(serde_json::json!({"kind":"plain", "constraints":{"active":active,"passive":passive}}))
}

pub(super) fn fixed(problem: &Problem, kind: ColoringKind, colors: u32) -> bool {
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

// Independent color enumeration, deliberately including all permutations.
// Construct a small explicit colored input promise and use the separately
// tested local-rule solver as a reference for the compact membership encoding.
fn explicit_coloring(problem: &Problem, kind: ColoringKind, colors: u32) -> Problem {
    fn palettes(colors: u32, degree: u32) -> Vec<Vec<u32>> {
        if degree == 0 {
            return vec![vec![]];
        }
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
    let original_inputs = prepare::input_labels(problem);
    let constraints = |active: bool| {
        let degrees = if active {
            problem.graph_class().active()
        } else {
            problem.graph_class().passive()
        };
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
        let mut configurations = Vec::new();
        for (degree, input) in inputs {
            for palette in palettes(colors, degree) {
                let equal = (active && kind == ColoringKind::Vertex)
                    || (!active && kind == ColoringKind::GraphEdges);
                if equal && palette.windows(2).any(|pair| pair[0] != pair[1]) {
                    continue;
                }
                if !equal
                    && palette
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != palette.len()
                {
                    continue;
                }
                let parts = input
                    .iter()
                    .zip(palette)
                    .map(|(&input, color)| {
                        let index = prepare::input_index(input) as u32 * colors + color;
                        Part::new(LabelSet::new(vec![LabelId(index)]), 1).unwrap()
                    })
                    .collect();
                configurations.push(CondensedConfiguration::new(parts).unwrap());
            }
        }
        Constraint::new(configurations)
    };
    let names = original_inputs
        .iter()
        .enumerate()
        .flat_map(|(input, _)| (0..colors).map(move |color| format!("{input}/{color}")))
        .collect();
    let input = ConstraintPair::new(names, constraints(true), constraints(false)).unwrap();
    let mapping = original_inputs
        .iter()
        .flat_map(|&input| {
            (0..colors).map(move |_| {
                LabelSet::new(
                    (0..problem.output().labels().len())
                        .map(|id| LabelId(id as u32))
                        .filter(|&output| problem.output_allowed(input, output))
                        .collect(),
                )
            })
        })
        .collect();
    Problem::new(
        problem.graph_class().clone(),
        Input::Explicit(input),
        problem.output().clone(),
        OutputMapping::Restricted(mapping),
    )
    .unwrap()
}

#[test]
fn compact_encoding_agrees_with_explicit_colored_inputs() {
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
    for fixture in &fixtures {
        for kind in [
            ColoringKind::Vertex,
            ColoringKind::GraphEdges,
            ColoringKind::IncidenceEdges,
        ] {
            for colors in prepare::minimum(fixture, kind).unwrap()..=3 {
                let reference = explicit_coloring(fixture, kind, colors);
                let expected = matches!(
                    zero_round::check(&reference, PassiveMode::General).unwrap(),
                    ZeroRoundResult::Solvable { .. }
                );
                assert_eq!(
                    fixed(fixture, kind, colors),
                    expected,
                    "{kind:?}/{colors}: {fixture:?}"
                );
            }
        }
    }
}

#[test]
fn original_inputs_and_colors_are_both_available_and_mapping_is_preserved() {
    let p = problem(serde_json::json!({"kind":"mapped",
        "input":{"active":"a^2\nb^2","passive":"ab^2"},
        "output":{"active":"A^2\nB^2\nC^2\nD^2","passive":"A BCD\nB CD\nC D"},
        "mapping":"a -> AB\nb -> CD"}));
    assert!(matches!(
        zero_round::check(&p, PassiveMode::General).unwrap(),
        ZeroRoundResult::Unsolvable
    ));
    assert!(fixed(&p, ColoringKind::Vertex, 2));
    assert!(!fixed(&p, ColoringKind::Vertex, 3));
    // Using only the color would require one output allowed for both a and b;
    // their permitted output sets are disjoint. The combined strategy works.
}

#[test]
fn maximum_search_streams_proved_bounds_for_consecutive_palettes() {
    let p = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let mut updates = Vec::new();
    let mut progress = |_| {};
    let result = check_with(
        &p,
        ColoringKind::Vertex,
        PaletteSearch::Maximum,
        PassiveMode::General,
        &mut Computation::new(Cancellation::default(), &mut progress),
        &mut |bounds| updates.push(bounds),
    )
    .unwrap();
    assert!(matches!(result, ColoringResult::Maximum { colors: 3, .. }));
    assert_eq!(
        updates
            .iter()
            .filter_map(|bounds| bounds.testing)
            .collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
    assert!(
        updates
            .iter()
            .any(|bounds| bounds.lower == Some(3) && bounds.upper == Some(3))
    );
    for bounds in updates {
        if let Some(witness) = bounds.witness {
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}

#[test]
fn maximum_search_stops_at_the_first_unsolvable_palette() {
    let mut updates = Vec::new();
    let mut queries = Vec::new();
    let mut progress = |_| {};
    // A decision oracle isolates the search policy from SAT heuristics. This
    // models the reported 10/11 boundary and catches accidental doubling.
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
    assert_eq!(queries, (3..=11).collect::<Vec<_>>());
    let ColoringResult::Maximum { colors, witness } = result else {
        panic!("expected finite maximum");
    };
    assert_eq!(colors, 10);
    let ColoringWitness::LocalColors { behaviors } = witness else {
        panic!("expected the last solvable witness");
    };
    assert_eq!(behaviors.len(), 10);
    let last = updates.last().unwrap();
    assert_eq!(
        (last.lower, last.upper, last.testing),
        (Some(10), Some(10), None)
    );
}

#[test]
fn bipartite_maximal_matching_incidence_maximum() {
    // The degree-ten version has the reported 10/11 boundary; the preset's
    // degree-three version instead has maximum three.
    let p = plain("M U^9\nP^10", "M UP^9\nU^10");
    let mut queries = Vec::new();
    let mut progress = |_| {};
    let started = std::time::Instant::now();
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
    assert_eq!(queries, vec![10, 11]);
    let ColoringResult::Maximum { colors, witness } = result else {
        panic!("expected finite matching maximum");
    };
    assert_eq!(colors, 10);
    assert!(verify_witness(
        &p,
        ColoringKind::IncidenceEdges,
        &witness,
        false
    ));
    eprintln!("Proved maximum 10 in {:?}.", started.elapsed());
}

#[test]
fn edge_palette_maxima_and_shared_vs_distinct_passive_colors() {
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

#[test]
fn both_passive_modes_agree_on_manually_maximized_constraints() {
    for p in [
        plain("A^2\nB^2", "A B"),
        plain("A B", "AB^2"),
        plain("A\nB\nC", "A B C"),
        plain("A B\nA C\nB C", "A A\nB B\nC C"),
    ] {
        for kind in [
            ColoringKind::Vertex,
            ColoringKind::GraphEdges,
            ColoringKind::IncidenceEdges,
        ] {
            if prepare::minimum(&p, kind).is_err() {
                continue;
            }
            let general = check(&p, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap();
            let maximized =
                check(&p, kind, PaletteSearch::Maximum, PassiveMode::Maximized).unwrap();
            assert_eq!(
                std::mem::discriminant(&general),
                std::mem::discriminant(&maximized)
            );
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

#[test]
fn unbounded_certificates_use_original_inputs_and_cover_nonregular_and_zero_degree() {
    let inputs = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"a^2\nb^2","passive":"a b"},
        "output":{"active":"A^2\nB^2","passive":"A B"},"mapping":"a -> A\nb -> B"}),
    );
    for p in [inputs, plain("()\nA\nA^3", "()\nA^2"), plain("A B", "AB^2")] {
        for kind in [ColoringKind::Vertex, ColoringKind::IncidenceEdges] {
            let ColoringResult::Unbounded { witness, .. } =
                check(&p, kind, PaletteSearch::Maximum, PassiveMode::General).unwrap()
            else {
                panic!("expected unbounded");
            };
            assert!(verify_witness(&p, kind, &witness, true));
            let mut broken = witness;
            let ColoringWitness::LocalColors { behaviors } = &mut broken else {
                panic!("expected local-color witness");
            };
            for behavior in behaviors {
                behavior.outputs_by_input.fill(LabelSet::default());
            }
            assert!(!verify_witness(&p, kind, &broken, true));
        }
    }
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
    let impossible_zero = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"","passive":"()"}, "degrees":{"active":"0","passive":"0"}}),
    );
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

#[test]
fn partial_results_can_cancel_before_the_next_query() {
    let p = plain("A^3\nB^3\nC^3", "A BC\nB C");
    let cancellation = Cancellation::default();
    let mut progress = |_| {};
    let mut last = None;
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
    assert!(result.is_err());
    let last = last.unwrap();
    assert_eq!(last.lower, Some(2));
    assert!(verify_witness(
        &p,
        ColoringKind::Vertex,
        &last.witness.unwrap(),
        false
    ));
}

#[test]
fn re_matching_preserves_the_incidence_palette_boundary() {
    for degree in [2, 3, 10] {
        let p = plain(
            &format!("M U^{}\nP^{degree}", degree - 1),
            &format!("M UP^{}\nU^{degree}", degree - 1),
        );
        let p = crate::algorithms::round_elimination::apply(&p).unwrap();
        let mut report = |_| {};
        let mut queries = Vec::new();
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
        assert_eq!(queries, [degree, degree + 1]);
        let ColoringResult::Maximum { colors, witness } = result else {
            panic!("finite maximum expected")
        };
        assert_eq!(colors, degree);
        assert!(verify_witness(
            &p,
            ColoringKind::IncidenceEdges,
            &witness,
            false
        ));
    }
}

#[test]
fn incremental_solver_can_disable_and_reenable_colors() {
    let p = plain("A^2\nB^2\nC^2", "A BC\nB C");
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    let mut solver = encoding::PaletteSolver::new(
        &p,
        ColoringKind::Vertex,
        crate::algorithms::sat::backend(&computation.cancellation),
        &mut computation,
    )
    .unwrap();
    for colors in [2, 3, 4, 2, 3, 5, 2] {
        let witness = solver
            .solve(colors, PassiveMode::General, &mut computation)
            .unwrap();
        assert_eq!(witness.is_some(), colors <= 3);
        if let Some(witness) = witness {
            assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
        }
    }
}
