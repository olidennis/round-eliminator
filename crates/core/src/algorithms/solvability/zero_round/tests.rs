use super::*;
use crate::model::LabelSet;
use crate::model::{
    CondensedConfiguration, Constraint, ConstraintPair, GraphClass, Input, OutputMapping, Part,
};
use crate::parse_problem;
use std::collections::BTreeSet;

/// LLM: Deserialize the JSON fixture as text entry and parse it into the canonical model. Used
/// LLM: by this module's tests or benchmark preparation.
fn parse(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

/// LLM: Parse a plain active/passive fixture with inferred degrees. Used by algorithm tests
/// LLM: and benchmark preparation.
fn plain(active: &str, passive: &str) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse(
        serde_json::json!({ "kind": "plain", "constraints": { "active": active, "passive": passive } }),
    )
}

/// LLM: Check the requested solvability classification and independently verify any returned
/// LLM: strategy. Used by zero-round regression tests; eligible plain fixtures also compare
/// LLM: the specialized search with SAT.
fn expect(problem: &Problem, solvable: bool) -> Vec<LocalRule> {
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = check(problem, PassiveMode::General).unwrap();
    let mut report = |_| {};
    // LLM: Check the returned local rules independently of the solver encoding.
    if degree_two::applicable(problem) {
        // LLM: Prepare `port_result` for the following computation; it stores the port result
        // LLM: for this stage.
        let port_result = search(
            problem,
            PassiveMode::General,
            None,
            backend(&Cancellation::default()),
            &mut Computation::new(Cancellation::default(), &mut report),
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            matches!(port_result, ZeroRoundResult::Solvable { .. }),
            solvable
        );
        // LLM: Prepare `specialized` for the following computation; it stores the specialized
        // LLM: for this stage.
        let specialized = degree_two::check(
            problem,
            &mut Computation::new(Cancellation::default(), &mut report),
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            matches!(specialized, ZeroRoundResult::Solvable { .. }),
            solvable
        );
        if let ZeroRoundResult::Solvable { strategy } = specialized {
            assert!(verify_strategy(problem, &strategy));
        }
    }
    // LLM: Check the returned local rules independently of the solver encoding.
    match result {
        ZeroRoundResult::Solvable { strategy } => {
            assert!(solvable, "unexpected witness: {strategy:?}");
            assert!(verify_strategy(problem, &strategy));
            strategy
        }
        ZeroRoundResult::Unsolvable => {
            assert!(!solvable);
            vec![]
        }
    }
}

/// LLM: Regression test: verify that plain degree two and unmaximized higher degrees. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn plain_degree_two_and_unmaximized_higher_degrees() {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    expect(&plain("A B", "A^2\nA B\nB^2"), true);
    expect(&plain("A B", "A B"), false);
    // LLM: Collectively covers every triple over {A,B}; no one line covers it.

    expect(&plain("A B", "A^3\nA^2 B\nA B^2\nB^3"), true);
    expect(&plain("A B", "A^2 B\nA B^2"), false);
    // LLM: Refinement must retain valid alternatives, not just reject one model.

    expect(&plain("A^2\nB^2", "B^3"), true);
}

/// LLM: Regression test: verify that nonregular degrees zero and empty constraints. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn nonregular_degrees_zero_and_empty_constraints() {
    // LLM: Allocate or reconstruct one local rule per promised active context.
    let strategy = expect(&plain("()\nA\nA^3", "()\nA^2\nA^4"), true);
    assert_eq!(
        strategy.iter().map(|rule| rule.degree).collect::<Vec<_>>(),
        [0, 1, 3]
    );
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"A", "passive":"A^2"},
        "degrees":{"active":"1,3"}}),
        ),
        false,
    );
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"()", "passive":""},
        "degrees":{"passive":"0"}}),
        ),
        false,
    );
    expect(&plain("()", "()"), true);
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    expect(&plain("", ""), true);
    expect(&plain("AB", "A"), true);
    expect(&plain("B", "A"), false);
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"A\nA B", "passive":"A^2"},
        "degrees":{"active":"1"}}),
        ),
        true,
    );
}

/// LLM: Regression test: verify that repeated inputs can receive different outputs. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn repeated_inputs_can_receive_different_outputs() {
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = parse(
        serde_json::json!({ "kind":"independent", "input":{"active":"a^2", "passive":"a^3"},
        "output":{"active":"A B", "passive":"AB^3"} }),
    );
    let strategy = expect(&problem, true);
    assert_ne!(strategy[0].output[0], strategy[0].output[1]);
    let mut invalid = strategy.clone();
    invalid[0].output[1] = invalid[0].output[0];
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(!verify_strategy(&problem, &invalid));
    assert!(!verify_strategy(&problem, &[]));
    invalid = strategy.clone();
    invalid.push(strategy[0].clone());
    assert!(!verify_strategy(&problem, &invalid));
    invalid = strategy;
    invalid[0].input = Some(vec![LabelId(99); 2]);
    assert!(!verify_strategy(&problem, &invalid));
}

/// LLM: Regression test: verify that outputs can depend on the whole active input. Used by the
/// LLM: test suite to protect this behavior while implementations change.
#[test]
fn outputs_can_depend_on_the_whole_active_input() {
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = parse(serde_json::json!({"kind":"paired", "constraints":{
        "active":"(a,A) (b,B)\n(a,B) (c,A)", "passive":"(a,A)(a,B)\n(b,B)\n(c,A)"}}));
    let strategy = expect(&problem, true);
    assert_eq!(strategy.len(), 2);
    // LLM: The occurrence of input a needs different outputs in its two contexts.

    assert_ne!(strategy[0].output[0], strategy[1].output[0]);
}

/// LLM: Regression test: verify that paired correlations and mapping restrictions are
/// LLM: preserved. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn paired_correlations_and_mapping_restrictions_are_preserved() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (passive, solvable) in [("(a,A) (b,B)", true), ("(a,B) (b,A)", false)] {
        expect(
            &parse(serde_json::json!({"kind":"paired", "constraints":{
            "active":"(a,A) (b,B)", "passive":passive}})),
            solvable,
        );
    }
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (mapping, solvable) in [
        ("a -> A\nb -> B", true),
        ("a -> A\nb -> A", false),
        ("a ->\nb -> B", false),
    ] {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        expect(
            &parse(
                serde_json::json!({"kind":"mapped", "input":{"active":"a b", "passive":"a b"},
            "output":{"active":"A B", "passive":"AB^2"}, "mapping":mapping}),
            ),
            solvable,
        );
    }
    // LLM: Input and output labels have unrelated numeric IDs in their own tables.

    expect(
        &parse(
            serde_json::json!({"kind":"mapped", "input":{"active":"a b", "passive":"a b"},
        "output":{"active":"B A", "passive":"A B"}, "mapping":"a -> A\nb -> B"}),
        ),
        true,
    );
}

/// LLM: Construct the small two-label constraint selected by its degree and bit mask. Used as
/// LLM: an independent exhaustive solvability fixture.
fn relation(degree: u32, mask: usize) -> Constraint {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Constraint::new(
        (0..=degree)
            .filter(|ones| mask & (1 << ones) != 0)
            .map(|ones| {
                let parts = [(LabelId(0), degree - ones), (LabelId(1), ones)]
                    .into_iter()
                    .map(|(label, count)| Part::new(LabelSet::new(vec![label]), count).unwrap())
                    .collect();
                CondensedConfiguration::new(parts).unwrap()
            })
            .collect(),
    )
}

/// LLM: Construct a validated active/passive fixture over its shared name table. Used by
/// LLM: exhaustive zero-round tests.
fn pair(active: Constraint, passive: Constraint) -> ConstraintPair {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    ConstraintPair::new(vec!["A".into(), "B".into()], active, passive).unwrap()
}

// LLM: Deliberately separate from the production matching algorithm: try all
// LLM: bijections to explicitly expanded part slots for these tiny test instances.
/// LLM: Check concrete membership by exhaustive occurrence assignment. Used as an independent
/// LLM: oracle for the counted matcher and SAT tests.
fn brute_accepts<L: Copy + Ord>(constraint: &Constraint<L>, labels: &[L]) -> bool {
    /// LLM: Explore a concrete occurrence-to-part assignment while tracking remaining
    /// LLM: capacities. Used by the independent brute-force membership oracle.
    fn assign<L: Copy + Ord>(
        slots: &[&LabelSet<L>],
        labels: &[L],
        used: usize,
        position: usize,
    ) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if position == labels.len() {
            return true;
        }
        (0..slots.len()).any(|slot| {
            used & (1 << slot) == 0
                && slots[slot].contains(labels[position])
                && assign(slots, labels, used | (1 << slot), position + 1)
        })
    }
    // LLM: Update the current working state before continuing this operation.
    constraint.configurations().any(|configuration| {
        // LLM: Prepare `slots` for the following computation; it stores the slots for this
        // LLM: stage.
        let slots: Vec<_> = configuration
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        slots.len() == labels.len() && assign(&slots, labels, 0, 0)
    })
}

// LLM: Keep the oracle's concrete input enumeration independent of the condensed checker.
/// LLM: Expand passive input promises only for the small independent test oracle. Used to
/// LLM: compare production condensed passive verification with explicit enumeration.
fn concrete_passive_inputs(problem: &Problem) -> Vec<crate::model::Configuration<Option<LabelId>>> {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use crate::algorithms::configurations::concrete_multisets;
    use crate::model::Configuration;
    // LLM: Select the representation path that preserves the supplied input promise and
    // LLM: incidence permissions.
    match problem.input() {
        Input::Explicit(input) => {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            concrete_multisets(input.passive(), problem.graph_class().passive())
                .into_iter()
                .map(|input| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    Configuration::new(
                        input
                            .multiplicities()
                            .iter()
                            .map(|&(label, count)| (Some(label), count))
                            .collect(),
                    )
                    .unwrap()
                })
                .collect()
        }
        Input::Trivial => problem
            .graph_class()
            .passive()
            .iter()
            .map(|&degree| Configuration::new(vec![(None, degree)]).unwrap())
            .collect(),
    }
}

/// LLM: Exhaustively search small zero-round strategies. Used to validate SAT and specialized
/// LLM: decisions without relying on their encodings.
fn brute_solvable(problem: &Problem) -> bool {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    brute_solvable_with(
        problem,
        problem.output().labels().len(),
        |_, output| brute_accepts(problem.output().active(), output),
        |_, output| brute_accepts(problem.output().passive(), output),
    )
}

// LLM: Supply original paired validity as well as canonical validity, so conversion
// LLM: is checked against the entry semantics independently of its own tag encoding.
/// LLM: Recursively assign outputs to the prepared small local contexts and validate all
/// LLM: passive combinations. Used by the independent exhaustive zero-round oracle.
fn brute_solvable_with(
    problem: &Problem,
    alphabet: usize,
    active: impl Fn(&[LabelId], &[LabelId]) -> bool,
    passive: impl Fn(&[LabelId], &[LabelId]) -> bool,
) -> bool {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = Cancellation::default();
    let mut rules = prepare::prepare_local_rules(problem, &cancellation).unwrap();
    let count: usize = rules.iter().map(|rule| rule.degree as usize).sum();
    assert!(count < 16);
    // LLM: Process the relevant label choices while preserving their table identities.
    for mut code in 0usize..alphabet.pow(count as u32) {
        // LLM: Prepare `emitted` for the following computation; it stores the emitted for this
        // LLM: stage.
        let mut emitted = BTreeSet::new();
        let mut valid = true;
        // LLM: Process the relevant label choices while preserving their table identities.
        for rule in &mut rules {
            // LLM: Update the current working state before continuing this operation.
            rule.output = (0..rule.degree)
                .map(|_| {
                    let id = LabelId((code % alphabet) as u32);
                    code /= alphabet;
                    id
                })
                .collect();
            valid &= active(rule.input.as_deref().unwrap_or(&[]), &rule.output);
            // LLM: Process the relevant label choices while preserving their table identities.
            for (position, &output) in rule.output.iter().enumerate() {
                let input = rule.input.as_ref().map(|labels| labels[position]);
                valid &= problem.output_allowed(input, output);
                emitted.insert((input, output));
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if !valid {
            continue;
        }
        // LLM: Process the relevant label choices while preserving their table identities.
        for input in concrete_passive_inputs(problem) {
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let input: Vec<_> = input.occurrences().collect();
            for mut combination in 0usize..alphabet.pow(input.len() as u32) {
                // LLM: Prepare required output data without modifying the original problem
                // LLM: snapshot.
                let output: Vec<_> = (0..input.len())
                    .map(|_| {
                        let label = LabelId((combination % alphabet) as u32);
                        combination /= alphabet;
                        label
                    })
                    .collect();
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if input
                    .iter()
                    .copied()
                    .zip(output.iter().copied())
                    .all(|pair| emitted.contains(&pair))
                {
                    valid &= passive(
                        &input.iter().flatten().copied().collect::<Vec<_>>(),
                        &output,
                    );
                }
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if valid {
            return true;
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    false
}

/// LLM: Regression test: verify that sat agrees with exhaustive plain and input search. Used
/// LLM: by the test suite to protect this behavior while implementations change.
#[test]
fn sat_agrees_with_exhaustive_plain_and_input_search() {
    // LLM: Process the relevant label choices while preserving their table identities.
    for passive_degree in [2, 3] {
        // LLM: Prepare `graph` for the following computation; it stores the graph for this
        // LLM: stage.
        let graph = GraphClass::new(vec![2], vec![passive_degree]);
        for active_mask in 0..8 {
            // LLM: Process the relevant label choices while preserving their table identities.
            for passive_mask in 0..(1 << (passive_degree + 1)) {
                // LLM: Prepare required output data without modifying the original problem
                // LLM: snapshot.
                let output = pair(
                    relation(2, active_mask),
                    relation(passive_degree, passive_mask),
                );
                // LLM: Prepare `problem` for the following computation; it stores the problem
                // LLM: for this stage.
                let problem = Problem::new(
                    graph.clone(),
                    Input::Trivial,
                    output.clone(),
                    OutputMapping::Unrestricted,
                )
                .unwrap();
                expect(&problem, brute_solvable(&problem));
                // LLM: Retain or prepare the original promised input semantics for this stage.
                let input = pair(
                    relation(2, 0b111),
                    relation(passive_degree, (1 << (passive_degree + 1)) - 1),
                );
                // LLM: Prepare `problem` for the following computation; it stores the problem
                // LLM: for this stage.
                let problem = Problem::new(
                    graph.clone(),
                    Input::Explicit(input.clone()),
                    output.clone(),
                    OutputMapping::Unrestricted,
                )
                .unwrap();
                expect(&problem, brute_solvable(&problem));
                // LLM: Process the relevant label choices while preserving their table
                // LLM: identities.
                for first in 0..4 {
                    // LLM: Process the relevant label choices while preserving their table
                    // LLM: identities.
                    for second in 0..4 {
                        // LLM: Prepare `allowed` for the following computation; it stores the
                        // LLM: allowed for this stage.
                        let allowed = [first, second].map(|mask| {
                            // LLM: Delegate to the common implementation so this entry point
                            // LLM: shares its semantics and checks.
                            LabelSet::new(
                                (0..2)
                                    .filter(|label| mask & (1 << label) != 0)
                                    .map(LabelId)
                                    .collect(),
                            )
                        });
                        // LLM: Prepare `problem` for the following computation; it stores the
                        // LLM: problem for this stage.
                        let problem = Problem::new(
                            graph.clone(),
                            Input::Explicit(input.clone()),
                            output.clone(),
                            OutputMapping::Restricted(allowed.to_vec()),
                        )
                        .unwrap();
                        expect(&problem, brute_solvable(&problem));
                    }
                }
            }
        }
    }
}

/// LLM: Regression test: verify that condensed active matching agrees with exhaustive search.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn condensed_active_matching_agrees_with_exhaustive_search() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for active in ["AB^2", "AB A", "AB B", "AB AB", "AB^3", "AB A^2\nB^3"] {
        for passive in ["AB^3", "A^3\nA B^2", "A^2 B\nB^3"] {
            let problem = plain(active, passive);
            expect(&problem, brute_solvable(&problem));
        }
    }
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for active in [
        "(a,A)(a,B)(b,A)(b,B)^2",
        "(a,A)(b,B) (a,B)(b,A)",
        "(a,A)(a,B) (a,A)(b,B)",
    ] {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for passive in [
            "(a,A)(a,B)(b,A)(b,B)^3",
            "(a,A)(b,A)^3\n(a,B)(b,B)^3",
            "(a,A)(b,A)^2 (a,B)(b,B)",
        ] {
            let problem = parse(
                serde_json::json!({"kind":"paired", "constraints":{"active":active,"passive":passive}}),
            );
            expect(&problem, brute_solvable(&problem));
        }
    }
}

/// LLM: Regression test: verify that paired sat agrees with exhaustive search. Used by the
/// LLM: test suite to protect this behavior while implementations change.
#[test]
fn paired_sat_agrees_with_exhaustive_search() {
    // LLM: Queue the parent pairs for this bounded generation batch.
    let pairs = [
        (LabelId(0), LabelId(0)),
        (LabelId(0), LabelId(1)),
        (LabelId(1), LabelId(0)),
        (LabelId(1), LabelId(1)),
    ];
    // LLM: Prepare `relation` for the following computation; it stores the relation for this
    // LLM: stage.
    let relation = |degree: u32, mask: usize| {
        // LLM: Prepare `tuples` for the following computation; it stores the tuples for this
        // LLM: stage.
        let tuples: Vec<Vec<_>> = (0usize..4usize.pow(degree))
            .map(|mut code| {
                // LLM: Update the current working state before continuing this operation.
                (0..degree)
                    .map(|_| {
                        let index = code % 4;
                        code /= 4;
                        index
                    })
                    .collect()
            })
            .filter(|tuple: &Vec<usize>| tuple.windows(2).all(|pair| pair[0] <= pair[1]))
            .collect();
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Constraint::new(
            tuples
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, tuple)| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    CondensedConfiguration::new(
                        tuple
                            .iter()
                            .map(|&index| Part::new(LabelSet::new(vec![pairs[index]]), 1).unwrap())
                            .collect(),
                    )
                    .unwrap()
                })
                .collect(),
        )
    };
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for passive_degree in [2, 3] {
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for seed in 0..80 {
            // LLM: Prepare `active` for the following computation; it stores the active for
            // LLM: this stage.
            let active = (seed * 137 + 31) % 1024;
            let passive = if passive_degree == 2 {
                (seed * 211 + 7) % 1024
            } else {
                (seed * 27961 + 7817) % (1 << 20)
            };
            let active = relation(2, active);
            let passive = relation(passive_degree, passive);
            // LLM: The oracle projects tuples independently of the parser's pair
            // LLM: types and conversion, keeping this equivalence test independent.

            let project = |constraint: &Constraint<(LabelId, LabelId)>| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                Constraint::new(
                    constraint
                        .configurations()
                        .map(|configuration| {
                            // LLM: Delegate to the common implementation so this entry point
                            // LLM: shares its semantics and checks.
                            CondensedConfiguration::new(
                                configuration
                                    .parts()
                                    .iter()
                                    .map(|part| {
                                        // LLM: Delegate to the common implementation so this
                                        // LLM: entry point shares its semantics and checks.
                                        Part::new(
                                            LabelSet::new(
                                                part.labels().iter().map(|pair| pair.0).collect(),
                                            ),
                                            part.multiplicity(),
                                        )
                                        .unwrap()
                                    })
                                    .collect(),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
            };
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let input = ConstraintPair::new(
                vec!["a".into(), "b".into()],
                project(&active),
                project(&passive),
            )
            .unwrap();
            let graph = GraphClass::new(vec![2], vec![passive_degree]);
            // LLM: Prepare `promise` for the following computation; it stores the promise for
            // LLM: this stage.
            let promise = Problem::new(
                graph.clone(),
                Input::Explicit(input),
                pair(Constraint::new(vec![]), Constraint::new(vec![])),
                OutputMapping::Unrestricted,
            )
            .unwrap();
            // LLM: Prepare `original_validity` for the following computation; it stores the
            // LLM: original validity for this stage.
            let original_validity = |constraint: &Constraint<(LabelId, LabelId)>,
                                     input: &[LabelId],
                                     output: &[LabelId]| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                brute_accepts(
                    constraint,
                    &input
                        .iter()
                        .zip(output)
                        .map(|(&input, &output)| (input, output))
                        .collect::<Vec<_>>(),
                )
            };
            // LLM: Construct the independent reference result for this fixture.
            let expected = brute_solvable_with(
                &promise,
                2,
                |input, output| original_validity(&active, input, output),
                |input, output| original_validity(&passive, input, output),
            );
            // LLM: Prepare `entry` for the following computation; it stores the entry for this
            // LLM: stage.
            let entry = |constraint: &Constraint<(LabelId, LabelId)>| {
                // LLM: Update the current working state before continuing this operation.
                constraint
                    .configurations()
                    .map(|configuration| {
                        // LLM: Handle the empty case explicitly before the general traversal
                        // LLM: or allocation path.
                        if configuration.parts().is_empty() {
                            return "()".to_owned();
                        }
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        configuration
                            .parts()
                            .iter()
                            .map(|part| {
                                // LLM: Prepare `labels` for the following computation; it
                                // LLM: stores the labels for this stage.
                                let labels = part
                                    .labels()
                                    .iter()
                                    .map(|pair| {
                                        format!(
                                            "({},{})",
                                            ["a", "b"][pair.0.0 as usize],
                                            ["A", "B"][pair.1.0 as usize]
                                        )
                                    })
                                    .collect::<String>();
                                // LLM: Update the current working state before continuing this
                                // LLM: operation.
                                format!("{labels}^{}", part.multiplicity())
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            // LLM: Prepare `problem` for the following computation; it stores the problem for
            // LLM: this stage.
            let problem = parse(serde_json::json!({"kind": "paired", "constraints": {
                "active": entry(&active), "passive": entry(&passive)
            }, "degrees": {"active": "2", "passive": passive_degree.to_string()}}));
            expect(&problem, expected);
        }
    }
}

mod passive;

/// LLM: Regression test: verify that specialized degree two handles deep configurations and
/// LLM: interrupts search. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn specialized_degree_two_handles_deep_configurations_and_interrupts_search() {
    // LLM: Prepare `part` for the following computation; it stores the part for this stage.
    let part = Part::new(LabelSet::new(vec![LabelId(0)]), 1).unwrap();
    let output = ConstraintPair::new(
        vec!["A".into()],
        Constraint::new(vec![
            CondensedConfiguration::new(vec![part; 10_000]).unwrap(),
        ]),
        Constraint::new(vec![
            CondensedConfiguration::new(vec![
                Part::new(LabelSet::new(vec![LabelId(0)]), 2).unwrap(),
            ])
            .unwrap(),
        ]),
    )
    .unwrap();
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = Problem::new(
        GraphClass::new(vec![10_000], vec![2]),
        Input::Trivial,
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap();
    let result = check(&problem, PassiveMode::General).unwrap();
    // LLM: Prepare `ZeroRoundResult` for the following computation; it stores the zero round
    // LLM: result for this stage.
    let ZeroRoundResult::Solvable { strategy } = result else {
        panic!()
    };
    assert_eq!(strategy[0].output.len(), 10_000);
    assert!(verify_strategy(&problem, &strategy));
    let cancellation = Cancellation::default();
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |progress: crate::computation::Progress| {
        if progress.phase == Phase::Solving && progress.iterations >= 2 {
            cancellation.cancel();
        }
    };
    // LLM: Assert that interruption propagates as cancellation and does not produce a
    // LLM: completed mathematical result.
    assert!(
        check_with(
            &problem,
            PassiveMode::General,
            &mut Computation::new(cancellation.clone(), &mut report)
        )
        .is_err()
    );
}
