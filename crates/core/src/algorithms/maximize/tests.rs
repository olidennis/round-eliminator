use super::*;
use crate::{
    algorithms::{
        configurations::{accepts, concrete_multisets, contains_condensed_with},
        diagram::{
            self, DiagramKind, DiagramOptions, InputOutputDiagramMethod, OutputDiagramMethod,
        },
    },
    computation::{Cancelled, Phase},
    model::{Configuration, LabelId},
    parser::{ProblemText, parse_problem},
};

/// LLM: Create a fresh computation context with the supplied test progress consumer and
/// LLM: cancellation flag. Used by algorithm fixtures to observe phases and interruption.
fn context<'a>(report: &'a mut dyn FnMut(crate::computation::Progress)) -> Computation<'a> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Computation::new(Cancellation::default(), report)
}
/// LLM: Construct a condensed test family from label domains and their multiplicities. Used by
/// LLM: small coverage, maximization, or representation fixtures.
fn family(parts: &[u8]) -> CondensedConfiguration<u8> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    CondensedConfiguration::new(
        parts
            .iter()
            .map(|mask| {
                Part::new(
                    LabelSet::new((0..3).filter(|i| mask & (1 << i) != 0).collect()),
                    1,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
/// LLM: Decide concrete membership of the fixture family using its independent small oracle.
/// LLM: Used to check universal maxima against enumeration.
fn contains(c: &Constraint<u8>, f: &CondensedConfiguration<u8>) -> bool {
    // LLM: Try a single-row containment proof using counted subset-compatible matching.
    c.configurations_at_degree(f.degree())
        .iter()
        .any(|target| contains_condensed_with(target, f, &Cancellation::default()).unwrap())
}

/// LLM: Regression test: verify that exhaustive universal maxima agree with direct
/// LLM: enumeration. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn exhaustive_universal_maxima_agree_with_direct_enumeration() {
    // LLM: Independent oracle: list all concrete languages on three labels, degree 2.
    // LLM: Enumerate every rectangle, test all choices, then require coverage and
    // LLM: single-label extension maximality. No closure operations are used here.

    let concrete = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for mask in 0u8..64 {
        // LLM: Prepare `initial` for the following computation; it stores the initial for this
        // LLM: stage.
        let initial = Constraint::new(
            concrete
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, &(a, b))| family(&[1 << a, 1 << b]))
                .collect(),
        );
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
        for a in 1..8 {
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for b in 1..8 {
                // LLM: Prepare `candidate` for the following computation; it stores the
                // LLM: candidate for this stage.
                let candidate = family(&[a, b]);
                let valid = concrete_multisets(&Constraint::new(vec![candidate.clone()]), &[2])
                    .iter()
                    .all(|c| accepts(&initial, c));
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    contains(&result, &candidate),
                    valid,
                    "language {mask}, parts {a}/{b}"
                );
            }
        }
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for f in result.configurations() {
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for index in 0..f.parts().len() {
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for label in 0..3 {
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if f.parts()[index].labels().contains(label) {
                        continue;
                    }
                    let mut parts = f.parts().to_vec();
                    // LLM: Enlarge one occurrence, including within a repeated part.

                    let part = parts.remove(index);
                    if part.multiplicity() > 1 {
                        parts.push(
                            Part::new(part.labels().clone(), part.multiplicity() - 1).unwrap(),
                        );
                    }
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    parts.push(
                        Part::new(part.labels().union(&LabelSet::new(vec![label])), 1).unwrap(),
                    );
                    let extension = CondensedConfiguration::new(parts).unwrap();
                    // LLM: Check the expected result and invariants for this fixture,
                    // LLM: including the relevant edge case.
                    assert!(
                        concrete_multisets(&Constraint::new(vec![extension]), &[2])
                            .iter()
                            .any(|c| !accepts(&initial, c))
                    );
                }
            }
        }
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            maximize_constraint_with(&result, &mut context(&mut |_| {})).unwrap(),
            result
        );
    }
}

/// LLM: Regression test: verify that self combinations repetitions nonregular and zero degree.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn self_combinations_repetitions_nonregular_and_zero_degree() {
    let seed = family(&[3, 3, 2]); // LLM: AB AB B must combine with itself to grow a part.
    let initial = Constraint::new(vec![seed.clone(), family(&[1]), family(&[2]), family(&[])]);
    let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    assert!(!contains(&result, &family(&[3, 3, 3]))); // LLM: AAA is forbidden.
    assert!(contains(&result, &family(&[3, 3, 2])));
    assert!(contains(&result, &family(&[3])));
    assert_eq!(result.configurations_at_degree(0).len(), 1);
    // LLM: Prepare `large` for the following computation; it stores the large for this stage.
    let large = Constraint::new(vec![
        CondensedConfiguration::new(vec![
            Part::new(LabelSet::new(vec![0u8, 1]), u32::MAX).unwrap(),
        ])
        .unwrap(),
    ]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        maximize_constraint_with(&large, &mut context(&mut |_| {})).unwrap(),
        large
    );
}

/// LLM: Regression test: verify that degree three condensed constraints match universal
/// LLM: coverage. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn degree_three_condensed_constraints_match_universal_coverage() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for a in 1..4 {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for b in 1..4 {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for c in 1..4 {
                // LLM: Prepare `initial` for the following computation; it stores the initial
                // LLM: for this stage.
                let initial = Constraint::new(vec![family(&[a, b, c]), family(&[2, 2, 2])]);
                let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for x in 1..4 {
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for y in 1..4 {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for z in 1..4 {
                            // LLM: Prepare `candidate` for the following computation; it
                            // LLM: stores the candidate for this stage.
                            let candidate = family(&[x, y, z]);
                            let valid =
                                concrete_multisets(&Constraint::new(vec![candidate.clone()]), &[3])
                                    .iter()
                                    .all(|v| accepts(&initial, v));
                            assert_eq!(contains(&result, &candidate), valid);
                        }
                    }
                }
            }
        }
    }
}

/// LLM: Regression test: verify that maximum must cover rectangles by uniform domination. Used
/// LLM: by the test suite to protect this behavior while implementations change.
#[test]
fn maximum_must_cover_rectangles_by_uniform_domination() {
    // LLM: Every concrete output of A BC² is already admitted by AB AC BC, but its
    // LLM: two BC parts cannot both match the original constraint's single BC part.
    // LLM: Semantic-language pruning would incorrectly omit this necessary rectangle.

    let initial = Constraint::new(vec![family(&[3, 5, 6])]);
    let candidate = family(&[1, 6, 6]);
    assert!(
        concrete_multisets(&Constraint::new(vec![candidate.clone()]), &[3])
            .iter()
            .all(|v| accepts(&initial, v))
    );
    assert!(!contains(&initial, &candidate));
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    assert!(contains(&maximum, &candidate));
}

/// LLM: Regression test: verify that custom lattice empty values and derivations. Used by the
/// LLM: test suite to protect this behavior while implementations change.
#[test]
fn custom_lattice_empty_values_and_derivations() {
    /// LLM: A small bit-set algebra used as an independent test/benchmark value domain. Join
    /// LLM: is bitwise OR, meet is AND, and containment tests inclusion of set bits.
    struct Bits;
    impl Operations<u8> for Bits {
        /// LLM: Compute the join in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn union(&self, a: &u8, b: &u8) -> u8 {
            // LLM: Apply the representation's set/order operation, preserving the algebra used
            // LLM: by containment and closure.
            a | b
        }
        /// LLM: Compute the meet in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn intersection(&self, a: &u8, b: &u8) -> u8 {
            // LLM: Apply the representation's set/order operation, preserving the algebra used
            // LLM: by containment and closure.
            a & b
        }
        /// LLM: Test domination in this representation's chosen standard or dual order. Called
        /// LLM: by the generic pairwise closure and its pruning proofs.
        fn is_superset(&self, a: &u8, b: &u8) -> bool {
            // LLM: Apply the representation's set/order operation, preserving the algebra used
            // LLM: by containment and closure.
            a & b == *b
        }
    }
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let initial = [Configuration::new(vec![(1, 1), (2, 1)]).unwrap()];
    let mut derivations = 0;
    // LLM: Compute exact closure under the selected standard or dual algebra.
    let result = maximize_custom_with(
        &initial,
        &Bits,
        &mut context(&mut |_| {}),
        Some(&mut |cfg, recipe| {
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(recipe.left.degree(), cfg.degree());
            assert_eq!(recipe.right.degree(), cfg.degree());
            assert_eq!(
                recipe.intersection_counts.iter().sum::<u32>(),
                cfg.degree() - 1
            );
            let a = recipe.left.multiplicities();
            let b = recipe.right.multiplicities();
            // LLM: Update the current working state before continuing this operation.
            let (i, j) = recipe.union_parts;
            let mut reconstructed = vec![(a[i].0 | b[j].0, 1)];
            for (cell, &count) in recipe.intersection_counts.iter().enumerate() {
                if count > 0 {
                    reconstructed.push((a[cell / b.len()].0 & b[cell % b.len()].0, count));
                }
            }
            assert_eq!(&Configuration::new(reconstructed).unwrap(), cfg);
            // LLM: Update the current working state before continuing this operation.
            derivations += 1;
        }),
    )
    .unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(derivations > 0);
    assert!(
        result
            .iter()
            .any(|cfg| cfg.multiplicities().contains(&(0, 1)))
    );
    // LLM: Prepare `with_empty` for the following computation; it stores the with empty for
    // LLM: this stage.
    let with_empty = [Configuration::new(vec![
        (LabelSet::<u8>::default(), 1),
        (LabelSet::new(vec![1]), 1),
    ])
    .unwrap()];
    // LLM: Compute exact closure under the selected standard or dual algebra.
    let allow = maximize_custom_with(
        &with_empty,
        &SetOperations { allow_empty: true },
        &mut context(&mut |_| {}),
        None,
    )
    .unwrap();
    assert!(!allow.is_empty());
    // LLM: Compute exact closure under the selected standard or dual algebra.
    let reject = maximize_custom_with(
        &with_empty,
        &SetOperations { allow_empty: false },
        &mut context(&mut |_| {}),
        None,
    )
    .unwrap();
    assert!(reject.is_empty());
}

/// LLM: Regression test: verify that cancelled work never returns a maximum. Used by the test
/// LLM: suite to protect this behavior while implementations change.
#[test]
fn cancelled_work_never_returns_a_maximum() {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut report = move |p: crate::computation::Progress| {
        if p.phase == Phase::Maximizing {
            cancel.cancel();
        }
    };
    let mut computation = Computation::new(cancellation, &mut report);
    // LLM: Prepare `err` for the following computation; it stores the err for this stage.
    let err = maximize_constraint_with(
        &Constraint::new(vec![family(&[1]), family(&[2])]),
        &mut computation,
    )
    .unwrap_err();
    assert!(err.downcast_ref::<Cancelled>().is_some());
}

/// LLM: Parse a JSON problem definition for the independent algorithm fixtures. Used by tests
/// LLM: to exercise the same entry conversion as the UI.
fn problem(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse_problem(serde_json::from_value::<ProblemText>(value).unwrap()).unwrap()
}

/// LLM: Regression test: verify that joint maxima preserve mapping and fast input aware
/// LLM: arrows. Used by the test suite to protect this behavior while implementations change.
#[test]
fn joint_maxima_preserve_mapping_and_fast_input_aware_arrows() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"ij k", "passive":"ij k"},
        "output":{"active":"A B C", "passive":"A C\nB C"}, "mapping":"i -> A\nj -> AB\nk -> C"}),
    );
    let before = p.clone();
    let MaximizationResult::InputOutput(maximum) = compute(&p).unwrap() else {
        panic!()
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(p, before);
    // LLM: Compute membership independently from input/output projections.

    let original = &p;
    // LLM: Prepare `labels` for the following computation; it stores the labels for this
    // LLM: stage.
    let labels: Vec<_> = (0..3)
        .flat_map(|input| {
            // LLM: Update the current working state before continuing this operation.
            (0..3).filter_map(move |output| {
                // LLM: Update the current working state before continuing this operation.
                original
                    .output_allowed(Some(LabelId(input)), LabelId(output))
                    .then_some(InputOutputLabel {
                        input: Some(LabelId(input)),
                        output: LabelId(output),
                    })
            })
        })
        .collect();
    // LLM: Retain the original passive promise as condensed input families.
    for &a in &labels {
        // LLM: Retain the original passive promise as condensed input families.
        for &b in &labels {
            // LLM: Prepare `v` for the following computation; it stores the v for this stage.
            let v = Configuration::from_occurrences([a, b]).unwrap();
            let projected = Configuration::from_occurrences([a.output, b.output]).unwrap();
            let allowed_inputs =
                Constraint::new(prepare_passive_inputs(&p, &Cancellation::default()).unwrap());
            let ins = Configuration::from_occurrences([a.input, b.input]).unwrap();
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                accepts(&maximum, &v),
                accepts(&allowed_inputs, &ins) && accepts(p.output().passive(), &projected)
            );
        }
    }
    // LLM: Prepare `exact` for the following computation; it stores the exact for this stage.
    let exact = diagram::compute(
        &p,
        DiagramOptions {
            kind: DiagramKind::InputOutput,
            output: OutputDiagramMethod::Exact,
            input_output: InputOutputDiagramMethod::Exact,
        },
    )
    .unwrap();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for (a, source) in exact.diagram.nodes.iter().enumerate() {
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for (b, target) in exact
            .diagram
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.input == source.input)
        {
            // LLM: Prepare `occurrence` for the following computation; it stores the
            // LLM: occurrence for this stage.
            let occurrence = maximum
                .configurations()
                .flat_map(|f| f.parts())
                .all(|part| !part.labels().contains(*source) || part.labels().contains(*target));
            assert_eq!(
                occurrence,
                exact.diagram.successors[a].contains(&(b as u32))
            );
        }
    }
}

/// LLM: Regression test: verify that unrestricted inputs do not expand and impossible inputs
/// LLM: remain promised. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn unrestricted_inputs_do_not_expand_and_impossible_inputs_remain_promised() {
    // LLM: Prepare `independent` for the following computation; it stores the independent for
    // LLM: this stage.
    let independent = problem(
        serde_json::json!({"kind":"independent", "input":{"active":"abcdefghijklmnopqrst^100", "passive":"abcdefghijklmnopqrst^100"}, "output":{"active":"A^100","passive":"A^100"}}),
    );
    assert!(matches!(
        compute(&independent).unwrap(),
        MaximizationResult::OutputOnly(_)
    ));
    // LLM: Prepare `impossible` for the following computation; it stores the impossible for
    // LLM: this stage.
    let impossible = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"i","passive":"i^2"},"output":{"active":"A","passive":"B^2"},"mapping":"i -> A"}),
    );
    let MaximizationResult::InputOutput(result) = compute(&impossible).unwrap() else {
        panic!()
    };
    assert!(result.configurations().next().is_none());
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(matches!(
        impossible.input(),
        crate::model::Input::Explicit(_)
    ));
}

/// LLM: Regression test: verify that exhaustive joint maxima cover exactly the condensed input
/// LLM: output relation. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn exhaustive_joint_maxima_cover_exactly_the_condensed_input_output_relation() {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use crate::model::{ConstraintPair, GraphClass, Input, OutputMapping};
    let make = |mask: u8| {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Constraint::new(
            [(0, 0), (0, 1), (1, 1)]
                .iter()
                .enumerate()
                .filter(|(k, _)| mask & (1 << k) != 0)
                .map(|(_, &(a, b))| {
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![LabelId(a)]), 1).unwrap(),
                        Part::new(LabelSet::new(vec![LabelId(b)]), 1).unwrap(),
                    ])
                    .unwrap()
                })
                .collect(),
        )
    };
    // LLM: Try a single-row containment proof using counted subset-compatible matching.
    for input_mask in 1..8 {
        // LLM: Try a single-row containment proof using counted subset-compatible matching.
        for output_mask in 1..8 {
            // LLM: Try a single-row containment proof using counted subset-compatible
            // LLM: matching.
            for a in 1u8..4 {
                // LLM: Try a single-row containment proof using counted subset-compatible
                // LLM: matching.
                for b in 1u8..4 {
                    // LLM: Build readable names in the same order as the corresponding stored
                    // LLM: IDs.
                    let names = vec!["0".into(), "1".into()];
                    let input =
                        ConstraintPair::new(names.clone(), make(7), make(input_mask)).unwrap();
                    let output = ConstraintPair::new(names, make(7), make(output_mask)).unwrap();
                    // LLM: Preserve the incidence permission relation while constructing the
                    // LLM: transformed problem.
                    let mapping = [a, b].map(|mask| {
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
                        LabelSet::new(
                            (0..2)
                                .filter(|k| mask & (1 << k) != 0)
                                .map(LabelId)
                                .collect(),
                        )
                    });
                    // LLM: Prepare `p` for the following computation; it stores the p for this
                    // LLM: stage.
                    let p = Problem::new(
                        GraphClass::new(vec![2], vec![2]),
                        Input::Explicit(input),
                        output,
                        OutputMapping::Restricted(mapping.to_vec()),
                    )
                    .unwrap();
                    // LLM: Prepare `MaximizationResult` for the following computation; it
                    // LLM: stores the maximization result for this stage.
                    let MaximizationResult::InputOutput(maximum) = compute(&p).unwrap() else {
                        panic!()
                    };
                    let mut alphabet = Vec::new();
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for i in 0..2 {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for o in 0..2 {
                            // LLM: Handle this condition before continuing with the remaining
                            // LLM: search alternatives.
                            if p.output_allowed(Some(LabelId(i)), LabelId(o)) {
                                alphabet.push(InputOutputLabel {
                                    input: Some(LabelId(i)),
                                    output: LabelId(o),
                                });
                            }
                        }
                    }
                    // LLM: Retain or prepare the original promised input semantics for this
                    // LLM: stage.
                    let input = make(input_mask);
                    for x in 1..(1 << alphabet.len()) {
                        // LLM: Try a single-row containment proof using counted
                        // LLM: subset-compatible matching.
                        for y in x..(1 << alphabet.len()) {
                            // LLM: Prepare `choices` for the following computation; it stores
                            // LLM: the choices for this stage.
                            let choices = |mask: usize| {
                                // LLM: Delegate to the common implementation so this entry
                                // LLM: point shares its semantics and checks.
                                LabelSet::new(
                                    alphabet
                                        .iter()
                                        .enumerate()
                                        .filter(|(k, _)| mask & (1 << k) != 0)
                                        .map(|(_, p)| *p)
                                        .collect(),
                                )
                            };
                            // LLM: Prepare `candidate` for the following computation; it
                            // LLM: stores the candidate for this stage.
                            let candidate = CondensedConfiguration::new(vec![
                                Part::new(choices(x), 1).unwrap(),
                                Part::new(choices(y), 1).unwrap(),
                            ])
                            .unwrap();
                            // LLM: Construct the independent reference result for this
                            // LLM: fixture.
                            let expected = candidate.parts()[0].labels().iter().all(|a| {
                                // LLM: Update the current working state before continuing this
                                // LLM: operation.
                                candidate.parts()[1].labels().iter().all(|b| {
                                    // LLM: Delegate to the common implementation so this entry
                                    // LLM: point shares its semantics and checks.
                                    accepts(
                                        &input,
                                        &Configuration::from_occurrences([
                                            a.input.unwrap(),
                                            b.input.unwrap(),
                                        ])
                                        .unwrap(),
                                    ) && accepts(
                                        p.output().passive(),
                                        &Configuration::from_occurrences([a.output, b.output])
                                            .unwrap(),
                                    )
                                })
                            });
                            // LLM: Try a single-row containment proof using counted
                            // LLM: subset-compatible matching.
                            let covered = maximum.configurations().any(|f| {
                                contains_condensed_with(f, &candidate, &Cancellation::default())
                                    .unwrap()
                            });
                            assert_eq!(
                                covered, expected,
                                "input {input_mask}, output {output_mask}, mappings {a}/{b}, choices {x}/{y}"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// LLM: Regression test: verify that restricted high degree inputs remain condensed. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn restricted_high_degree_inputs_remain_condensed() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = problem(serde_json::json!({"kind":"mapped",
        "input":{"active":"ij^100000", "passive":"ij^100000"},
        "output":{"active":"A^100000","passive":"A^100000"},"mapping":"i -> A\nj -> A"}));
    let MaximizationResult::InputOutput(result) = compute(&p).unwrap() else {
        panic!()
    };
    let configurations = result.configurations_at_degree(100000);
    assert_eq!(configurations.len(), 1);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(configurations[0].parts().len(), 1);
    assert_eq!(configurations[0].parts()[0].labels().len(), 2);
    assert_eq!(configurations[0].parts()[0].multiplicity(), 100000);
}

/// LLM: Regression test: verify that native parallel and single worker results are identical.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn native_parallel_and_single_worker_results_are_identical() {
    // LLM: Prepare `initial` for the following computation; it stores the initial for this
    // LLM: stage.
    let initial = Constraint::new(vec![family(&[1, 2, 3, 7]), family(&[1, 4, 5, 7])]);
    let parallel = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let serial =
        pool.install(|| maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap());
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(parallel, serial);
}
