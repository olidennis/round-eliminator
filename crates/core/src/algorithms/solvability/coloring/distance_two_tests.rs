use super::*;
use crate::{
    algorithms::{configurations::concrete_multisets, solvability::zero_round::ZeroRoundResult},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, Input, LabelId, OutputMapping, Part,
    },
};

use super::tests::{fixed, plain, problem};

// LLM: This intentionally materializes tiny colored input problems as an independent
// LLM: reference. Production keeps color neighborhoods and original inputs implicit.
/// LLM: Materialize own/neighbor colors and original inputs as a tiny explicit input problem.
/// LLM: Used as the independent oracle for the compact distance-two encoding.
fn explicit_distance_two(problem: &Problem, colors: u32) -> Problem {
    /// LLM: Enumerate small neighbor-color tuples for the explicit distance-two oracle. Used
    /// LLM: only in equivalence tests, not production palette preparation.
    fn tuples(colors: u32, degree: u32) -> Vec<Vec<u32>> {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if degree == 0 {
            return vec![vec![]];
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
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
    // LLM: Prepare `originals` for the following computation; it stores the originals for this
    // LLM: stage.
    let originals = prepare::input_labels(problem);
    let index = |input: Option<LabelId>, own: u32, neighbor: u32| {
        LabelId((prepare::input_index(input) as u32 * colors + own) * colors + neighbor)
    };
    // LLM: Prepare `constraints` for the following computation; it stores the constraints for
    // LLM: this stage.
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
        // LLM: Prepare `configurations` for the following computation; it stores the
        // LLM: configurations for this stage.
        let mut configurations = Vec::new();
        for input in inputs {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if active {
                // LLM: Visit the color placements required by the auxiliary promise while
                // LLM: retaining original input context.
                for own in 0..colors {
                    // LLM: Visit the color placements required by the auxiliary promise while
                    // LLM: retaining original input context.
                    for neighbors in tuples(colors, input.len() as u32) {
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if neighbors.contains(&own)
                            || neighbors
                                .iter()
                                .collect::<std::collections::BTreeSet<_>>()
                                .len()
                                != neighbors.len()
                        {
                            continue;
                        }
                        // LLM: Prepare `parts` for the following computation; it stores the
                        // LLM: parts for this stage.
                        let parts = input
                            .iter()
                            .zip(&neighbors)
                            .map(|(&input, &neighbor)| {
                                Part::new(LabelSet::new(vec![index(input, own, neighbor)]), 1)
                                    .unwrap()
                            })
                            .collect();
                        // LLM: Retain the newly constructed choices in the result for this
                        // LLM: stage.
                        configurations.push(CondensedConfiguration::new(parts).unwrap());
                    }
                }
            } else {
                // LLM: Visit the color placements required by the auxiliary promise while
                // LLM: retaining original input context.
                for first in 0..colors {
                    // LLM: Visit the color placements required by the auxiliary promise while
                    // LLM: retaining original input context.
                    for second in 0..colors {
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if first == second {
                            continue;
                        }
                        // LLM: The passive input is unordered, so permit both ways
                        // LLM: of attaching its original inputs to the endpoints.

                        for (left, right) in [(input[0], input[1]), (input[1], input[0])] {
                            // LLM: Retain the newly constructed choices in the result for this
                            // LLM: stage.
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
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Constraint::new(configurations)
    };
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let names = originals
        .iter()
        .enumerate()
        .flat_map(|(input, _)| {
            (0..colors).flat_map(move |own| {
                (0..colors).map(move |neighbor| format!("{input}/{own}/{neighbor}"))
            })
        })
        .collect();
    // LLM: Retain or prepare the original promised input semantics for this stage.
    let input = ConstraintPair::new(names, constraints(true), constraints(false)).unwrap();
    let mapping = originals
        .iter()
        .flat_map(|&input| {
            // LLM: Update the current working state before continuing this operation.
            (0..colors * colors).map(move |_| {
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

/// LLM: Regression test: verify that distance two agrees with explicit neighbor color inputs.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn distance_two_agrees_with_explicit_neighbor_color_inputs() {
    // LLM: Prepare `fixtures` for the following computation; it stores the fixtures for this
    // LLM: stage.
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
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for fixture in &fixtures {
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for colors in prepare::minimum(fixture, ColoringKind::DistanceTwo).unwrap()..=4 {
            // LLM: Prepare `reference` for the following computation; it stores the reference
            // LLM: for this stage.
            let reference = explicit_distance_two(fixture, colors);
            let expected = matches!(
                zero_round::check(&reference, PassiveMode::General).unwrap(),
                ZeroRoundResult::Solvable { .. }
            );
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(
                fixed(fixture, ColoringKind::DistanceTwo, colors),
                expected,
                "{colors}: {fixture:?}"
            );
        }
    }
}

/// LLM: Regression test: verify that knowing neighbor colors enables sinkless and sourceless
/// LLM: on cycles. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn knowing_neighbor_colors_enables_sinkless_and_sourceless_on_cycles() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("O I", "O I");
    assert!(!fixed(&p, ColoringKind::Vertex, 3));
    // LLM: With three distance-two colors, every node has the other two colors
    // LLM: as neighbors. Orient the color triangle cyclically.

    assert!(fixed(&p, ColoringKind::DistanceTwo, 3));
    assert!(!fixed(&p, ColoringKind::DistanceTwo, 4));
    // LLM: O I is already maximized: adding either label to either part would
    // LLM: introduce the forbidden O O or I I.

    for colors in [3, 4] {
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = check(
            &p,
            ColoringKind::DistanceTwo,
            PaletteSearch::Fixed { colors },
            PassiveMode::Maximized,
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            matches!(result, ColoringResult::Solvable { .. }),
            colors == 3
        );
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        if let ColoringResult::Solvable { witness, .. } = result {
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
}

/// LLM: Regression test: verify that distance two validates graph palette and witness shape.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn distance_two_validates_graph_palette_and_witness_shape() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
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
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        check(
            &plain("A", "A^3"),
            ColoringKind::DistanceTwo,
            PaletteSearch::Fixed { colors: 3 },
            PassiveMode::General
        )
        .is_err()
    );
    // LLM: Prepare `ColoringResult` for the following computation; it stores the coloring
    // LLM: result for this stage.
    let ColoringResult::Solvable { witness, .. } = check(
        &p,
        ColoringKind::DistanceTwo,
        PaletteSearch::Fixed { colors: 3 },
        PassiveMode::General,
    )
    .unwrap() else {
        panic!("expected solvable");
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_witness(&p, ColoringKind::Vertex, &witness, false));
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &witness,
        true
    ));
    let mut broken = witness.clone();
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    behaviors[0].outputs_by_neighbor.pop();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
    let mut broken = witness.clone();
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    behaviors[0].outputs_by_neighbor[0].outputs_by_input[0] = LabelSet::new(vec![LabelId(0)]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
    let mut broken = witness;
    // LLM: Prepare `ColoringWitness` for the following computation; it stores the coloring
    // LLM: witness for this stage.
    let ColoringWitness::NeighborColors { behaviors } = &mut broken else {
        panic!("expected neighbor colors");
    };
    for behavior in behaviors {
        for neighbor in &mut behavior.outputs_by_neighbor {
            neighbor.outputs_by_input.fill(LabelSet::default());
        }
    }
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &broken,
        false
    ));
}

/// LLM: Regression test: verify that distance two handles missing outputs isolated nodes and
/// LLM: cancellation. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn distance_two_handles_missing_outputs_isolated_nodes_and_cancellation() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"","passive":"A^2"}, "degrees":{"active":"0 1 2","passive":"2"}}),
    );
    assert!(!fixed(&p, ColoringKind::DistanceTwo, 3));
    assert!(fixed(&plain("()", "A^2"), ColoringKind::DistanceTwo, 1));
    let cancellation = Cancellation::default();
    let mut phases = Vec::new();
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |progress: crate::computation::Progress| {
        phases.push(progress.phase);
        if progress.phase == crate::computation::Phase::Verifying {
            cancellation.cancel();
        }
    };
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check_with(
        &plain("O I", "O I"),
        ColoringKind::DistanceTwo,
        PaletteSearch::Fixed { colors: 3 },
        PassiveMode::General,
        &mut Computation::new(cancellation.clone(), &mut report),
        &mut |_| {},
    );
    // LLM: Assert that interruption propagates as cancellation and does not produce a
    // LLM: completed mathematical result.
    assert!(result.is_err());
    assert!(phases.contains(&crate::computation::Phase::Solving));
    assert!(cancellation.is_cancelled());
}

/// LLM: Regression test: verify that distance two maximum streams bounds for consecutive
/// LLM: palettes. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn distance_two_maximum_streams_bounds_for_consecutive_palettes() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("O I", "O I");
    let mut updates = Vec::new();
    let mut progress = |_| {};
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check_with(
        &p,
        ColoringKind::DistanceTwo,
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
        vec![3, 4]
    );
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for bounds in updates {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        if let Some(witness) = bounds.witness {
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
    // LLM: Prepare `ColoringResult` for the following computation; it stores the coloring
    // LLM: result for this stage.
    let ColoringResult::Maximum { colors, witness } = check(
        &plain("A B C", "A A\nB B\nC C"),
        ColoringKind::DistanceTwo,
        PaletteSearch::Maximum,
        PassiveMode::General,
    )
    .unwrap() else {
        panic!("expected finite edge-coloring bound");
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(colors, 4);
    assert!(verify_witness(
        &plain("A B C", "A A\nB B\nC C"),
        ColoringKind::DistanceTwo,
        &witness,
        false
    ));
    // LLM: Prepare `impossible` for the following computation; it stores the impossible for
    // LLM: this stage.
    let impossible = problem(
        serde_json::json!({"kind":"plain", "constraints":{"active":"A^2","passive":""}, "degrees":{"active":"2","passive":"2"}}),
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
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

/// LLM: Regression test: verify that order certificates work for all entry forms and
/// LLM: nonregular degrees. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn order_certificates_work_for_all_entry_forms_and_nonregular_degrees() {
    // LLM: Prepare `fixtures` for the following computation; it stores the fixtures for this
    // LLM: stage.
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
    // LLM: These passive rectangles are all manually maximized. There is no
    // LLM: maximization step or model flag in either analysis.

    for p in &fixtures {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for mode in [PassiveMode::General, PassiveMode::Maximized] {
            // LLM: Prepare `ColoringResult` for the following computation; it stores the
            // LLM: coloring result for this stage.
            let ColoringResult::Unbounded { minimum, witness } =
                check(p, ColoringKind::DistanceTwo, PaletteSearch::Maximum, mode).unwrap()
            else {
                panic!("expected unbounded: {p:?}");
            };
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                minimum,
                prepare::minimum(p, ColoringKind::DistanceTwo).unwrap()
            );
            assert!(matches!(witness, ColoringWitness::ColorOrder { .. }));
            assert!(verify_witness(p, ColoringKind::DistanceTwo, &witness, true));
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(!verify_witness(
                p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
            assert!(!verify_witness(p, ColoringKind::Vertex, &witness, true));
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for colors in minimum..=minimum + 2 {
                assert!(fixed(p, ColoringKind::DistanceTwo, colors));
            }
        }
    }
    // LLM: Orienting from smaller to larger needs comparisons; there is no
    // LLM: color-independent zero-round strategy for this problem.

    assert!(matches!(
        zero_round::check(&fixtures[0], PassiveMode::General).unwrap(),
        ZeroRoundResult::Unsolvable
    ));
    // LLM: Prepare `broken` for the following computation; it stores the broken for this
    // LLM: stage.
    let mut broken = check(
        &fixtures[4],
        ColoringKind::DistanceTwo,
        PaletteSearch::Maximum,
        PassiveMode::General,
    )
    .unwrap();
    // LLM: Prepare `ColoringResult` for the following computation; it stores the coloring
    // LLM: result for this stage.
    let ColoringResult::Unbounded {
        witness: ColoringWitness::ColorOrder { smaller, larger },
        ..
    } = &mut broken
    else {
        panic!("expected order certificate");
    };
    smaller.outputs_by_input[0] = LabelSet::new(vec![LabelId(2)]); // LLM: C is forbidden for a.
    let witness = ColoringWitness::ColorOrder {
        smaller: smaller.clone(),
        larger: larger.clone(),
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_witness(
        &fixtures[4],
        ColoringKind::DistanceTwo,
        &witness,
        true
    ));
}

/// LLM: Regression test: verify that order encoding agrees with exhaustive output set tables.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn order_encoding_agrees_with_exhaustive_output_set_tables() {
    // LLM: Independent oracle: all two-label output sets, all four comparison
    // LLM: patterns at degree 2, and every smaller/larger pair across an edge.
    // LLM: Neither the sign-input reduction nor the generic witness checker is used.

    let configurations = ["O^2", "O I", "I^2"];
    // LLM: Validate the returned color behavior tables with the independent certificate
    // LLM: checker.
    for active in 0u8..8 {
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        for passive in 0u8..8 {
            // LLM: Prepare `allowed` for the following computation; it stores the allowed for
            // LLM: this stage.
            let allowed = |mask: u8| {
                // LLM: Update the current working state before continuing this operation.
                configurations
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, line)| *line)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            // LLM: Prepare `p` for the following computation; it stores the p for this stage.
            let p = problem(
                serde_json::json!({"kind":"plain", "constraints":{"active":allowed(active),"passive":allowed(passive)}, "degrees":{"active":"2","passive":"2"}}),
            );
            // LLM: Construct the independent reference result for this fixture.
            let expected = (0u8..4).any(|smaller| {
                // LLM: Update the current working state before continuing this operation.
                (0u8..4).any(|larger| {
                    // LLM: Prepare `available` for the following computation; it stores the
                    // LLM: available for this stage.
                    let available = |set: u8| (0u8..2).filter(move |label| set & (1 << label) != 0);
                    let active_valid = [smaller, larger].iter().all(|&first| {
                        [smaller, larger].iter().all(|&second| {
                            available(first).any(|left| {
                                available(second).any(|right| active & (1 << (left + right)) != 0)
                            })
                        })
                    });
                    // LLM: Update the current working state before continuing this operation.
                    active_valid
                        && available(smaller).all(|left| {
                            available(larger).all(|right| passive & (1 << (left + right)) != 0)
                        })
                })
            });
            // LLM: Prepare `progress` for the following computation; it stores the progress
            // LLM: for this stage.
            let mut progress = |_| {};
            let witness = order::check_with(
                &p,
                PassiveMode::General,
                &mut Computation::new(Cancellation::default(), &mut progress),
            )
            .unwrap();
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(
                witness.is_some(),
                expected,
                "active={active} passive={passive}"
            );
            // LLM: Validate the returned color behavior tables with the independent
            // LLM: certificate checker.
            if let Some(witness) = witness {
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert!(verify_witness(
                    &p,
                    ColoringKind::DistanceTwo,
                    &witness,
                    true
                ));
                // LLM: Visit the color placements required by the auxiliary promise while
                // LLM: retaining original input context.
                for colors in [3, 4, 5] {
                    assert!(fixed(&p, ColoringKind::DistanceTwo, colors));
                }
            }
        }
    }
}

/// LLM: Regression test: verify that distance two maximum cancellation retains the last proved
/// LLM: strategy. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn distance_two_maximum_cancellation_retains_the_last_proved_strategy() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("O I", "O I");
    let cancellation = Cancellation::default();
    let mut progress = |_| {};
    let mut last = None;
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
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
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(result.is_err());
    let bounds = last.unwrap();
    assert_eq!(bounds.lower, Some(3));
    assert_eq!(bounds.upper, None);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(verify_witness(
        &p,
        ColoringKind::DistanceTwo,
        &bounds.witness.unwrap(),
        false
    ));
}

/// LLM: Regression test: verify that incremental neighbor solver can disable and reenable
/// LLM: colors. Used by the test suite to protect this behavior while implementations change.
#[test]
fn incremental_neighbor_solver_can_disable_and_reenable_colors() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = plain("O I", "O I");
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    // LLM: Prepare `solver` for the following computation; it stores the solver for this
    // LLM: stage.
    let mut solver = distance_two::PaletteSolver::new(
        &p,
        crate::algorithms::sat::backend(&computation.cancellation),
        &mut computation,
    )
    .unwrap();
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    for colors in [3, 4, 3, 5, 3] {
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        let witness = solver
            .solve(colors, PassiveMode::General, &mut computation)
            .unwrap();
        assert_eq!(witness.is_some(), colors == 3);
        // LLM: Validate the returned color behavior tables with the independent certificate
        // LLM: checker.
        if let Some(witness) = witness {
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert!(verify_witness(
                &p,
                ColoringKind::DistanceTwo,
                &witness,
                false
            ));
        }
    }
}
