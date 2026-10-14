use super::*;
use crate::{
    algorithms::configurations::accepts,
    model::{CondensedConfiguration, Configuration, Part},
    parser::parse_problem,
};

fn parse(value: serde_json::Value) -> Problem {
    parse_problem(serde_json::from_value(value).unwrap()).unwrap()
}

fn context<'a>(report: &'a mut dyn FnMut(crate::computation::Progress)) -> Computation<'a> {
    Computation::new(Cancellation::default(), report)
}

#[test]
fn standard_step_matches_the_old_matching_example_and_can_be_repeated() {
    let original = parse(serde_json::json!({"kind":"plain", "constraints":{
        "active":"M U^3\nP^4", "passive":"M^2\nU UP"}}));
    let result = apply(&original).unwrap();
    assert_eq!(result.graph_class().active(), &[2]);
    assert_eq!(result.graph_class().passive(), &[4]);
    assert_eq!(result.output().labels(), ["A", "B", "C"]);
    // Sorted set-labels: A={M}, B={U}, C={U,P}. No diagram sorting is needed.
    let expected = parse(serde_json::json!({"kind":"plain", "constraints":{
        "active":"A^2\nB C", "passive":"A BC^3\nC^4"}}));
    assert_eq!(result, expected);
    let next = apply(&result).unwrap();
    assert_eq!(next.graph_class(), original.graph_class());
    assert_eq!(original.output().labels(), ["M", "U", "P"]);
}

#[test]
fn independent_and_paired_inputs_keep_their_original_promise_with_roles_swapped() {
    for text in [
        serde_json::json!({"kind":"independent", "input":{"active":"ij^3", "passive":"i j"},
            "output":{"active":"A^3", "passive":"A^2"}}),
        serde_json::json!({"kind":"paired", "constraints":{
            "active":"(i,A)(j,B)^2", "passive":"(i,A) (j,B)"}}),
    ] {
        let p = parse(text);
        let Input::Explicit(input) = p.input() else {
            panic!()
        };
        let result = apply(&p).unwrap();
        let Input::Explicit(new_input) = result.input() else {
            panic!()
        };
        assert_eq!(input.labels(), new_input.labels());
        assert_eq!(input.active(), new_input.passive());
        assert_eq!(input.passive(), new_input.active());
        assert_eq!(result.graph_class().active(), p.graph_class().passive());
        assert_eq!(result.graph_class().passive(), p.graph_class().active());
        assert!(
            matches!(result.mapping(), OutputMapping::Unrestricted)
                == matches!(p.mapping(), OutputMapping::Unrestricted)
        );
    }
}

#[test]
fn input_aware_fibers_retain_sets_that_global_output_maximality_would_remove() {
    let p = parse(serde_json::json!({"kind":"mapped",
        "input":{"active":"i j", "passive":"i j"},
        "output":{"active":"A C", "passive":"A C\nB C"},
        "mapping":"i -> A\nj -> C"}));
    // Global output maximization gives AB C. Its AB label would be forbidden
    // at input i; joint maximization instead retains the legal fiber {A}.
    let result = apply(&p).unwrap();
    assert_eq!(result.output().labels(), ["A", "B"]);
    let OutputMapping::Restricted(mapping) = result.mapping() else {
        panic!()
    };
    assert_eq!(
        mapping,
        &[
            LabelSet::new(vec![LabelId(0)]),
            LabelSet::new(vec![LabelId(1)])
        ]
    );
    assert!(accepts(
        result.output().active(),
        &Configuration::from_occurrences([LabelId(0), LabelId(1)]).unwrap()
    ));
    assert!(accepts(
        result.output().passive(),
        &Configuration::from_occurrences([LabelId(0), LabelId(1)]).unwrap()
    ));
}

#[test]
fn graph_overrides_missing_outputs_empty_alphabet_and_large_counts_are_preserved() {
    let p = parse(
        serde_json::json!({"kind":"plain", "degrees":{"active":"0 1 3", "passive":"0 2 4"},
        "constraints":{"active":"()\nA\nA^5", "passive":"()\nA^2\nA^6"}}),
    );
    let result = apply(&p).unwrap();
    assert_eq!(result.graph_class().active(), &[0, 2, 4]);
    assert_eq!(result.graph_class().passive(), &[0, 1, 3]);
    assert_eq!(
        result.output().active().configurations_at_degree(0).len(),
        1
    );
    assert_eq!(
        result.output().passive().configurations_at_degree(0).len(),
        1
    );
    assert!(
        result
            .output()
            .active()
            .configurations_at_degree(4)
            .is_empty()
    );
    assert!(
        result
            .output()
            .active()
            .configurations_at_degree(6)
            .is_empty()
    );
    assert!(
        result
            .output()
            .passive()
            .configurations_at_degree(5)
            .is_empty()
    );

    let impossible = parse(
        serde_json::json!({"kind":"mapped", "input":{"active":"i", "passive":"i^2"},
        "output":{"active":"A", "passive":"B^2"}, "mapping":"i -> A"}),
    );
    let result = apply(&impossible).unwrap();
    assert!(result.output().labels().is_empty());
    assert!(result.output().active().configurations().next().is_none());
    assert!(result.output().passive().configurations().next().is_none());
    let Input::Explicit(input) = result.input() else {
        panic!()
    };
    assert_eq!(input.active().configurations_at_degree(2).len(), 1);
    assert_eq!(input.passive().configurations_at_degree(1).len(), 1);

    let large = parse(
        serde_json::json!({"kind":"mapped", "input":{"active":"ij^100000", "passive":"ij^100000"},
        "output":{"active":"A^100000", "passive":"A^100000"}, "mapping":"i -> A\nj -> A"}),
    );
    let result = apply(&large).unwrap();
    assert_eq!(result.output().labels().len(), 1); // Equal fibers share an ID.
    for constraint in [result.output().active(), result.output().passive()] {
        let rows = constraint.configurations_at_degree(100_000);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].parts().len(), 1);
        assert_eq!(rows[0].parts()[0].multiplicity(), 100_000);
    }
}

#[test]
fn cancellation_cached_maxima_and_many_fresh_names() {
    let p = parse(serde_json::json!({"kind":"plain", "constraints":{"active":"A", "passive":"A"}}));
    let maximum = maximize::compute(&p).unwrap();
    let mut phases = Vec::new();
    let result = apply_with(&p, &maximum, &mut context(&mut |p| phases.push(p.phase))).unwrap();
    assert_eq!(result, apply(&p).unwrap());
    assert!(!phases.contains(&Phase::Maximizing));
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut report = move |_| cancel.cancel();
    let mut computation = Computation::new(cancellation, &mut report);
    assert!(
        apply_with(&p, &maximum, &mut computation)
            .unwrap_err()
            .downcast_ref::<Cancelled>()
            .is_some()
    );
    let names: BTreeSet<_> = (0..100).map(label_name).collect();
    assert_eq!(names.len(), 100);
}

fn two_label_constraint(mask: u8) -> Constraint {
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

fn choices(mask: u8) -> LabelSet {
    LabelSet::new(
        (0..2)
            .filter(|i| mask & (1 << i) != 0)
            .map(LabelId)
            .collect(),
    )
}

fn pair(a: LabelId, b: LabelId) -> Configuration {
    Configuration::from_occurrences([a, b]).unwrap()
}

#[test]
fn exhaustive_step_is_equivalent_to_unpruned_input_aware_re() {
    // Independent oracle: the unpruned alphabet is ALL nonempty output sets.
    // Universal validity means every concrete output selection is valid;
    // existential validity means at least one selection is valid. Test both
    // directions of the local zero-round mapping, rather than expecting the
    // compact alphabet to equal the (usually larger) unpruned alphabet.
    for input_mask in 0..8 {
        for output_mask in 0..8 {
            for ga in 0..4 {
                for gb in 0..4 {
                    let input = ConstraintPair::new(
                        vec!["i".into(), "j".into()],
                        two_label_constraint(7),
                        two_label_constraint(input_mask),
                    )
                    .unwrap();
                    let output = ConstraintPair::new(
                        vec!["A".into(), "B".into()],
                        two_label_constraint((input_mask + output_mask) % 8),
                        two_label_constraint(output_mask),
                    )
                    .unwrap();
                    let p = Problem::new(
                        GraphClass::new(vec![2], vec![2]),
                        Input::Explicit(input.clone()),
                        output,
                        OutputMapping::Restricted(vec![choices(ga), choices(gb)]),
                    )
                    .unwrap();
                    let maximum = maximize::compute(&p).unwrap();
                    let result = apply_with(&p, &maximum, &mut context(&mut |_| {})).unwrap();
                    let alphabet = set_alphabet_with(&maximum, &Cancellation::default()).unwrap();
                    let OutputMapping::Restricted(mapping) = result.mapping() else {
                        panic!()
                    };
                    for i in 0..2 {
                        for j in 0..2 {
                            let inputs = pair(LabelId(i), LabelId(j));
                            if !accepts(input.passive(), &inputs) {
                                continue;
                            }
                            // Unpruned -> compact: enlarge each source set within
                            // its actual input permissions at the universal node.
                            for x in 1..4 {
                                for y in 1..4 {
                                    let sx = choices(x);
                                    let sy = choices(y);
                                    if !sx.iter().all(|o| p.output_allowed(Some(LabelId(i)), o))
                                        || !sy.iter().all(|o| p.output_allowed(Some(LabelId(j)), o))
                                    {
                                        continue;
                                    }
                                    let valid = sx.iter().all(|a| {
                                        sy.iter()
                                            .all(|b| accepts(p.output().passive(), &pair(a, b)))
                                    });
                                    let covered = alphabet.iter().enumerate().any(|(a, ta)| {
                                        sx.is_subset_of(ta)
                                            && mapping[i as usize].contains(LabelId(a as u32))
                                            && alphabet.iter().enumerate().any(|(b, tb)| {
                                                sy.is_subset_of(tb)
                                                    && mapping[j as usize]
                                                        .contains(LabelId(b as u32))
                                                    && accepts(
                                                        result.output().active(),
                                                        &pair(LabelId(a as u32), LabelId(b as u32)),
                                                    )
                                            })
                                    });
                                    assert_eq!(
                                        covered, valid,
                                        "promise {input_mask}, outputs {output_mask}, map {ga}/{gb}, input {i}/{j}, sets {x}/{y}"
                                    );
                                }
                            }
                        }
                    }
                    // Compact -> unpruned: interpret new IDs as their sets.
                    // Check universal and existential sides independently; no
                    // SAT encoding or maximizer containment test is the oracle.
                    for (a, sa) in alphabet.iter().enumerate() {
                        for (b, sb) in alphabet.iter().enumerate() {
                            let tuple = pair(LabelId(a as u32), LabelId(b as u32));
                            if accepts(result.output().active(), &tuple) {
                                assert!(sa.iter().all(|x| {
                                    sb.iter()
                                        .all(|y| accepts(p.output().passive(), &pair(x, y)))
                                }));
                            }
                            let existential = sa.iter().any(|x| {
                                sb.iter().any(|y| accepts(p.output().active(), &pair(x, y)))
                            });
                            assert_eq!(accepts(result.output().passive(), &tuple), existential);
                        }
                    }
                    for (i, allowed) in [choices(ga), choices(gb)].iter().enumerate() {
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
