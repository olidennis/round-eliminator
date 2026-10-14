use super::*;
use crate::{
    algorithms::configurations::{accepts, concrete_multisets},
    model::{CondensedConfiguration, Configuration, ConstraintPair, GraphClass, LabelSet, Part},
};

/// LLM: Deserialize the JSON fixture as text entry and parse it into the canonical model. Used
/// LLM: by this module's tests or benchmark preparation.
fn parse(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    crate::parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}
/// LLM: Parse a plain active/passive fixture with inferred degrees. Used by algorithm tests
/// LLM: and benchmark preparation.
fn plain(passive: &str) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse(serde_json::json!({"kind":"plain", "constraints":{"active":"A B", "passive":passive}}))
}
/// LLM: Construct exact diagram options for the requested relation kind. Used by cache and
/// LLM: relation tests to avoid incidental method differences.
fn exact() -> DiagramOptions {
    // LLM: Update the current working state before continuing this operation.
    DiagramOptions {
        kind: DiagramKind::OutputOnly,
        output: OutputDiagramMethod::Exact,
        input_output: InputOutputDiagramMethod::Exact,
    }
}

/// LLM: Regression test: verify that each request computes only its relation and required
/// LLM: dependencies. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn each_request_computes_only_its_relation_and_required_dependencies() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j k", "passive":"i j\nj k"},
        "output":{"active":"A B X Y", "passive":"A X\nB X\nA Y"},
        "mapping":"i -> AB\nj -> X\nk -> Y"}));
    // LLM: Process the relevant label choices while preserving their table identities.
    for kind in [
        DiagramKind::OutputOnly,
        DiagramKind::InputOutput,
        DiagramKind::InputAware,
    ] {
        // LLM: Allocate shared coverage answers only when enough label profiles coincide to
        // LLM: justify it.
        let mut comparisons = Vec::new();
        let mut reductions = 0;
        // LLM: Connect phase/work updates to the caller while retaining cooperative
        // LLM: cancellation.
        let mut report = |progress: crate::computation::Progress| {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if progress.phase == Phase::Comparing {
                comparisons.push(progress.iterations);
            }
            if progress.phase == Phase::Reducing {
                reductions += 1;
            }
        };
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = compute_with(
            &p,
            DiagramOptions { kind, ..exact() },
            &mut Computation::new(Cancellation::default(), &mut report),
        )
        .unwrap();
        // LLM: There are four output labels and four admitted pairs. A second
        // LLM: comparison phase would reveal an unrequested output-only computation.

        assert_eq!(comparisons, vec![0, 1, 2, 3, 4]);
        assert_eq!(reductions, 1);
        assert_eq!(result.options.kind, kind);
        assert_eq!(result.diagram.nodes.len(), 4);
    }
    // LLM: Exact input-output checks must not depend on the output-only method choice.

    for kind in [DiagramKind::InputOutput, DiagramKind::InputAware] {
        // LLM: Prepare `exact` for the following computation; it stores the exact for this
        // LLM: stage.
        let exact = super::compute(&p, DiagramOptions { kind, ..exact() })
            .unwrap()
            .diagram;
        // LLM: Keep partial mathematical proofs independently of ordinary progress.
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
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(exact, partial);
    }
}
// LLM: Tests comparing the three semantics issue three independent requests.
/// LLM: A test fixture holding the related diagram results needed to compare relation kinds
/// LLM: against independent replacement oracles.
struct Diagrams {
    output_only: Diagram,
    input_output: Diagram,
    input_aware: Diagram,
}
/// LLM: Compute the related diagram variants needed by this test fixture. Used to compare
/// LLM: exact and cheap relations against independent replacement oracles.
fn compute_all(problem: &Problem, options: DiagramOptions) -> anyhow::Result<Diagrams> {
    // LLM: Prepare `requested` for the following computation; it stores the requested for this
    // LLM: stage.
    let requested = |kind| {
        super::compute(problem, DiagramOptions { kind, ..options }).map(|result| result.diagram)
    };
    Ok(Diagrams {
        output_only: requested(DiagramKind::OutputOnly)?,
        input_output: requested(DiagramKind::InputOutput)?,
        input_aware: requested(DiagramKind::InputAware)?,
    })
}
/// LLM: Look up whether a replacement arrow holds for the selected input and output IDs. Used
/// LLM: by diagram tests to compare semantic arrows independently of SCC display edges.
fn edge(diagram: &Diagram, input: Option<LabelId>, a: LabelId, b: LabelId) -> bool {
    // LLM: Create the source capacity side of the matching network.
    let source = diagram
        .nodes
        .iter()
        .position(|n| *n == InputOutputLabel { input, output: a })
        .unwrap();
    // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
    // LLM: traversal to backtrack or continue.
    let Some(target) = diagram
        .nodes
        .iter()
        .position(|n| *n == InputOutputLabel { input, output: b })
    else {
        return false;
    };
    diagram.successors[source].contains(&(target as u32))
}

/// LLM: Regression test: verify that exact output agrees with exhaustive replacements and
/// LLM: partial is sound. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn exact_output_agrees_with_exhaustive_replacements_and_partial_is_sound() {
    // LLM: Every binary multiset language of degree three. Concrete fixtures also
    // LLM: expose arrows covered collectively by rows rather than one condensed row.

    for mask in 0u32..16 {
        // LLM: Prepare `passive` for the following computation; it stores the passive for this
        // LLM: stage.
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
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
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
        // LLM: Evaluate the implementation under test on the same fixture.
        let actual = compute_all(&p, exact()).unwrap();
        let partial = compute_all(
            &p,
            DiagramOptions {
                output: OutputDiagramMethod::Partial,
                ..exact()
            },
        )
        .unwrap();
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for a in [LabelId(0), LabelId(1)] {
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for b in [LabelId(0), LabelId(1)] {
                // LLM: Construct the independent reference result for this fixture.
                let expected = concrete_multisets(&passive, &[3])
                    .iter()
                    .all(|configuration| {
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if !configuration
                            .multiplicities()
                            .iter()
                            .any(|&(label, _)| label == a)
                        {
                            return true;
                        }
                        // LLM: Prepare `changed` for the following computation; it stores the
                        // LLM: changed for this stage.
                        let changed = Configuration::new(
                            configuration
                                .multiplicities()
                                .iter()
                                .map(|&(label, count)| (label, count - u32::from(label == a)))
                                .chain(std::iter::once((b, 1)))
                                .collect(),
                        )
                        .unwrap();
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
                        accepts(&passive, &changed)
                    });
                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
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

/// LLM: Regression test: verify that maximal occurrence test and nonmaximal exact have the
/// LLM: same semantics. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn maximal_occurrence_test_and_nonmaximal_exact_have_the_same_semantics() {
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = plain("AB^3");
    let expanded = plain("A^3\nA^2 B\nA B^2\nB^3");
    // LLM: Prepare `full` for the following computation; it stores the full for this stage.
    let full = compute_all(
        &maximum,
        DiagramOptions {
            output: OutputDiagramMethod::Exact,
            ..exact()
        },
    )
    .unwrap();
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = compute_all(&expanded, exact()).unwrap();
    assert_eq!(full.output_only, result.output_only);
    // LLM: Keep partial mathematical proofs independently of ordinary progress.
    let partial = compute_all(
        &expanded,
        DiagramOptions {
            output: OutputDiagramMethod::Partial,
            ..exact()
        },
    )
    .unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!partial.output_only.complete);
    assert!(!edge(&partial.output_only, None, LabelId(0), LabelId(1)));
    assert_eq!(result.output_only.components, vec![vec![0, 1]]);
    assert!(result.output_only.edges.is_empty());
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for passive in ["A B\nB^2", "B\nA B\nB^2", "()\nA B\nB^2"] {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = plain(passive);
        let result = compute_all(&p, exact()).unwrap();
        assert!(edge(&result.output_only, None, LabelId(0), LabelId(1)));
        if passive.starts_with("B\n") {
            assert!(!edge(&result.output_only, None, LabelId(1), LabelId(0)));
        }
    }
}

/// LLM: Regression test: verify that exact pairs use the input promise and input aware checks
/// LLM: missing targets. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn exact_pairs_use_the_input_promise_and_input_aware_checks_missing_targets() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j k","passive":"i j\nj k"},
        "output":{"active":"A B X Y", "passive":"A X\nB X\nA Y"},
        "mapping":"i -> AB\nj -> X\nk -> Y"}));
    let result = compute_all(&p, exact()).unwrap();
    let id =
        |name: &str| LabelId(p.output().labels().iter().position(|s| s == name).unwrap() as u32);
    // LLM: A->B fails in the raw constraint because AY is allowed but BY is not.

    assert!(!edge(&result.output_only, None, id("A"), id("B")));
    // LLM: At i, only input j (output X) can be adjacent. The bad Y context is excluded.

    assert!(edge(
        &result.input_output,
        Some(LabelId(0)),
        id("A"),
        id("B")
    ));
    assert!(edge(&result.input_aware, None, id("A"), id("B")));
    let cheap = compute_all(&p, DiagramOptions::default()).unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!edge(
        &cheap.input_output,
        Some(LabelId(0)),
        id("A"),
        id("B")
    ));
    assert!(!cheap.input_output.complete && result.input_output.complete);
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j", "passive":"ij^2"},
        "output":{"active":"A B", "passive":"AB^2"},
        "mapping":"i -> AB\nj -> A"}));
    let result = compute_all(&p, exact()).unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(edge(
        &result.input_output,
        Some(LabelId(0)),
        LabelId(0),
        LabelId(1)
    ));
    assert!(!edge(&result.input_aware, None, LabelId(0), LabelId(1)));
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(result.input_output.edges.iter().all(|&(a, b)| {
        let a =
            result.input_output.nodes[result.input_output.components[a as usize][0] as usize].input;
        let b =
            result.input_output.nodes[result.input_output.components[b as usize][0] as usize].input;
        a == b
    }));
}

/// LLM: Regression test: verify that graph class counts trivial input and cancellation. Used
/// LLM: by the test suite to protect this behavior while implementations change.
#[test]
fn graph_class_counts_trivial_input_and_cancellation() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(
        serde_json::json!({"kind":"plain", "constraints":{"active":"A B","passive":"A\nAB^2"},
        "degrees":{"active":"2","passive":"2"}}),
    );
    let result = compute_all(&p, exact()).unwrap();
    assert_eq!(result.output_only, result.input_output);
    assert_eq!(result.output_only, result.input_aware);
    let p = plain("AB^4294967295");
    // LLM: Assert that interruption propagates as cancellation and does not produce a
    // LLM: completed mathematical result.
    assert!(edge(
        &compute_all(&p, exact()).unwrap().output_only,
        None,
        LabelId(0),
        LabelId(1)
    ));
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = move |progress: crate::computation::Progress| {
        if progress.phase == Phase::Comparing {
            cancel.cancel()
        }
    };
    // LLM: Assert that interruption propagates as cancellation and does not produce a
    // LLM: completed mathematical result.
    assert!(
        compute_with(
            &p,
            exact(),
            &mut Computation::new(cancellation.clone(), &mut report)
        )
        .is_err()
    );
    // LLM: Assert that interruption propagates as cancellation and does not produce a
    // LLM: completed mathematical result.
    assert!(cancellation.is_cancelled());
}

/// LLM: Regression test: verify that exact joint relations agree with exhaustive input output
/// LLM: multisets. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn exact_joint_relations_agree_with_exhaustive_input_output_multisets() {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use std::collections::BTreeSet;
    let constraint = |mask: u32| {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
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
    // LLM: Process the relevant label choices while preserving their table identities.
    for case in 0..128u32 {
        // LLM: Retain or prepare the original promised input semantics for this stage.
        let input = constraint(case % 16);
        let output = constraint((case * 7 + case / 16) % 16);
        let masks = [(case / 4) % 4, (case / 16) % 4];
        // LLM: Prepare `permissions` for the following computation; it stores the permissions
        // LLM: for this stage.
        let permissions: Vec<_> = masks
            .iter()
            .map(|mask| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                LabelSet::new(
                    (0..2)
                        .filter(|i| mask & (1 << i) != 0)
                        .map(LabelId)
                        .collect(),
                )
            })
            .collect();
        // LLM: Prepare `active` for the following computation; it stores the active for this
        // LLM: stage.
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
        // LLM: Prepare `allowed` for the following computation; it stores the allowed for this
        // LLM: stage.
        let mut allowed = BTreeSet::new();
        // LLM: An independent oracle enumerates joint labels, projects to the original
        // LLM: constraints, and checks permissions; it never calls the product helper.

        for mut code in 0..64 {
            // LLM: Queue the parent pairs for this bounded generation batch.
            let mut pairs = Vec::new();
            for _ in 0..3 {
                // LLM: Prepare `pair` for the following computation; it stores the pair for
                // LLM: this stage.
                let pair = code % 4;
                code /= 4;
                pairs.push(InputOutputLabel {
                    input: Some(LabelId(pair / 2)),
                    output: LabelId(pair % 2),
                });
            }
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
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
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = compute_all(&p, exact()).unwrap();
        for (source, a) in result.input_output.nodes.iter().enumerate() {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for (target, b) in result.input_output.nodes.iter().enumerate() {
                // LLM: Construct the independent reference result for this fixture.
                let expected = a.input == b.input
                    && allowed.iter().all(|configuration| {
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if !configuration.multiplicities().iter().any(|&(n, _)| n == *a) {
                            return true;
                        }
                        // LLM: Prepare `changed` for the following computation; it stores the
                        // LLM: changed for this stage.
                        let changed = Configuration::new(
                            configuration
                                .multiplicities()
                                .iter()
                                .map(|&(n, count)| (n, count - u32::from(n == *a)))
                                .chain(std::iter::once((*b, 1)))
                                .collect(),
                        )
                        .unwrap();
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        allowed.contains(&changed)
                    });
                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
                assert_eq!(
                    result.input_output.successors[source].contains(&(target as u32)),
                    expected,
                    "case {case}: {a:?}->{b:?}"
                );
            }
        }
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for a in [LabelId(0), LabelId(1)] {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for b in [LabelId(0), LabelId(1)] {
                // LLM: Construct the independent reference result for this fixture.
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
