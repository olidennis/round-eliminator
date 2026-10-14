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

fn context<'a>(report: &'a mut dyn FnMut(crate::computation::Progress)) -> Computation<'a> {
    Computation::new(Cancellation::default(), report)
}
fn family(parts: &[u8]) -> CondensedConfiguration<u8> {
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
fn contains(c: &Constraint<u8>, f: &CondensedConfiguration<u8>) -> bool {
    c.configurations_at_degree(f.degree())
        .iter()
        .any(|target| contains_condensed_with(target, f, &Cancellation::default()).unwrap())
}

#[test]
fn exhaustive_universal_maxima_agree_with_direct_enumeration() {
    // Independent oracle: list all concrete languages on three labels, degree 2.
    // Enumerate every rectangle, test all choices, then require coverage and
    // single-label extension maximality. No closure operations are used here.
    let concrete = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    for mask in 0u8..64 {
        let initial = Constraint::new(
            concrete
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, &(a, b))| family(&[1 << a, 1 << b]))
                .collect(),
        );
        let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
        for a in 1..8 {
            for b in 1..8 {
                let candidate = family(&[a, b]);
                let valid = concrete_multisets(&Constraint::new(vec![candidate.clone()]), &[2])
                    .iter()
                    .all(|c| accepts(&initial, c));
                assert_eq!(
                    contains(&result, &candidate),
                    valid,
                    "language {mask}, parts {a}/{b}"
                );
            }
        }
        for f in result.configurations() {
            for index in 0..f.parts().len() {
                for label in 0..3 {
                    if f.parts()[index].labels().contains(label) {
                        continue;
                    }
                    let mut parts = f.parts().to_vec();
                    // Enlarge one occurrence, including within a repeated part.
                    let part = parts.remove(index);
                    if part.multiplicity() > 1 {
                        parts.push(
                            Part::new(part.labels().clone(), part.multiplicity() - 1).unwrap(),
                        );
                    }
                    parts.push(
                        Part::new(part.labels().union(&LabelSet::new(vec![label])), 1).unwrap(),
                    );
                    let extension = CondensedConfiguration::new(parts).unwrap();
                    assert!(
                        concrete_multisets(&Constraint::new(vec![extension]), &[2])
                            .iter()
                            .any(|c| !accepts(&initial, c))
                    );
                }
            }
        }
        assert_eq!(
            maximize_constraint_with(&result, &mut context(&mut |_| {})).unwrap(),
            result
        );
    }
}

#[test]
fn self_combinations_repetitions_nonregular_and_zero_degree() {
    let seed = family(&[3, 3, 2]); // AB AB B must combine with itself to grow a part.
    let initial = Constraint::new(vec![seed.clone(), family(&[1]), family(&[2]), family(&[])]);
    let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    assert!(!contains(&result, &family(&[3, 3, 3]))); // AAA is forbidden.
    assert!(contains(&result, &family(&[3, 3, 2])));
    assert!(contains(&result, &family(&[3])));
    assert_eq!(result.configurations_at_degree(0).len(), 1);
    let large = Constraint::new(vec![
        CondensedConfiguration::new(vec![
            Part::new(LabelSet::new(vec![0u8, 1]), u32::MAX).unwrap(),
        ])
        .unwrap(),
    ]);
    assert_eq!(
        maximize_constraint_with(&large, &mut context(&mut |_| {})).unwrap(),
        large
    );
}

#[test]
fn degree_three_condensed_constraints_match_universal_coverage() {
    for a in 1..4 {
        for b in 1..4 {
            for c in 1..4 {
                let initial = Constraint::new(vec![family(&[a, b, c]), family(&[2, 2, 2])]);
                let result = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
                for x in 1..4 {
                    for y in 1..4 {
                        for z in 1..4 {
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

#[test]
fn maximum_must_cover_rectangles_by_uniform_domination() {
    // Every concrete output of A BC² is already admitted by AB AC BC, but its
    // two BC parts cannot both match the original constraint's single BC part.
    // Semantic-language pruning would incorrectly omit this necessary rectangle.
    let initial = Constraint::new(vec![family(&[3, 5, 6])]);
    let candidate = family(&[1, 6, 6]);
    assert!(
        concrete_multisets(&Constraint::new(vec![candidate.clone()]), &[3])
            .iter()
            .all(|v| accepts(&initial, v))
    );
    assert!(!contains(&initial, &candidate));
    let maximum = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    assert!(contains(&maximum, &candidate));
}

#[test]
fn custom_lattice_empty_values_and_derivations() {
    struct Bits;
    impl Operations<u8> for Bits {
        fn union(&self, a: &u8, b: &u8) -> u8 {
            a | b
        }
        fn intersection(&self, a: &u8, b: &u8) -> u8 {
            a & b
        }
        fn is_superset(&self, a: &u8, b: &u8) -> bool {
            a & b == *b
        }
    }
    let initial = [Configuration::new(vec![(1, 1), (2, 1)]).unwrap()];
    let mut derivations = 0;
    let result = maximize_custom_with(
        &initial,
        &Bits,
        &mut context(&mut |_| {}),
        Some(&mut |cfg, recipe| {
            assert_eq!(recipe.left.degree(), cfg.degree());
            assert_eq!(recipe.right.degree(), cfg.degree());
            assert_eq!(
                recipe.intersection_counts.iter().sum::<u32>(),
                cfg.degree() - 1
            );
            let a = recipe.left.multiplicities();
            let b = recipe.right.multiplicities();
            let (i, j) = recipe.union_parts;
            let mut reconstructed = vec![(a[i].0 | b[j].0, 1)];
            for (cell, &count) in recipe.intersection_counts.iter().enumerate() {
                if count > 0 {
                    reconstructed.push((a[cell / b.len()].0 & b[cell % b.len()].0, count));
                }
            }
            assert_eq!(&Configuration::new(reconstructed).unwrap(), cfg);
            derivations += 1;
        }),
    )
    .unwrap();
    assert!(derivations > 0);
    assert!(
        result
            .iter()
            .any(|cfg| cfg.multiplicities().contains(&(0, 1)))
    );
    let with_empty = [Configuration::new(vec![
        (LabelSet::<u8>::default(), 1),
        (LabelSet::new(vec![1]), 1),
    ])
    .unwrap()];
    let allow = maximize_custom_with(
        &with_empty,
        &SetOperations { allow_empty: true },
        &mut context(&mut |_| {}),
        None,
    )
    .unwrap();
    assert!(!allow.is_empty());
    let reject = maximize_custom_with(
        &with_empty,
        &SetOperations { allow_empty: false },
        &mut context(&mut |_| {}),
        None,
    )
    .unwrap();
    assert!(reject.is_empty());
}

#[test]
fn cancelled_work_never_returns_a_maximum() {
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut report = move |p: crate::computation::Progress| {
        if p.phase == Phase::Maximizing {
            cancel.cancel();
        }
    };
    let mut computation = Computation::new(cancellation, &mut report);
    let err = maximize_constraint_with(
        &Constraint::new(vec![family(&[1]), family(&[2])]),
        &mut computation,
    )
    .unwrap_err();
    assert!(err.downcast_ref::<Cancelled>().is_some());
}

fn problem(value: serde_json::Value) -> Problem {
    parse_problem(serde_json::from_value::<ProblemText>(value).unwrap()).unwrap()
}

#[test]
fn joint_maxima_preserve_mapping_and_fast_input_aware_arrows() {
    let p = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"ij k", "passive":"ij k"},
        "output":{"active":"A B C", "passive":"A C\nB C"}, "mapping":"i -> A\nj -> AB\nk -> C"}),
    );
    let before = p.clone();
    let MaximizationResult::InputOutput(maximum) = compute(&p).unwrap() else {
        panic!()
    };
    assert_eq!(p, before);
    // Compute membership independently from input/output projections.
    let original = &p;
    let labels: Vec<_> = (0..3)
        .flat_map(|input| {
            (0..3).filter_map(move |output| {
                original
                    .output_allowed(Some(LabelId(input)), LabelId(output))
                    .then_some(InputOutputLabel {
                        input: Some(LabelId(input)),
                        output: LabelId(output),
                    })
            })
        })
        .collect();
    for &a in &labels {
        for &b in &labels {
            let v = Configuration::from_occurrences([a, b]).unwrap();
            let projected = Configuration::from_occurrences([a.output, b.output]).unwrap();
            let allowed_inputs =
                Constraint::new(prepare_passive_inputs(&p, &Cancellation::default()).unwrap());
            let ins = Configuration::from_occurrences([a.input, b.input]).unwrap();
            assert_eq!(
                accepts(&maximum, &v),
                accepts(&allowed_inputs, &ins) && accepts(p.output().passive(), &projected)
            );
        }
    }
    let exact = diagram::compute(
        &p,
        DiagramOptions {
            kind: DiagramKind::InputOutput,
            output: OutputDiagramMethod::Exact,
            input_output: InputOutputDiagramMethod::Exact,
        },
    )
    .unwrap();
    for (a, source) in exact.diagram.nodes.iter().enumerate() {
        for (b, target) in exact
            .diagram
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.input == source.input)
        {
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

#[test]
fn unrestricted_inputs_do_not_expand_and_impossible_inputs_remain_promised() {
    let independent = problem(
        serde_json::json!({"kind":"independent", "input":{"active":"abcdefghijklmnopqrst^100", "passive":"abcdefghijklmnopqrst^100"}, "output":{"active":"A^100","passive":"A^100"}}),
    );
    assert!(matches!(
        compute(&independent).unwrap(),
        MaximizationResult::OutputOnly(_)
    ));
    let impossible = problem(
        serde_json::json!({"kind":"mapped", "input":{"active":"i","passive":"i^2"},"output":{"active":"A","passive":"B^2"},"mapping":"i -> A"}),
    );
    let MaximizationResult::InputOutput(result) = compute(&impossible).unwrap() else {
        panic!()
    };
    assert!(result.configurations().next().is_none());
    assert!(matches!(
        impossible.input(),
        crate::model::Input::Explicit(_)
    ));
}

#[test]
fn exhaustive_joint_maxima_cover_exactly_the_condensed_input_output_relation() {
    use crate::model::{ConstraintPair, GraphClass, Input, OutputMapping};
    let make = |mask: u8| {
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
    for input_mask in 1..8 {
        for output_mask in 1..8 {
            for a in 1u8..4 {
                for b in 1u8..4 {
                    let names = vec!["0".into(), "1".into()];
                    let input =
                        ConstraintPair::new(names.clone(), make(7), make(input_mask)).unwrap();
                    let output = ConstraintPair::new(names, make(7), make(output_mask)).unwrap();
                    let mapping = [a, b].map(|mask| {
                        LabelSet::new(
                            (0..2)
                                .filter(|k| mask & (1 << k) != 0)
                                .map(LabelId)
                                .collect(),
                        )
                    });
                    let p = Problem::new(
                        GraphClass::new(vec![2], vec![2]),
                        Input::Explicit(input),
                        output,
                        OutputMapping::Restricted(mapping.to_vec()),
                    )
                    .unwrap();
                    let MaximizationResult::InputOutput(maximum) = compute(&p).unwrap() else {
                        panic!()
                    };
                    let mut alphabet = Vec::new();
                    for i in 0..2 {
                        for o in 0..2 {
                            if p.output_allowed(Some(LabelId(i)), LabelId(o)) {
                                alphabet.push(InputOutputLabel {
                                    input: Some(LabelId(i)),
                                    output: LabelId(o),
                                });
                            }
                        }
                    }
                    let input = make(input_mask);
                    for x in 1..(1 << alphabet.len()) {
                        for y in x..(1 << alphabet.len()) {
                            let choices = |mask: usize| {
                                LabelSet::new(
                                    alphabet
                                        .iter()
                                        .enumerate()
                                        .filter(|(k, _)| mask & (1 << k) != 0)
                                        .map(|(_, p)| *p)
                                        .collect(),
                                )
                            };
                            let candidate = CondensedConfiguration::new(vec![
                                Part::new(choices(x), 1).unwrap(),
                                Part::new(choices(y), 1).unwrap(),
                            ])
                            .unwrap();
                            let expected = candidate.parts()[0].labels().iter().all(|a| {
                                candidate.parts()[1].labels().iter().all(|b| {
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

#[test]
fn restricted_high_degree_inputs_remain_condensed() {
    let p = problem(serde_json::json!({"kind":"mapped",
        "input":{"active":"ij^100000", "passive":"ij^100000"},
        "output":{"active":"A^100000","passive":"A^100000"},"mapping":"i -> A\nj -> A"}));
    let MaximizationResult::InputOutput(result) = compute(&p).unwrap() else {
        panic!()
    };
    let configurations = result.configurations_at_degree(100000);
    assert_eq!(configurations.len(), 1);
    assert_eq!(configurations[0].parts().len(), 1);
    assert_eq!(configurations[0].parts()[0].labels().len(), 2);
    assert_eq!(configurations[0].parts()[0].multiplicity(), 100000);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn native_parallel_and_single_worker_results_are_identical() {
    let initial = Constraint::new(vec![family(&[1, 2, 3, 7]), family(&[1, 4, 5, 7])]);
    let parallel = maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let serial =
        pool.install(|| maximize_constraint_with(&initial, &mut context(&mut |_| {})).unwrap());
    assert_eq!(parallel, serial);
}
