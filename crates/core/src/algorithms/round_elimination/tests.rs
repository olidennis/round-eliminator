use super::*;
use crate::{
    algorithms::configurations::accepts,
    model::{CondensedConfiguration, Configuration, Part},
    parser::parse_problem,
};

/// LLM: Deserialize the JSON fixture as text entry and parse it into the canonical model. Used
/// LLM: by this module's tests or benchmark preparation.
fn parse(value: serde_json::Value) -> Problem {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

/// LLM: Create a fresh computation context with the supplied test progress consumer and
/// LLM: cancellation flag. Used by algorithm fixtures to observe phases and interruption.
fn context<'a>(report: &'a mut dyn FnMut(crate::computation::Progress)) -> Computation<'a> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Computation::new(Cancellation::default(), report)
}

/// LLM: Regression test: verify that standard step matches the old matching example and can be
/// LLM: repeated. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn standard_step_matches_the_old_matching_example_and_can_be_repeated() {
    // LLM: Prepare `original` for the following computation; it stores the original for this
    // LLM: stage.
    let original = parse(serde_json::json!({"kind":"plain", "constraints":{
        "active":"M U^3\nP^4", "passive":"M^2\nU UP"}}));
    let result = apply(&original).unwrap();
    assert_eq!(result.graph_class().active(), &[2]);
    assert_eq!(result.graph_class().passive(), &[4]);
    assert_eq!(result.output().labels(), ["A", "B", "C"]);
    // LLM: Sorted set-labels: A={M}, B={U}, C={U,P}. No diagram sorting is needed.

    let expected = parse(serde_json::json!({"kind":"plain", "constraints":{
        "active":"A^2\nB C", "passive":"A BC^3\nC^4"}}));
    assert_eq!(result, expected);
    let next = apply(&result).unwrap();
    assert_eq!(next.graph_class(), original.graph_class());
    assert_eq!(original.output().labels(), ["M", "U", "P"]);
}

/// LLM: Regression test: verify that independent and paired inputs keep their original promise
/// LLM: with roles swapped. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn independent_and_paired_inputs_keep_their_original_promise_with_roles_swapped() {
    // LLM: Process the relevant label choices while preserving their table identities.
    for text in [
        serde_json::json!({"kind":"independent", "input":{"active":"ij^3", "passive":"i j"},
            "output":{"active":"A^3", "passive":"A^2"}}),
        serde_json::json!({"kind":"paired", "constraints":{
            "active":"(i,A)(j,B)^2", "passive":"(i,A) (j,B)"}}),
    ] {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = parse(text);
        let Input::Explicit(input) = p.input() else {
            panic!()
        };
        let result = apply(&p).unwrap();
        let Input::Explicit(new_input) = result.input() else {
            panic!()
        };
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(input.labels(), new_input.labels());
        assert_eq!(input.active(), new_input.passive());
        assert_eq!(input.passive(), new_input.active());
        assert_eq!(result.graph_class().active(), p.graph_class().passive());
        assert_eq!(result.graph_class().passive(), p.graph_class().active());
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            matches!(result.mapping(), OutputMapping::Unrestricted)
                == matches!(p.mapping(), OutputMapping::Unrestricted)
        );
    }
}

/// LLM: Regression test: verify that input aware fibers retain sets that global output
/// LLM: maximality would remove. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn input_aware_fibers_retain_sets_that_global_output_maximality_would_remove() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j", "passive":"i j"},
        "output":{"active":"A C", "passive":"A C\nB C"},
        "mapping":"i -> A\nj -> C"}));
    // LLM: Global output maximization gives AB C. Its AB label would be forbidden
    // LLM: at input i; joint maximization instead retains the legal fiber {A}.

    let result = apply(&p).unwrap();
    assert_eq!(result.output().labels(), ["A", "B"]);
    let OutputMapping::Restricted(mapping) = result.mapping() else {
        panic!()
    };
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        mapping,
        &[
            LabelSet::new(vec![LabelId(0)]),
            LabelSet::new(vec![LabelId(1)])
        ]
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(accepts(
        result.output().active(),
        &Configuration::from_occurrences([LabelId(0), LabelId(1)]).unwrap()
    ));
    assert!(accepts(
        result.output().passive(),
        &Configuration::from_occurrences([LabelId(0), LabelId(1)]).unwrap()
    ));
}

/// LLM: Regression test: verify that graph overrides missing outputs empty alphabet and large
/// LLM: counts are preserved. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn graph_overrides_missing_outputs_empty_alphabet_and_large_counts_are_preserved() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(
        serde_json::json!({"kind":"plain", "degrees":{"active":"0 1 3", "passive":"0 2 4"},
        "constraints":{"active":"()\nA\nA^5", "passive":"()\nA^2\nA^6"}}),
    );
    let result = apply(&p).unwrap();
    assert_eq!(result.graph_class().active(), &[0, 2, 4]);
    assert_eq!(result.graph_class().passive(), &[0, 1, 3]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        result.output().active().configurations_at_degree(0).len(),
        1
    );
    assert_eq!(
        result.output().passive().configurations_at_degree(0).len(),
        1
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        result
            .output()
            .active()
            .configurations_at_degree(4)
            .is_empty()
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        result
            .output()
            .active()
            .configurations_at_degree(6)
            .is_empty()
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        result
            .output()
            .passive()
            .configurations_at_degree(5)
            .is_empty()
    );

    // LLM: Prepare `impossible` for the following computation; it stores the impossible for
    // LLM: this stage.
    let impossible = parse(
        serde_json::json!({"kind":"mapped", "input":{"active":"i", "passive":"i^2"},
        "output":{"active":"A", "passive":"B^2"}, "mapping":"i -> A"}),
    );
    let result = apply(&impossible).unwrap();
    assert!(result.output().labels().is_empty());
    assert!(result.output().active().configurations().next().is_none());
    assert!(result.output().passive().configurations().next().is_none());
    // LLM: Prepare `Input` for the following computation; it stores the input for this stage.
    let Input::Explicit(input) = result.input() else {
        panic!()
    };
    assert_eq!(input.active().configurations_at_degree(2).len(), 1);
    assert_eq!(input.passive().configurations_at_degree(1).len(), 1);

    // LLM: Prepare `large` for the following computation; it stores the large for this stage.
    let large = parse(
        serde_json::json!({"kind":"mapped", "input":{"active":"ij^100000", "passive":"ij^100000"},
        "output":{"active":"A^100000", "passive":"A^100000"}, "mapping":"i -> A\nj -> A"}),
    );
    let result = apply(&large).unwrap();
    assert_eq!(result.output().labels().len(), 1); // LLM: Equal fibers share an ID.
    for constraint in [result.output().active(), result.output().passive()] {
        let rows = constraint.configurations_at_degree(100_000);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].parts().len(), 1);
        assert_eq!(rows[0].parts()[0].multiplicity(), 100_000);
    }
}

/// LLM: Regression test: verify that cancellation cached maxima and many fresh names. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn cancellation_cached_maxima_and_many_fresh_names() {
    // LLM: Prepare `p` for the following computation; it stores the p for this stage.
    let p = parse(serde_json::json!({"kind":"plain", "constraints":{"active":"A", "passive":"A"}}));
    let maximum = maximize::compute(&p).unwrap();
    let mut phases = Vec::new();
    let result = apply_with(&p, &maximum, &mut context(&mut |p| phases.push(p.phase))).unwrap();
    assert_eq!(result, apply(&p).unwrap());
    assert!(!phases.contains(&Phase::Maximizing));
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = move |_| cancel.cancel();
    let mut computation = Computation::new(cancellation, &mut report);
    assert!(
        apply_with(&p, &maximum, &mut computation)
            .unwrap_err()
            .downcast_ref::<Cancelled>()
            .is_some()
    );
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let names: BTreeSet<_> = (0..100).map(label_name).collect();
    assert_eq!(names.len(), 100);
}

/// LLM: Build the two-label constraint selected by a small mask. Used by exhaustive RE
/// LLM: equivalence tests.
fn two_label_constraint(mask: u8) -> Constraint {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Constraint::new(
        [(0, 0), (0, 1), (1, 1)]
            .into_iter()
            .enumerate()
            .filter(|(k, _)| mask & (1 << k) != 0)
            .map(|(_, (a, b))| {
                CondensedConfiguration::new(vec![
                    Part::new(LabelSet::new(vec![LabelId(a)]), 1).unwrap(),
                    Part::new(LabelSet::new(vec![LabelId(b)]), 1).unwrap(),
                ])
                .unwrap()
            })
            .collect(),
    )
}

/// LLM: Convert a small bit mask to a label-choice set. Used by the exhaustive RE oracle to
/// LLM: enumerate possible set labels.
fn choices(mask: u8) -> LabelSet {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    LabelSet::new(
        (0..2)
            .filter(|i| mask & (1 << i) != 0)
            .map(LabelId)
            .collect(),
    )
}

/// LLM: Construct a small occurrence pair as a concrete multiset. Used by the independent RE
/// LLM: oracle.
fn pair(a: LabelId, b: LabelId) -> Configuration {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Configuration::from_occurrences([a, b]).unwrap()
}

/// LLM: Regression test: verify that exhaustive step is equivalent to unpruned input aware re.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn exhaustive_step_is_equivalent_to_unpruned_input_aware_re() {
    // LLM: Independent oracle: the unpruned alphabet is ALL nonempty output sets.
    // LLM: Universal validity means every concrete output selection is valid;
    // LLM: existential validity means at least one selection is valid. Test both
    // LLM: directions of the local zero-round mapping, rather than expecting the
    // LLM: compact alphabet to equal the (usually larger) unpruned alphabet.

    for input_mask in 0..8 {
        // LLM: Intern the distinct nonempty output-set labels needed by the universal RE side.
        for output_mask in 0..8 {
            // LLM: Intern the distinct nonempty output-set labels needed by the universal RE
            // LLM: side.
            for ga in 0..4 {
                // LLM: Intern the distinct nonempty output-set labels needed by the universal
                // LLM: RE side.
                for gb in 0..4 {
                    // LLM: Retain or prepare the original promised input semantics for this
                    // LLM: stage.
                    let input = ConstraintPair::new(
                        vec!["i".into(), "j".into()],
                        two_label_constraint(7),
                        two_label_constraint(input_mask),
                    )
                    .unwrap();
                    // LLM: Prepare required output data without modifying the original problem
                    // LLM: snapshot.
                    let output = ConstraintPair::new(
                        vec!["A".into(), "B".into()],
                        two_label_constraint((input_mask + output_mask) % 8),
                        two_label_constraint(output_mask),
                    )
                    .unwrap();
                    // LLM: Prepare `p` for the following computation; it stores the p for this
                    // LLM: stage.
                    let p = Problem::new(
                        GraphClass::new(vec![2], vec![2]),
                        Input::Explicit(input.clone()),
                        output,
                        OutputMapping::Restricted(vec![choices(ga), choices(gb)]),
                    )
                    .unwrap();
                    let maximum = maximize::compute(&p).unwrap();
                    // LLM: Collect the computed values, preserving their association with the
                    // LLM: requested work items.
                    let result = apply_with(&p, &maximum, &mut context(&mut |_| {})).unwrap();
                    let alphabet = set_alphabet_with(&maximum, &Cancellation::default()).unwrap();
                    let OutputMapping::Restricted(mapping) = result.mapping() else {
                        panic!()
                    };
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for i in 0..2 {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for j in 0..2 {
                            // LLM: Prepare `inputs` for the following computation; it stores
                            // LLM: the inputs for this stage.
                            let inputs = pair(LabelId(i), LabelId(j));
                            if !accepts(input.passive(), &inputs) {
                                continue;
                            }
                            // LLM: Unpruned -> compact: enlarge each source set within
                            // LLM: its actual input permissions at the universal node.

                            for x in 1..4 {
                                // LLM: Visit the remaining alternatives for this stage,
                                // LLM: stopping when the required search condition is settled.
                                for y in 1..4 {
                                    // LLM: Prepare `sx` for the following computation; it
                                    // LLM: stores the sx for this stage.
                                    let sx = choices(x);
                                    let sy = choices(y);
                                    if !sx.iter().all(|o| p.output_allowed(Some(LabelId(i)), o))
                                        || !sy.iter().all(|o| p.output_allowed(Some(LabelId(j)), o))
                                    {
                                        continue;
                                    }
                                    // LLM: Prepare `valid` for the following computation; it
                                    // LLM: stores the valid for this stage.
                                    let valid = sx.iter().all(|a| {
                                        sy.iter()
                                            .all(|b| accepts(p.output().passive(), &pair(a, b)))
                                    });
                                    // LLM: Prepare `covered` for the following computation; it
                                    // LLM: stores the covered for this stage.
                                    let covered = alphabet.iter().enumerate().any(|(a, ta)| {
                                        // LLM: Update the current working state before
                                        // LLM: continuing this operation.
                                        sx.is_subset_of(ta)
                                            && mapping[i as usize].contains(LabelId(a as u32))
                                            && alphabet.iter().enumerate().any(|(b, tb)| {
                                                // LLM: Update the current working state before
                                                // LLM: continuing this operation.
                                                sy.is_subset_of(tb)
                                                    && mapping[j as usize]
                                                        .contains(LabelId(b as u32))
                                                    && accepts(
                                                        result.output().active(),
                                                        &pair(LabelId(a as u32), LabelId(b as u32)),
                                                    )
                                            })
                                    });
                                    // LLM: Check the expected result and invariants for this
                                    // LLM: fixture, including the relevant edge case.
                                    assert_eq!(
                                        covered, valid,
                                        "promise {input_mask}, outputs {output_mask}, map {ga}/{gb}, input {i}/{j}, sets {x}/{y}"
                                    );
                                }
                            }
                        }
                    }
                    // LLM: Compact -> unpruned: interpret new IDs as their sets.
                    // LLM: Check universal and existential sides independently; no
                    // LLM: SAT encoding or maximizer containment test is the oracle.

                    for (a, sa) in alphabet.iter().enumerate() {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for (b, sb) in alphabet.iter().enumerate() {
                            // LLM: Prepare `tuple` for the following computation; it stores
                            // LLM: the tuple for this stage.
                            let tuple = pair(LabelId(a as u32), LabelId(b as u32));
                            if accepts(result.output().active(), &tuple) {
                                assert!(sa.iter().all(|x| {
                                    sb.iter()
                                        .all(|y| accepts(p.output().passive(), &pair(x, y)))
                                }));
                            }
                            // LLM: Prepare `existential` for the following computation; it
                            // LLM: stores the existential for this stage.
                            let existential = sa.iter().any(|x| {
                                sb.iter().any(|y| accepts(p.output().active(), &pair(x, y)))
                            });
                            assert_eq!(accepts(result.output().passive(), &tuple), existential);
                        }
                    }
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for (i, allowed) in [choices(ga), choices(gb)].iter().enumerate() {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for (j, set) in alphabet.iter().enumerate() {
                            assert_eq!(
                                mapping[i].contains(LabelId(j as u32)),
                                set.is_subset_of(allowed)
                            );
                        }
                    }
                }
            }
        }
    }
}
