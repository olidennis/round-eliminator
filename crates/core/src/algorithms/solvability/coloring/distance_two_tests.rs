use super::*;
use crate::{
    algorithms::{configurations::concrete_multisets, solvability::zero_round::ZeroRoundResult},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, Input, LabelId, OutputMapping, Part,
    },
};

use super::tests::{fixed, plain, problem};

// This intentionally materializes tiny colored input problems as an independent
// reference. Production keeps color neighborhoods and original inputs implicit.
fn explicit_distance_two(problem: &Problem, colors: u32) -> Problem {
    fn tuples(colors: u32, degree: u32) -> Vec<Vec<u32>> {
        if degree == 0 {
            return vec![vec![]];
        }
        tuples(colors, degree - 1)
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
    let originals = prepare::input_labels(problem);
    let index = |input: Option<LabelId>, own: u32, neighbor: u32| {
        LabelId((prepare::input_index(input) as u32 * colors + own) * colors + neighbor)
    };
    let constraints = |active: bool| {
        let degrees = if active {
            problem.graph_class().active()
        } else {
            problem.graph_class().passive()
        };
        let inputs: Vec<Vec<Option<LabelId>>> = match problem.input() {
            Input::Trivial => degrees
                .iter()
                .map(|&degree| vec![None; degree as usize])
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
            .map(|configuration| configuration.occurrences().map(Some).collect())
            .collect(),
        };
        let mut configurations = Vec::new();
        for input in inputs {
            if active {
                for own in 0..colors {
                    for neighbors in tuples(colors, input.len() as u32) {
                        if neighbors.contains(&own)
                            || neighbors
                                .iter()
                                .collect::<std::collections::BTreeSet<_>>()
                                .len()
                                != neighbors.len()
                        {
                            continue;
                        }
                        let parts = input
                            .iter()
                            .zip(&neighbors)
                            .map(|(&input, &neighbor)| {
                                Part::new(LabelSet::new(vec![index(input, own, neighbor)]), 1)
                                    .unwrap()
                            })
                            .collect();
                        configurations.push(CondensedConfiguration::new(parts).unwrap());
                    }
                }
            } else {
                for first in 0..colors {
                    for second in 0..colors {
                        if first == second {
                            continue;
                        }
                        // The passive input is unordered, so permit both ways
                        // of attaching its original inputs to the endpoints.
                        for (left, right) in [(input[0], input[1]), (input[1], input[0])] {
                            configurations.push(
                                CondensedConfiguration::new(vec![
                                    Part::new(LabelSet::new(vec![index(left, first, second)]), 1)
                                        .unwrap(),
                                    Part::new(LabelSet::new(vec![index(right, second, first)]), 1)
                                        .unwrap(),
                                ])
                                .unwrap(),
                            );
                        }
                    }
                }
            }
        }
        Constraint::new(configurations)
    };
    let names = originals
        .iter()
        .enumerate()
        .flat_map(|(input, _)| {
            (0..colors).flat_map(move |own| {
                (0..colors).map(move |neighbor| format!("{input}/{own}/{neighbor}"))
            })
        })
        .collect();
    let input = ConstraintPair::new(names, constraints(true), constraints(false)).unwrap();
    let mapping = originals
        .iter()
        .flat_map(|&input| {
            (0..colors * colors).map(move |_| {
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
fn distance_two_agrees_with_explicit_neighbor_color_inputs() {
    let fixtures = [
        plain("O I", "O I"),
        plain("A B", "A A\nB B"),
        plain("A^2\nB^2\nC^2", "A BC\nB C"),
        plain("()\nA\nA B", "AB^2"),
        problem(serde_json::json!({"kind":"independent",
            "input":{"active":"a b","passive":"ab^2"},
            "output":{"active":"O I","passive":"O I"}})),
        problem(serde_json::json!({"kind":"mapped",
            "input":{"active":"a b\nb^2","passive":"ab^2"},
            "output":{"active":"A B\nB^2","passive":"AB^2"},
            "mapping":"a -> A\nb -> AB"})),
        problem(serde_json::json!({"kind":"paired",
            "constraints":{"active":"(a,A) (b,B)\n(a,B) (b,A)",
                "passive":"(a,A)(a,B) (b,A)(b,B)"}})),
    ];
    for fixture in &fixtures {
        for colors in prepare::minimum(fixture, ColoringKind::DistanceTwo).unwrap()..=4 {
            let reference = explicit_distance_two(fixture, colors);
            let expected = matches!(
                zero_round::check(&reference, PassiveMode::General).unwrap(),
                ZeroRoundResult::Solvable { .. }
            );
            assert_eq!(
                fixed(fixture, ColoringKind::DistanceTwo, colors),
                expected,
                "{colors}: {fixture:?}"
            );
        }
    }
}

#[test]
fn knowing_neighbor_colors_enables_sinkless_and_sourceless_on_cycles() {
    let p = plain("O I", "O I");
    assert!(!fixed(&p, ColoringKind::Vertex, 3));
    // With three distance-two colors, every node has the other two colors
    // as neighbors. Orient the color triangle cyclically.
    assert!(fixed(&p, ColoringKind::DistanceTwo, 3));
    assert!(!fixed(&p, ColoringKind::DistanceTwo, 4));
    // O I is already maximized: adding either label to either part would
    // introduce the forbidden O O or I I.
    for colors in [3, 4] {
        let result = check(
            &p,
            ColoringKind::DistanceTwo,
            PaletteSearch::Fixed { colors },
            PassiveMode::Maximized,
        )
        .unwrap();
        assert_eq!(
            matches!(result, ColoringResult::Solvable { .. }),
            colors == 3
        );
        if let ColoringResult::Solvable { witness, .. } = result {
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
}

#[test]
fn distance_two_validates_graph_palette_and_witness_shape() {
    let p = plain("O I", "O I");
    assert!(
        check(
            &p,
            ColoringKind::DistanceTwo,
            PaletteSearch::Fixed { colors: 2 },
            PassiveMode::General
        )
        .is_err()
    );
    assert!(
        check(
            &plain("A", "A^3"),
            ColoringKind::DistanceTwo,
            PaletteSearch::Fixed { colors: 3 },
            PassiveMode::General
        )
        .is_err()
    );
    let ColoringResult::Solvable { witness, .. } = check(
        &p,
        ColoringKind::DistanceTwo,
        PaletteSearch::Fixed { colors: 3 },
        PassiveMode::General,
    )
    .unwrap() else {
        panic!("expected solvable");
    };
    assert!(!verify_witness(&p, ColoringKind::Vertex, &witness, false));
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &witness,
        true
    ));
    let mut broken = witness.clone();
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    behaviors[0].outputs_by_neighbor.pop();
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
    let mut broken = witness.clone();
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    behaviors[0].outputs_by_neighbor[0].outputs_by_input[0] = LabelSet::new(vec![LabelId(0)]);
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
    let mut broken = witness;
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    for behavior in behaviors {
        for neighbor in &mut behavior.outputs_by_neighbor {
            neighbor.outputs_by_input.fill(LabelSet::default());
        }
    }
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
}

#[test]
fn distance_two_handles_missing_outputs_isolated_nodes_and_cancellation() {
    let p = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"","passive":"A^2"}, "degrees":{"active":"0 1 2","passive":"2"}}),
    );
    assert!(!fixed(&p, ColoringKind::DistanceTwo, 3));
    assert!(fixed(&plain("()", "A^2"), ColoringKind::DistanceTwo, 1));
    let cancellation = Cancellation::default();
    let mut phases = Vec::new();
    let mut report = |progress: crate::computation::Progress| {
        phases.push(progress.phase);
        if progress.phase == crate::computation::Phase::Verifying {
            cancellation.cancel();
        }
    };
    let result = check_with(
        &plain("O I", "O I"),
        ColoringKind::DistanceTwo,
        PaletteSearch::Fixed { colors: 3 },
        PassiveMode::General,
        &mut Computation::new(cancellation.clone(), &mut report),
        &mut |_| {},
    );
    assert!(result.is_err());
    assert!(phases.contains(&crate::computation::Phase::Solving));
    assert!(cancellation.is_cancelled());
}

#[test]
fn distance_two_maximum_streams_bounds_and_shrinks_the_incremental_palette() {
    let p = plain("O I", "O I");
    let mut updates = Vec::new();
    let mut progress = |_| {};
    let result = check_with(
        &p,
        ColoringKind::DistanceTwo,
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
        vec![3, 6, 4]
    );
    for bounds in updates {
        if let Some(witness) = bounds.witness {
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
    let ColoringResult::Maximum { colors, witness } = check(
        &plain("A B C", "A A\nB B\nC C"),
        ColoringKind::DistanceTwo,
        PaletteSearch::Maximum,
        PassiveMode::General,
    )
    .unwrap() else {
        panic!("expected finite edge-coloring bound");
    };
    assert_eq!(colors, 4);
    assert!(verify_witness(
        &plain("A B C", "A A\nB B\nC C"),
        ColoringKind::DistanceTwo,
        &witness,
        false
    ));
    let impossible = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"A^2","passive":""}, "degrees":{"active":"2","passive":"2"}}),
    );
    assert!(matches!(
        check(
            &impossible,
            ColoringKind::DistanceTwo,
            PaletteSearch::Maximum,
            PassiveMode::General
        )
        .unwrap(),
        ColoringResult::NoPalette
    ));
}

#[test]
fn order_certificates_work_for_all_entry_forms_and_nonregular_degrees() {
    let fixtures = [
        plain("OI^2", "O I"),
        plain("()\nOI\nOI^3", "O I"),
        plain("()", "O I"),
        problem(
            serde_json::json!({"kind":"independent", "input":{"active":"a^2\nb^2","passive":"ab^2"}, "output":{"active":"OI^2","passive":"O I"}}),
        ),
        problem(
            serde_json::json!({"kind":"mapped", "input":{"active":"a^2\nb^2","passive":"ab^2"}, "output":{"active":"AB^2\nCD^2","passive":"AC BD"}, "mapping":"a -> AB\nb -> CD"}),
        ),
        problem(
            serde_json::json!({"kind":"paired", "constraints":{"active":"(a,O)(a,I)^2\n(b,O)(b,I)^2", "passive":"(a,O)(b,O) (a,I)(b,I)"}}),
        ),
    ];
    // These passive rectangles are all manually maximized. There is no
    // maximization step or model flag in either analysis.
    for p in &fixtures {
        for mode in [PassiveMode::General, PassiveMode::Maximized] {
            let ColoringResult::Unbounded { minimum, witness } =
                check(p, ColoringKind::DistanceTwo, PaletteSearch::Maximum, mode).unwrap()
            else {
                panic!("expected unbounded: {p:?}");
            };
            assert_eq!(
                minimum,
                prepare::minimum(p, ColoringKind::DistanceTwo).unwrap()
            );
            assert!(matches!(witness, ColoringWitness::ColorOrder { .. }));
            assert!(verify_witness(p, ColoringKind::DistanceTwo, &witness, true));
            assert!(!verify_witness(
                p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
            assert!(!verify_witness(p, ColoringKind::Vertex, &witness, true));
            for colors in minimum..=minimum + 2 {
                assert!(fixed(p, ColoringKind::DistanceTwo, colors));
            }
        }
    }
    // Orienting from smaller to larger needs comparisons; there is no
    // color-independent zero-round strategy for this problem.
    assert!(matches!(
        zero_round::check(&fixtures[0], PassiveMode::General).unwrap(),
        ZeroRoundResult::Unsolvable
    ));
    let mut broken = check(
        &fixtures[4],
        ColoringKind::DistanceTwo,
        PaletteSearch::Maximum,
        PassiveMode::General,
    )
    .unwrap();
    let ColoringResult::Unbounded {
        witness: ColoringWitness::ColorOrder { smaller, larger },
        ..
    } = &mut broken
    else {
        panic!("expected order certificate");
    };
    smaller.outputs_by_input[0] = LabelSet::new(vec![LabelId(2)]); // C is forbidden for a.
    let witness = ColoringWitness::ColorOrder {
        smaller: smaller.clone(),
        larger: larger.clone(),
    };
    assert!(!verify_witness(
        &fixtures[4],
        ColoringKind::DistanceTwo,
        &witness,
        true
    ));
}

#[test]
fn order_encoding_agrees_with_exhaustive_output_set_tables() {
    // Independent oracle: all two-label output sets, all four comparison
    // patterns at degree 2, and every smaller/larger pair across an edge.
    // Neither the sign-input reduction nor the generic witness checker is used.
    let configurations = ["O^2", "O I", "I^2"];
    for active in 0u8..8 {
        for passive in 0u8..8 {
            let allowed = |mask: u8| {
                configurations
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, line)| *line)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let p = problem(
                serde_json::json!({"kind":"plain", "constraints":{"active":allowed(active),"passive":allowed(passive)}, "degrees":{"active":"2","passive":"2"}}),
            );
            let expected = (0u8..4).any(|smaller| {
                (0u8..4).any(|larger| {
                    let available = |set: u8| (0u8..2).filter(move |label| set & (1 << label) != 0);
                    let active_valid = [smaller, larger].iter().all(|&first| {
                        [smaller, larger].iter().all(|&second| {
                            available(first).any(|left| {
                                available(second).any(|right| active & (1 << (left + right)) != 0)
                            })
                        })
                    });
                    active_valid
                        && available(smaller).all(|left| {
                            available(larger).all(|right| passive & (1 << (left + right)) != 0)
                        })
                })
            });
            let mut progress = |_| {};
            let witness = order::check_with(
                &p,
                PassiveMode::General,
                &mut Computation::new(Cancellation::default(), &mut progress),
            )
            .unwrap();
            assert_eq!(
                witness.is_some(),
                expected,
                "active={active} passive={passive}"
            );
            if let Some(witness) = witness {
                assert!(verify_witness(
                    &p,
                    ColoringKind::DistanceTwo,
                    &witness,
                    true
                ));
                for colors in [3, 4, 5] {
                    assert!(fixed(&p, ColoringKind::DistanceTwo, colors));
                }
            }
        }
    }
}

#[test]
fn distance_two_maximum_cancellation_retains_the_last_proved_strategy() {
    let p = plain("O I", "O I");
    let cancellation = Cancellation::default();
    let mut progress = |_| {};
    let mut last = None;
    let result = check_with(
        &p,
        ColoringKind::DistanceTwo,
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
    let bounds = last.unwrap();
    assert_eq!(bounds.lower, Some(3));
    assert_eq!(bounds.upper, None);
    assert!(verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &bounds.witness.unwrap(),
        false
    ));
}
