//! Passive family checks and blocking clauses against independent oracles.

use super::*;

#[test]
fn condensed_passive_check_matches_concrete_inputs_and_preserves_counterexamples() {
    use super::super::passive::{Emission, forbidden_combination, visit_forbidden};
    use crate::algorithms::configurations::concrete_multisets;

    for entry in [
        "ab^2",
        "ab^3",
        "ab^2 bc",
        "ab ac bc",
        "ab^2 bc\nac^3",
        "a^2 b",
        "()",
    ] {
        let problem = parse(serde_json::json!({"kind":"independent",
            "input":{"active":"abc", "passive":entry},
            "output":{"active":"AB", "passive":"AB^3"}}));
        let Input::Explicit(input) = problem.input() else {
            unreachable!()
        };
        let degree = problem.graph_class().passive()[0];
        let concrete = concrete_multisets(input.passive(), &[degree]);
        let families = prepare::prepare_passive_inputs(&problem, &Cancellation::default()).unwrap();
        let all_emissions: Vec<_> = (0..6)
            .map(|source| Emission {
                input: Some(LabelId(source / 2)),
                output: LabelId(source % 2),
                source,
            })
            .collect();
        let mut outputs: Vec<_> = (0..(1 << (degree + 1)))
            .map(|mask| (relation(degree, mask), false))
            .collect();
        // Hand-written maximized constraints: universal, monochrome, exact
        // counts, and at most one A. No maximization algorithm is used here.
        let maximized = if degree == 0 {
            vec!["()".to_owned(), String::new()]
        } else {
            vec![
                format!("AB^{degree}"),
                format!("A^{degree}\nB^{degree}"),
                format!("A B^{}", degree - 1),
                format!("AB B^{}", degree - 1),
                String::new(),
            ]
        };
        for passive in maximized {
            outputs.push((plain("AB", &passive).output().passive().clone(), true));
        }
        for (output_mask, (output, maximized)) in outputs.into_iter().enumerate() {
            let brute_bad = |emissions: &[Emission<u32>]| {
                concrete.iter().any(|input| {
                    let input: Vec<_> = input.occurrences().collect();
                    (0usize..(1 << degree)).any(|code| {
                        let labels: Vec<_> = (0..degree)
                            .map(|index| LabelId(((code >> index) & 1) as u32))
                            .collect();
                        input.iter().zip(&labels).all(|(&input, &output)| {
                            emissions.iter().any(|emission| {
                                emission.input == Some(input) && emission.output == output
                            })
                        }) && !brute_accepts(&output, &labels)
                    })
                })
            };
            let mut eager_clauses = Vec::new();
            if degree <= 2 {
                let literals: Vec<_> = all_emissions
                    .iter()
                    .map(|emission| Emission {
                        input: emission.input,
                        output: emission.output,
                        source: rustsat::types::Lit::positive(emission.source),
                    })
                    .collect();
                for family in &families {
                    visit_forbidden(
                        &output,
                        family,
                        &literals,
                        &Cancellation::default(),
                        |clause| {
                            eager_clauses.push(clause);
                            Ok(())
                        },
                    )
                    .unwrap();
                }
            }
            for mask in 0..64 {
                let emissions: Vec<_> = all_emissions
                    .iter()
                    .copied()
                    .filter(|emission| mask & (1 << emission.source) != 0)
                    .collect();
                let expected = brute_bad(&emissions);
                for mode in [PassiveMode::General, PassiveMode::Maximized] {
                    if mode == PassiveMode::Maximized && !maximized {
                        continue;
                    }
                    let counterexample = forbidden_combination(
                        &output,
                        &families,
                        &emissions,
                        mode,
                        &Cancellation::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        counterexample.is_some(),
                        expected,
                        "{entry}, {mask}, {output_mask}, {mode:?}"
                    );
                    if let Some(sources) = counterexample {
                        let supporting: Vec<_> = emissions
                            .iter()
                            .copied()
                            .filter(|emission| sources.contains(&emission.source))
                            .collect();
                        assert!(
                            brute_bad(&supporting),
                            "counterexample lost its input origins"
                        );
                    }
                }
                if degree <= 2 {
                    assert_eq!(
                        eager_clauses.iter().any(|clause| clause
                            .iter()
                            .all(|literal| mask & (1 << literal.var().idx32()) != 0)),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn both_modes_agree_on_manually_maximized_fixtures() {
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../benches/support/zero_round_maximized_cases.json"
    ))
    .unwrap();
    for fixture in fixtures {
        let problem = parse(fixture["definition"].clone());
        for mode in [PassiveMode::General, PassiveMode::Maximized] {
            let result = check(&problem, mode).unwrap();
            let solvable = matches!(result, ZeroRoundResult::Solvable { .. });
            assert_eq!(
                solvable,
                fixture["expected"] == "solvable",
                "{}, {mode:?}",
                fixture["name"]
            );
            if let ZeroRoundResult::Solvable { strategy } = result {
                // Always use the general verifier, irrespective of search mode.
                assert!(verify_strategy(&problem, &strategy));
            }
        }
    }
}
