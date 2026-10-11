use super::*;

#[test]
fn concrete_configurations_canonicalize_counts_and_round_trip_labels() {
    let concrete = Configuration::new(vec![(2u8, 3), (1, 0), (0, 2), (2, 1)]).unwrap();
    assert_eq!(concrete.multiplicities(), &[(0, 2), (2, 4)]);
    assert_eq!(concrete.degree(), 6);
    assert_eq!(
        concrete.occurrences().collect::<Vec<_>>(),
        [0, 0, 2, 2, 2, 2]
    );
    assert_eq!(
        Configuration::from_occurrences([2, 0, 2, 0, 2, 2]).unwrap(),
        concrete
    );
    assert_eq!(
        Configuration::new(vec![(0u8, 0)]).unwrap(),
        Configuration::default()
    );

    let concrete = Configuration::new(vec![(LabelId(1), 2), (LabelId(1), 1)]).unwrap();
    assert_eq!(concrete.multiplicities(), &[(LabelId(1), 3)]);
    let json = serde_json::to_value(&concrete).unwrap();
    assert_eq!(
        serde_json::from_value::<Configuration>(json).unwrap(),
        concrete
    );
}

#[test]
fn concrete_configurations_check_overflow_without_expanding_counts() {
    let largest = Configuration::new(vec![(0u8, u32::MAX)]).unwrap();
    assert_eq!(largest.degree(), u32::MAX);
    assert_eq!(largest.multiplicities(), &[(0, u32::MAX)]);
    assert!(Configuration::new(vec![(0u8, u32::MAX), (0, 1)]).is_err());
    assert!(Configuration::new(vec![(0u8, u32::MAX), (1, 1)]).is_err());
}

fn singleton(id: u32, count: u32) -> Part {
    Part::new(LabelSet::new(vec![LabelId(id)]), count).unwrap()
}

#[test]
fn constraints_discard_exact_duplicates_preserving_first_occurrence() {
    let a = CondensedConfiguration::new(vec![singleton(0, 3)]).unwrap();
    let b = CondensedConfiguration::new(vec![singleton(1, 3)]).unwrap();
    let split_a = CondensedConfiguration::new(vec![singleton(0, 1), singleton(0, 2)]).unwrap();
    let empty = CondensedConfiguration::new(vec![]).unwrap();
    let constraint = Constraint::new(vec![
        b.clone(),
        a.clone(),
        b.clone(),
        empty.clone(),
        split_a.clone(),
        a.clone(),
        empty.clone(),
    ]);
    assert_eq!(constraint.configurations_at_degree(0), &[empty]);
    // The split form remains distinct until configuration normalization exists.
    assert_eq!(constraint.configurations_at_degree(3), &[b, a, split_a]);
}

#[test]
fn construction_enforces_multiplicities_degrees_and_references() {
    assert!(Part::<LabelId>::new(LabelSet::default(), 1).is_err());
    assert!(CondensedConfiguration::new(vec![singleton(0, u32::MAX), singleton(0, 1)]).is_err());

    let constraint = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(0, 2), singleton(0, 1)]).unwrap(),
        CondensedConfiguration::new(vec![singleton(0, 1)]).unwrap(),
        CondensedConfiguration::new(vec![singleton(0, 3)]).unwrap(),
    ]);
    assert_eq!(
        constraint
            .degrees()
            .iter()
            .map(DegreeGroup::degree)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(constraint.degrees()[1].configurations().len(), 2);
    for labels in [vec![], vec!["A".into(), "A".into()], vec!["".into()]] {
        assert!(ConstraintPair::new(labels, constraint.clone(), Constraint::new(vec![])).is_err());
    }
    assert!(ConstraintPair::new(vec!["A".into()], constraint, Constraint::new(vec![])).is_ok());
}

#[test]
fn configuration_construction_discards_zero_count_parts() {
    let configuration =
        CondensedConfiguration::new(vec![singleton(0, 0), singleton(1, 2), singleton(2, 0)])
            .unwrap();
    assert_eq!(configuration.parts(), &[singleton(1, 2)]);
    assert_eq!(configuration.degree(), 2);
    let empty = CondensedConfiguration::new(vec![singleton(0, 0)]).unwrap();
    assert!(empty.parts().is_empty());
    assert_eq!(empty.degree(), 0);
}

#[test]
fn graph_class_can_require_degrees_with_no_output_configurations() {
    let output =
        ConstraintPair::new(vec![], Constraint::new(vec![]), Constraint::new(vec![])).unwrap();
    let problem = Problem::new(
        GraphClass::new(vec![3, 1, 3], vec![2]),
        Input::Trivial,
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap();
    assert_eq!(problem.graph_class().active(), [1, 3]);
    assert!(problem.output().active().degrees().is_empty());
}

#[test]
fn empty_multiset_and_empty_constraint_have_different_meanings() {
    let empty = Constraint::<LabelId>::new(vec![]);
    let zero = Constraint::<LabelId>::new(vec![CondensedConfiguration::new(vec![]).unwrap()]);
    assert_ne!(empty, zero);
    assert_eq!(zero.degrees()[0].degree(), 0);
    assert_eq!(zero.degrees()[0].configurations().len(), 1);
}

#[test]
fn mapping_must_be_total_but_may_disallow_every_output() {
    let input = ConstraintPair::new(
        vec!["a".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    let output = ConstraintPair::new(
        vec!["X".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    let graph = GraphClass::new(vec![1], vec![1]);
    for mapping in [vec![], vec![LabelSet::new(vec![LabelId(1)])]] {
        assert!(
            Problem::new(
                graph.clone(),
                Input::Explicit(input.clone()),
                output.clone(),
                OutputMapping::Restricted(mapping)
            )
            .is_err()
        );
    }
    assert!(
        Problem::new(
            graph,
            Input::Explicit(input),
            output,
            OutputMapping::Restricted(vec![LabelSet::default()])
        )
        .is_ok()
    );
}

#[test]
fn ordinary_constraint_pairs_validate_passive_references_and_keep_separate_tables() {
    let valid = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(0, 1)]).unwrap(),
    ]);
    let invalid = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(1, 1)]).unwrap(),
    ]);
    assert!(ConstraintPair::new(vec!["a".into()], Constraint::new(vec![]), invalid).is_err());
    // Separate input/output tables may use the same names.
    let input =
        ConstraintPair::new(vec!["A".into()], Constraint::new(vec![]), valid.clone()).unwrap();
    let output = ConstraintPair::new(vec!["A".into()], valid, Constraint::new(vec![])).unwrap();
    let problem = Problem::new(
        GraphClass::new(vec![1], vec![1]),
        Input::Explicit(input),
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap();
    let Input::Explicit(input) = problem.input() else {
        panic!()
    };
    assert_eq!(input.labels(), ["A"]);
    assert_eq!(problem.output().labels(), ["A"]);
}

#[test]
fn serialized_problems_round_trip_without_changing_ids_or_graph_classes() {
    for text in [
        serde_json::json!({"kind":"plain", "constraints":{"active":"AB^0..2", "passive":"A^2"}, "degrees":{"active":"0 2 3"}}),
        serde_json::json!({"kind":"independent", "input":{"active":"ab^2", "passive":"a b"}, "output":{"active":"B A", "passive":"AB^2"}}),
        serde_json::json!({"kind":"mapped", "input":{"active":"a b", "passive":"ab^2"}, "output":{"active":"A B", "passive":"AB^2"}, "mapping":"a -> AB\nb -> B"}),
        serde_json::json!({"kind":"paired", "constraints":{"active":"(a,A)(a,B)^2", "passive":"(a,A) (b,B)"}, "degrees":{"passive":"0 2 4"}}),
    ] {
        let original = crate::parser::parse_problem(serde_json::from_value(text).unwrap()).unwrap();
        let json = serde_json::to_value(&original).unwrap();
        let decoded: Problem = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(serde_json::to_value(decoded).unwrap(), json);
    }
}

#[test]
fn restricted_trivial_input_has_one_implicit_entry() {
    let output = ConstraintPair::new(
        vec!["A".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    let graph = GraphClass::new(vec![1], vec![1]);
    assert!(
        Problem::new(
            graph.clone(),
            Input::Trivial,
            output.clone(),
            OutputMapping::Restricted(vec![])
        )
        .is_err()
    );
    let problem = Problem::new(
        graph,
        Input::Trivial,
        output,
        OutputMapping::Restricted(vec![LabelSet::default()]),
    )
    .unwrap();
    assert!(!problem.output_allowed(None, LabelId(0)));
}
