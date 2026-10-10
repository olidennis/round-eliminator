use super::*;
use crate::labels::LabelSet;
use crate::parse_problem;
use crate::problem::{
    CondensedConfiguration, Constraint, ConstraintPair, GraphClass, IndependentProblem,
    MappedProblem, Part, PlainProblem,
};
use std::collections::BTreeSet;

fn parse(value: serde_json::Value) -> Problem {
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

fn plain(active: &str, passive: &str) -> Problem {
    parse(
        serde_json::json!({ "kind": "plain", "constraints": { "active": active, "passive": passive } }),
    )
}

fn expect(problem: &Problem, solvable: bool) -> Vec<LocalRule> {
    let result = check(problem).unwrap();
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

#[test]
fn plain_degree_two_and_unmaximized_higher_degrees() {
    expect(&plain("A B", "A^2\nA B\nB^2"), true);
    expect(&plain("A B", "A B"), false);
    // Collectively covers every triple over {A,B}; no one line covers it.
    expect(&plain("A B", "A^3\nA^2 B\nA B^2\nB^3"), true);
    expect(&plain("A B", "A^2 B\nA B^2"), false);
    // Refinement must retain valid alternatives, not just reject one model.
    expect(&plain("A^2\nB^2", "B^3"), true);
}

#[test]
fn nonregular_degrees_zero_and_empty_constraints() {
    let strategy = expect(&plain("()\nA\nA^3", "()\nA^2\nA^4"), true);
    assert_eq!(
        strategy.iter().map(|rule| rule.degree).collect::<Vec<_>>(),
        [0, 1, 3]
    );
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"A", "passive":"A^2"},
        "degrees":{"active":"1,3"}}),
        ),
        false,
    );
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"()", "passive":""},
        "degrees":{"passive":"0"}}),
        ),
        false,
    );
    expect(&plain("()", "()"), true);
    expect(&plain("", ""), true);
    expect(&plain("AB", "A"), true);
    expect(&plain("B", "A"), false);
    expect(
        &parse(
            serde_json::json!({"kind":"plain", "constraints":{"active":"A\nA B", "passive":"A^2"},
        "degrees":{"active":"1"}}),
        ),
        true,
    );
}

#[test]
fn repeated_inputs_can_receive_different_outputs() {
    let problem = parse(
        serde_json::json!({ "kind":"independent", "input":{"active":"a^2", "passive":"a^3"},
        "output":{"active":"A B", "passive":"AB^3"} }),
    );
    let strategy = expect(&problem, true);
    assert_ne!(strategy[0].output[0], strategy[0].output[1]);
    let mut invalid = strategy.clone();
    invalid[0].output[1] = invalid[0].output[0];
    assert!(!verify_strategy(&problem, &invalid));
    assert!(!verify_strategy(&problem, &[]));
    invalid = strategy.clone();
    invalid.push(strategy[0].clone());
    assert!(!verify_strategy(&problem, &invalid));
    invalid = strategy;
    invalid[0].input = Some(vec![LabelId(99); 2]);
    assert!(!verify_strategy(&problem, &invalid));
}

#[test]
fn outputs_can_depend_on_the_whole_active_input() {
    let problem = parse(serde_json::json!({"kind":"paired", "constraints":{
        "active":"(a,A) (b,B)\n(a,B) (c,A)", "passive":"(a,A)(a,B)\n(b,B)\n(c,A)"}}));
    let strategy = expect(&problem, true);
    assert_eq!(strategy.len(), 2);
    // The occurrence of input a needs different outputs in its two contexts.
    assert_ne!(strategy[0].output[0], strategy[1].output[0]);
}

#[test]
fn paired_correlations_and_mapping_restrictions_are_preserved() {
    for (passive, solvable) in [("(a,A) (b,B)", true), ("(a,B) (b,A)", false)] {
        expect(
            &parse(serde_json::json!({"kind":"paired", "constraints":{
            "active":"(a,A) (b,B)", "passive":passive}})),
            solvable,
        );
    }
    for (mapping, solvable) in [
        ("a -> A\nb -> B", true),
        ("a -> A\nb -> A", false),
        ("a ->\nb -> B", false),
    ] {
        expect(
            &parse(
                serde_json::json!({"kind":"mapped", "input":{"active":"a b", "passive":"a b"},
            "output":{"active":"A B", "passive":"AB^2"}, "mapping":mapping}),
            ),
            solvable,
        );
    }
    // Input and output labels have unrelated numeric IDs in their own tables.
    expect(
        &parse(
            serde_json::json!({"kind":"mapped", "input":{"active":"a b", "passive":"a b"},
        "output":{"active":"B A", "passive":"A B"}, "mapping":"a -> A\nb -> B"}),
        ),
        true,
    );
}

fn relation(degree: u32, mask: usize) -> Constraint {
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

fn pair(active: Constraint, passive: Constraint) -> ConstraintPair {
    ConstraintPair::new(vec!["A".into(), "B".into()], active, passive).unwrap()
}

// Deliberately separate from the production matching algorithm: try all
// bijections to explicitly expanded part slots for these tiny test instances.
fn brute_accepts<L: Copy + Ord>(constraint: &Constraint<L>, labels: &[L]) -> bool {
    fn assign<L: Copy + Ord>(
        slots: &[&LabelSet<L>],
        labels: &[L],
        used: usize,
        position: usize,
    ) -> bool {
        if position == labels.len() {
            return true;
        }
        (0..slots.len()).any(|slot| {
            used & (1 << slot) == 0
                && slots[slot].contains(labels[position])
                && assign(slots, labels, used | (1 << slot), position + 1)
        })
    }
    constraint.configurations().any(|configuration| {
        let slots: Vec<_> = configuration
            .parts()
            .iter()
            .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
            .collect();
        slots.len() == labels.len() && assign(&slots, labels, 0, 0)
    })
}

fn brute_validity(validity: Validity<'_>, input: &[LabelId], output: &[LabelId]) -> bool {
    match validity {
        Validity::Labels(constraint) => brute_accepts(constraint, output),
        Validity::Pairs(constraint) => brute_accepts(
            constraint,
            &input
                .iter()
                .zip(output)
                .map(|(&input, &output)| LabelPair { input, output })
                .collect::<Vec<_>>(),
        ),
    }
}

fn brute_solvable(problem: &Problem) -> bool {
    let cancellation = Cancellation::default();
    let (active, passive) = Validity::for_problem(problem);
    let mut rules = prepare::contexts(problem, &cancellation).unwrap();
    let count: usize = rules.iter().map(|rule| rule.degree as usize).sum();
    assert!(count < 16);
    for mask in 0usize..(1 << count) {
        let mut bit = 0;
        let mut emitted = BTreeSet::new();
        let mut valid = true;
        for rule in &mut rules {
            rule.output = (0..rule.degree)
                .map(|_| {
                    let id = LabelId(((mask >> bit) & 1) as u32);
                    bit += 1;
                    id
                })
                .collect();
            valid &= brute_validity(active, rule.input.as_deref().unwrap_or(&[]), &rule.output);
            for (position, &output) in rule.output.iter().enumerate() {
                let input = rule.input.as_ref().map(|labels| labels[position]);
                valid &= validity::mapped_output_allowed(problem, input, output);
                emitted.insert((input, output));
            }
        }
        if !valid {
            continue;
        }
        for input in prepare::passive_inputs(problem, &cancellation).unwrap() {
            let input: Vec<_> = input.occurrences().collect();
            for combination in 0usize..(1 << input.len()) {
                let output: Vec<_> = (0..input.len())
                    .map(|position| LabelId(((combination >> position) & 1) as u32))
                    .collect();
                if input
                    .iter()
                    .copied()
                    .zip(output.iter().copied())
                    .all(|pair| emitted.contains(&pair))
                {
                    valid &= brute_validity(
                        passive,
                        &input.iter().flatten().copied().collect::<Vec<_>>(),
                        &output,
                    );
                }
            }
        }
        if valid {
            return true;
        }
    }
    false
}

#[test]
fn sat_agrees_with_exhaustive_plain_and_input_search() {
    for passive_degree in [2, 3] {
        let graph = GraphClass::new(vec![2], vec![passive_degree]);
        for active_mask in 0..8 {
            for passive_mask in 0..(1 << (passive_degree + 1)) {
                let output = pair(
                    relation(2, active_mask),
                    relation(passive_degree, passive_mask),
                );
                let problem = Problem::Plain(PlainProblem::new(graph.clone(), output.clone()));
                expect(&problem, brute_solvable(&problem));
                let input = pair(
                    relation(2, 0b111),
                    relation(passive_degree, (1 << (passive_degree + 1)) - 1),
                );
                let problem = Problem::Independent(IndependentProblem::new(
                    graph.clone(),
                    input.clone(),
                    output.clone(),
                ));
                expect(&problem, brute_solvable(&problem));
                for first in 0..4 {
                    for second in 0..4 {
                        let allowed = [first, second].map(|mask| {
                            LabelSet::new(
                                (0..2)
                                    .filter(|label| mask & (1 << label) != 0)
                                    .map(LabelId)
                                    .collect(),
                            )
                        });
                        let problem = Problem::Mapped(
                            MappedProblem::new(
                                graph.clone(),
                                input.clone(),
                                output.clone(),
                                allowed.to_vec(),
                            )
                            .unwrap(),
                        );
                        expect(&problem, brute_solvable(&problem));
                    }
                }
            }
        }
    }
}

#[test]
fn condensed_active_matching_agrees_with_exhaustive_search() {
    for active in ["AB^2", "AB A", "AB B", "AB AB", "AB^3", "AB A^2\nB^3"] {
        for passive in ["AB^3", "A^3\nA B^2", "A^2 B\nB^3"] {
            let problem = plain(active, passive);
            expect(&problem, brute_solvable(&problem));
        }
    }
    for active in [
        "(a,A)(a,B)(b,A)(b,B)^2",
        "(a,A)(b,B) (a,B)(b,A)",
        "(a,A)(a,B) (a,A)(b,B)",
    ] {
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

#[test]
fn paired_sat_agrees_with_exhaustive_search() {
    use crate::problem::{InputOutputConstraintPair, PairedProblem};
    let pairs = [
        LabelPair {
            input: LabelId(0),
            output: LabelId(0),
        },
        LabelPair {
            input: LabelId(0),
            output: LabelId(1),
        },
        LabelPair {
            input: LabelId(1),
            output: LabelId(0),
        },
        LabelPair {
            input: LabelId(1),
            output: LabelId(1),
        },
    ];
    let relation = |degree: u32, mask: usize| {
        let tuples: Vec<Vec<_>> = (0usize..4usize.pow(degree))
            .map(|mut code| {
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
        Constraint::new(
            tuples
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, tuple)| {
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
    for passive_degree in [2, 3] {
        for seed in 0..80 {
            let active = (seed * 137 + 31) % 1024;
            let passive = if passive_degree == 2 {
                (seed * 211 + 7) % 1024
            } else {
                (seed * 27961 + 7817) % (1 << 20)
            };
            let constraints = InputOutputConstraintPair::new(
                vec!["a".into(), "b".into()],
                vec!["A".into(), "B".into()],
                relation(2, active),
                relation(passive_degree, passive),
            )
            .unwrap();
            let problem = Problem::Paired(
                PairedProblem::new(GraphClass::new(vec![2], vec![passive_degree]), constraints)
                    .unwrap(),
            );
            expect(&problem, brute_solvable(&problem));
        }
    }
}
