use super::*;
use crate::{
    algorithms::configurations::{accepts, concrete_multisets},
    model::{CondensedConfiguration, Configuration, ConstraintPair, GraphClass, LabelSet, Part},
};

fn parse(value: serde_json::Value) -> Problem {
    crate::parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}
fn plain(passive: &str) -> Problem {
    parse(serde_json::json!({"kind":"plain", "constraints":{"active":"A B", "passive":passive}}))
}
fn exact() -> DiagramOptions {
    DiagramOptions {
        kind: DiagramKind::OutputOnly,
        output: OutputDiagramMethod::Exact,
        input_output: InputOutputDiagramMethod::Exact,
    }
}

#[test]
fn each_request_computes_only_its_relation_and_required_dependencies() {
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j k", "passive":"i j\nj k"},
        "output":{"active":"A B X Y", "passive":"A X\nB X\nA Y"},
        "mapping":"i -> AB\nj -> X\nk -> Y"}));
    for kind in [
        DiagramKind::OutputOnly,
        DiagramKind::InputOutput,
        DiagramKind::InputAware,
    ] {
        let mut comparisons = Vec::new();
        let mut reductions = 0;
        let mut report = |progress: crate::computation::Progress| {
            if progress.phase == Phase::Comparing {
                comparisons.push(progress.iterations);
            }
            if progress.phase == Phase::Reducing {
                reductions += 1;
            }
        };
        let result = compute_with(
            &p,
            DiagramOptions { kind, ..exact() },
            &mut Computation::new(Cancellation::default(), &mut report),
        )
        .unwrap();
        // There are four output labels and four admitted pairs. A second
        // comparison phase would reveal an unrequested output-only computation.
        assert_eq!(comparisons, vec![0, 1, 2, 3, 4]);
        assert_eq!(reductions, 1);
        assert_eq!(result.options.kind, kind);
        assert_eq!(result.diagram.nodes.len(), 4);
    }
    // Exact input-output checks must not depend on the output-only method choice.
    for kind in [DiagramKind::InputOutput, DiagramKind::InputAware] {
        let exact = super::compute(&p, DiagramOptions { kind, ..exact() })
            .unwrap()
            .diagram;
        let partial = super::compute(
            &p,
            DiagramOptions {
                kind,
                output: OutputDiagramMethod::Partial,
                input_output: InputOutputDiagramMethod::Exact,
            },
        )
        .unwrap()
        .diagram;
        assert_eq!(exact, partial);
    }
}
// Tests comparing the three semantics issue three independent requests.
struct Diagrams {
    output_only: Diagram,
    input_output: Diagram,
    input_aware: Diagram,
}
fn compute_all(problem: &Problem, options: DiagramOptions) -> anyhow::Result<Diagrams> {
    let requested = |kind| {
        super::compute(problem, DiagramOptions { kind, ..options }).map(|result| result.diagram)
    };
    Ok(Diagrams {
        output_only: requested(DiagramKind::OutputOnly)?,
        input_output: requested(DiagramKind::InputOutput)?,
        input_aware: requested(DiagramKind::InputAware)?,
    })
}
fn edge(diagram: &Diagram, input: Option<LabelId>, a: LabelId, b: LabelId) -> bool {
    let source = diagram
        .nodes
        .iter()
        .position(|n| *n == InputOutputLabel { input, output: a })
        .unwrap();
    let Some(target) = diagram
        .nodes
        .iter()
        .position(|n| *n == InputOutputLabel { input, output: b })
    else {
        return false;
    };
    diagram.successors[source].contains(&(target as u32))
}

#[test]
fn exact_output_agrees_with_exhaustive_replacements_and_partial_is_sound() {
    // Every binary multiset language of degree three. Concrete fixtures also
    // expose arrows covered collectively by rows rather than one condensed row.
    for mask in 0u32..16 {
        let passive = Constraint::new(
            (0..4)
                .filter(|i| mask & (1 << i) != 0)
                .map(|a| {
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![LabelId(0)]), a).unwrap(),
                        Part::new(LabelSet::new(vec![LabelId(1)]), 3 - a).unwrap(),
                    ])
                    .unwrap()
                })
                .collect(),
        );
        let p = Problem::new(
            GraphClass::new(vec![2], vec![3]),
            Input::Trivial,
            ConstraintPair::new(
                vec!["A".into(), "B".into()],
                plain("A B").output().active().clone(),
                passive.clone(),
            )
            .unwrap(),
            OutputMapping::Unrestricted,
        )
        .unwrap();
        let actual = compute_all(&p, exact()).unwrap();
        let partial = compute_all(
            &p,
            DiagramOptions {
                output: OutputDiagramMethod::Partial,
                ..exact()
            },
        )
        .unwrap();
        for a in [LabelId(0), LabelId(1)] {
            for b in [LabelId(0), LabelId(1)] {
                let expected = concrete_multisets(&passive, &[3])
                    .iter()
                    .all(|configuration| {
                        if !configuration
                            .multiplicities()
                            .iter()
                            .any(|&(label, _)| label == a)
                        {
                            return true;
                        }
                        let changed = Configuration::new(
                            configuration
                                .multiplicities()
                                .iter()
                                .map(|&(label, count)| (label, count - u32::from(label == a)))
                                .chain(std::iter::once((b, 1)))
                                .collect(),
                        )
                        .unwrap();
                        accepts(&passive, &changed)
                    });
                assert_eq!(
                    edge(&actual.output_only, None, a, b),
                    expected,
                    "mask {mask}, {a:?}->{b:?}"
                );
                assert!(!edge(&partial.output_only, None, a, b) || expected);
            }
        }
    }
}

#[test]
fn maximal_occurrence_test_and_nonmaximal_exact_have_the_same_semantics() {
    let maximum = plain("AB^3");
    let expanded = plain("A^3\nA^2 B\nA B^2\nB^3");
    let full = compute_all(
        &maximum,
        DiagramOptions {
            output: OutputDiagramMethod::Maximized,
            ..exact()
        },
    )
    .unwrap();
    let result = compute_all(&expanded, exact()).unwrap();
    assert_eq!(full.output_only, result.output_only);
    let partial = compute_all(
        &expanded,
        DiagramOptions {
            output: OutputDiagramMethod::Partial,
            ..exact()
        },
    )
    .unwrap();
    assert!(!partial.output_only.complete);
    assert!(!edge(&partial.output_only, None, LabelId(0), LabelId(1)));
    assert_eq!(result.output_only.components, vec![vec![0, 1]]);
    assert!(result.output_only.edges.is_empty());
    for passive in ["A B\nB^2", "B\nA B\nB^2", "()\nA B\nB^2"] {
        let p = plain(passive);
        let result = compute_all(&p, exact()).unwrap();
        assert!(edge(&result.output_only, None, LabelId(0), LabelId(1)));
        if passive.starts_with("B\n") {
            assert!(!edge(&result.output_only, None, LabelId(1), LabelId(0)));
        }
    }
}

#[test]
fn exact_pairs_use_the_input_promise_and_input_aware_checks_missing_targets() {
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j k","passive":"i j\nj k"},
        "output":{"active":"A B X Y", "passive":"A X\nB X\nA Y"},
        "mapping":"i -> AB\nj -> X\nk -> Y"}));
    let result = compute_all(&p, exact()).unwrap();
    let id =
        |name: &str| LabelId(p.output().labels().iter().position(|s| s == name).unwrap() as u32);
    // A->B fails in the raw constraint because AY is allowed but BY is not.
    assert!(!edge(&result.output_only, None, id("A"), id("B")));
    // At i, only input j (output X) can be adjacent. The bad Y context is excluded.
    assert!(edge(
        &result.input_output,
        Some(LabelId(0)),
        id("A"),
        id("B")
    ));
    assert!(edge(&result.input_aware, None, id("A"), id("B")));
    let cheap = compute_all(&p, DiagramOptions::default()).unwrap();
    assert!(!edge(
        &cheap.input_output,
        Some(LabelId(0)),
        id("A"),
        id("B")
    ));
    assert!(!cheap.input_output.complete && result.input_output.complete);
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j", "passive":"ij^2"},
        "output":{"active":"A B", "passive":"AB^2"},
        "mapping":"i -> AB\nj -> A"}));
    let result = compute_all(&p, exact()).unwrap();
    assert!(edge(
        &result.input_output,
        Some(LabelId(0)),
        LabelId(0),
        LabelId(1)
    ));
    assert!(!edge(&result.input_aware, None, LabelId(0), LabelId(1)));
    assert!(result.input_output.edges.iter().all(|&(a, b)| {
        let a =
            result.input_output.nodes[result.input_output.components[a as usize][0] as usize].input;
        let b =
            result.input_output.nodes[result.input_output.components[b as usize][0] as usize].input;
        a == b
    }));
}

#[test]
fn graph_class_counts_trivial_input_and_cancellation() {
    let p = parse(
        serde_json::json!({"kind":"plain", "constraints":{"active":"A B","passive":"A\nAB^2"},
        "degrees":{"active":"2","passive":"2"}}),
    );
    let result = compute_all(&p, exact()).unwrap();
    assert_eq!(result.output_only, result.input_output);
    assert_eq!(result.output_only, result.input_aware);
    let p = plain("AB^4294967295");
    assert!(edge(
        &compute_all(&p, exact()).unwrap().output_only,
        None,
        LabelId(0),
        LabelId(1)
    ));
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut report = move |progress: crate::computation::Progress| {
        if progress.phase == Phase::Comparing {
            cancel.cancel()
        }
    };
    assert!(
        compute_with(
            &p,
            exact(),
            &mut Computation::new(cancellation.clone(), &mut report)
        )
        .is_err()
    );
    assert!(cancellation.is_cancelled());
}

#[test]
fn exact_joint_relations_agree_with_exhaustive_input_output_multisets() {
    use std::collections::BTreeSet;
    let constraint = |mask: u32| {
        Constraint::new(
            (0..4)
                .filter(|i| mask & (1 << i) != 0)
                .map(|a| {
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![LabelId(0)]), a).unwrap(),
                        Part::new(LabelSet::new(vec![LabelId(1)]), 3 - a).unwrap(),
                    ])
                    .unwrap()
                })
                .collect(),
        )
    };
    for case in 0..128u32 {
        let input = constraint(case % 16);
        let output = constraint((case * 7 + case / 16) % 16);
        let masks = [(case / 4) % 4, (case / 16) % 4];
        let permissions: Vec<_> = masks
            .iter()
            .map(|mask| {
                LabelSet::new(
                    (0..2)
                        .filter(|i| mask & (1 << i) != 0)
                        .map(LabelId)
                        .collect(),
                )
            })
            .collect();
        let active = plain("A B").output().active().clone();
        let p = Problem::new(
            GraphClass::new(vec![2], vec![3]),
            Input::Explicit(
                ConstraintPair::new(vec!["i".into(), "j".into()], active.clone(), input.clone())
                    .unwrap(),
            ),
            ConstraintPair::new(vec!["A".into(), "B".into()], active, output.clone()).unwrap(),
            OutputMapping::Restricted(permissions),
        )
        .unwrap();
        let mut allowed = BTreeSet::new();
        // An independent oracle enumerates joint labels, projects to the original
        // constraints, and checks permissions; it never calls the product helper.
        for mut code in 0..64 {
            let mut pairs = Vec::new();
            for _ in 0..3 {
                let pair = code % 4;
                code /= 4;
                pairs.push(InputOutputLabel {
                    input: Some(LabelId(pair / 2)),
                    output: LabelId(pair % 2),
                });
            }
            if pairs.iter().all(|n| p.output_allowed(n.input, n.output))
                && accepts(
                    &input,
                    &Configuration::from_occurrences(pairs.iter().map(|n| n.input.unwrap()))
                        .unwrap(),
                )
                && accepts(
                    &output,
                    &Configuration::from_occurrences(pairs.iter().map(|n| n.output)).unwrap(),
                )
            {
                allowed.insert(Configuration::from_occurrences(pairs).unwrap());
            }
        }
        let result = compute_all(&p, exact()).unwrap();
        for (source, a) in result.input_output.nodes.iter().enumerate() {
            for (target, b) in result.input_output.nodes.iter().enumerate() {
                let expected = a.input == b.input
                    && allowed.iter().all(|configuration| {
                        if !configuration.multiplicities().iter().any(|&(n, _)| n == *a) {
                            return true;
                        }
                        let changed = Configuration::new(
                            configuration
                                .multiplicities()
                                .iter()
                                .map(|&(n, count)| (n, count - u32::from(n == *a)))
                                .chain(std::iter::once((*b, 1)))
                                .collect(),
                        )
                        .unwrap();
                        allowed.contains(&changed)
                    });
                assert_eq!(
                    result.input_output.successors[source].contains(&(target as u32)),
                    expected,
                    "case {case}: {a:?}->{b:?}"
                );
            }
        }
        for a in [LabelId(0), LabelId(1)] {
            for b in [LabelId(0), LabelId(1)] {
                let expected = [Some(LabelId(0)), Some(LabelId(1))]
                    .into_iter()
                    .all(|input| {
                        !p.output_allowed(input, a)
                            || (p.output_allowed(input, b)
                                && edge(&result.input_output, input, a, b))
                    });
                assert_eq!(edge(&result.input_aware, None, a, b), expected);
            }
        }
    }
}
