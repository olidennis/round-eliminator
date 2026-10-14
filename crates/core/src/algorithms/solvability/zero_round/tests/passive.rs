//! LLM: Passive family checks and blocking clauses against independent oracles.

use super::*;

/// LLM: Regression test: verify that condensed passive check matches concrete inputs and
/// LLM: preserves counterexamples. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn condensed_passive_check_matches_concrete_inputs_and_preserves_counterexamples() {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use super::super::passive::{Emission, forbidden_combination, visit_forbidden};
    use crate::algorithms::configurations::concrete_multisets;

    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    for entry in [
        "ab^2",
        "ab^3",
        "ab^2 bc",
        "ab ac bc",
        "ab^2 bc\nac^3",
        "a^2 b",
        "()",
    ] {
        // LLM: Prepare `problem` for the following computation; it stores the problem for this
        // LLM: stage.
        let problem = parse(serde_json::json!({"kind":"independent",
            "input":{"active":"abc", "passive":entry},
            "output":{"active":"AB", "passive":"AB^3"}}));
        let Input::Explicit(input) = problem.input() else {
            unreachable!()
        };
        let degree = problem.graph_class().passive()[0];
        let concrete = concrete_multisets(input.passive(), &[degree]);
        // LLM: Retain the original passive promise as condensed input families.
        let families = prepare::prepare_passive_inputs(&problem, &Cancellation::default()).unwrap();
        let all_emissions: Vec<_> = (0..6)
            .map(|source| Emission {
                input: Some(LabelId(source / 2)),
                output: LabelId(source % 2),
                source,
            })
            .collect();
        // LLM: Prepare `outputs` for the following computation; it stores the outputs for this
        // LLM: stage.
        let mut outputs: Vec<_> = (0..(1 << (degree + 1)))
            .map(|mask| (relation(degree, mask), false))
            .collect();
        // LLM: Hand-written maximized constraints: universal, monochrome, exact
        // LLM: counts, and at most one A. No maximization algorithm is used here.

        let maximized = if degree == 0 {
            vec!["()".to_owned(), String::new()]
        } else {
            // LLM: Update the current working state before continuing this operation.
            vec![
                format!("AB^{degree}"),
                format!("A^{degree}\nB^{degree}"),
                format!("A B^{}", degree - 1),
                format!("AB B^{}", degree - 1),
                String::new(),
            ]
        };
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for passive in maximized {
            outputs.push((plain("AB", &passive).output().passive().clone(), true));
        }
        // LLM: Find the emission origins of a passive violation so refinement blocks that
        // LLM: simultaneous choice.
        for (output_mask, (output, maximized)) in outputs.into_iter().enumerate() {
            // LLM: Prepare `brute_bad` for the following computation; it stores the brute bad
            // LLM: for this stage.
            let brute_bad = |emissions: &[Emission<u32>]| {
                // LLM: Update the current working state before continuing this operation.
                concrete.iter().any(|input| {
                    // LLM: Retain or prepare the original promised input semantics for this
                    // LLM: stage.
                    let input: Vec<_> = input.occurrences().collect();
                    (0usize..(1 << degree)).any(|code| {
                        // LLM: Prepare `labels` for the following computation; it stores the
                        // LLM: labels for this stage.
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
            // LLM: Prepare `eager_clauses` for the following computation; it stores the eager
            // LLM: clauses for this stage.
            let mut eager_clauses = Vec::new();
            if degree <= 2 {
                // LLM: Prepare `literals` for the following computation; it stores the
                // LLM: literals for this stage.
                let literals: Vec<_> = all_emissions
                    .iter()
                    .map(|emission| Emission {
                        input: emission.input,
                        output: emission.output,
                        source: rustsat::types::Lit::positive(emission.source),
                    })
                    .collect();
                // LLM: Encode every forbidden low-degree source combination needed by future
                // LLM: SAT candidates.
                for family in &families {
                    // LLM: Encode every forbidden low-degree source combination needed by
                    // LLM: future SAT candidates.
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
            // LLM: Find the emission origins of a passive violation so refinement blocks that
            // LLM: simultaneous choice.
            for mask in 0..64 {
                // LLM: Collect available outputs with their input/color descriptors and source
                // LLM: provenance.
                let emissions: Vec<_> = all_emissions
                    .iter()
                    .copied()
                    .filter(|emission| mask & (1 << emission.source) != 0)
                    .collect();
                let expected = brute_bad(&emissions);
                // LLM: Find the emission origins of a passive violation so refinement blocks
                // LLM: that simultaneous choice.
                for mode in [PassiveMode::General, PassiveMode::Maximized] {
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if mode == PassiveMode::Maximized && !maximized {
                        continue;
                    }
                    // LLM: Find the emission origins of a passive violation so refinement
                    // LLM: blocks that simultaneous choice.
                    let counterexample = forbidden_combination(
                        &output,
                        &families,
                        &emissions,
                        mode,
                        &Cancellation::default(),
                    )
                    .unwrap();
                    // LLM: Compare the implementation result with the independently prepared
                    // LLM: reference for this fixture.
                    assert_eq!(
                        counterexample.is_some(),
                        expected,
                        "{entry}, {mask}, {output_mask}, {mode:?}"
                    );
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if let Some(sources) = counterexample {
                        // LLM: Prepare `supporting` for the following computation; it stores
                        // LLM: the supporting for this stage.
                        let supporting: Vec<_> = emissions
                            .iter()
                            .copied()
                            .filter(|emission| sources.contains(&emission.source))
                            .collect();
                        // LLM: Check the expected result and invariants for this fixture,
                        // LLM: including the relevant edge case.
                        assert!(
                            brute_bad(&supporting),
                            "counterexample lost its input origins"
                        );
                    }
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if degree <= 2 {
                    // LLM: Compare the implementation result with the independently prepared
                    // LLM: reference for this fixture.
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

/// LLM: Regression test: verify that both modes agree on manually maximized fixtures. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn both_modes_agree_on_manually_maximized_fixtures() {
    // LLM: Prepare `fixtures` for the following computation; it stores the fixtures for this
    // LLM: stage.
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/benches/support/zero_round_maximized_cases.json"
    )))
    .unwrap();
    // LLM: Check the returned local rules independently of the solver encoding.
    for fixture in fixtures {
        // LLM: Prepare `problem` for the following computation; it stores the problem for this
        // LLM: stage.
        let problem = parse(fixture["definition"].clone());
        for mode in [PassiveMode::General, PassiveMode::Maximized] {
            // LLM: Collect the computed values, preserving their association with the
            // LLM: requested work items.
            let result = check(&problem, mode).unwrap();
            let solvable = matches!(result, ZeroRoundResult::Solvable { .. });
            assert_eq!(
                solvable,
                fixture["expected"] == "solvable",
                "{}, {mode:?}",
                fixture["name"]
            );
            // LLM: Check the returned local rules independently of the solver encoding.
            if let ZeroRoundResult::Solvable { strategy } = result {
                // LLM: Always use the general verifier, irrespective of search mode.

                assert!(verify_strategy(&problem, &strategy));
            }
        }
    }
}
