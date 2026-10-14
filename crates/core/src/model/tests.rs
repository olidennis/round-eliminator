use super::*;

/// LLM: Regression test: verify that concrete configurations canonicalize counts and round
/// LLM: trip labels. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn concrete_configurations_canonicalize_counts_and_round_trip_labels() {
    // LLM: Prepare `concrete` for the following computation; it stores the concrete for this
    // LLM: stage.
    let concrete = Configuration::new(vec![(2u8, 3), (1, 0), (0, 2), (2, 1)]).unwrap();
    assert_eq!(concrete.multiplicities(), &[(0, 2), (2, 4)]);
    assert_eq!(concrete.degree(), 6);
    assert_eq!(
        concrete.occurrences().collect::<Vec<_>>(),
        [0, 0, 2, 2, 2, 2]
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        Configuration::from_occurrences([2, 0, 2, 0, 2, 2]).unwrap(),
        concrete
    );
    assert_eq!(
        Configuration::new(vec![(0u8, 0)]).unwrap(),
        Configuration::default()
    );

    // LLM: Prepare `concrete` for the following computation; it stores the concrete for this
    // LLM: stage.
    let concrete = Configuration::new(vec![(LabelId(1), 2), (LabelId(1), 1)]).unwrap();
    assert_eq!(concrete.multiplicities(), &[(LabelId(1), 3)]);
    let json = serde_json::to_value(&concrete).unwrap();
    assert_eq!(
        serde_json::from_value::<Configuration>(json).unwrap(),
        concrete
    );
}

/// LLM: Regression test: verify that concrete configurations check overflow without expanding
/// LLM: counts. Used by the test suite to protect this behavior while implementations change.
#[test]
fn concrete_configurations_check_overflow_without_expanding_counts() {
    // LLM: Prepare `largest` for the following computation; it stores the largest for this
    // LLM: stage.
    let largest = Configuration::new(vec![(0u8, u32::MAX)]).unwrap();
    assert_eq!(largest.degree(), u32::MAX);
    assert_eq!(largest.multiplicities(), &[(0, u32::MAX)]);
    assert!(Configuration::new(vec![(0u8, u32::MAX), (0, 1)]).is_err());
    assert!(Configuration::new(vec![(0u8, u32::MAX), (1, 1)]).is_err());
}

/// LLM: Construct a one-label condensed part with the requested count. Used by model tests to
/// LLM: isolate multiplicity and reference invariants.
fn singleton(id: u32, count: u32) -> Part {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Part::new(LabelSet::new(vec![LabelId(id)]), count).unwrap()
}

/// LLM: Regression test: verify that constraints discard exact duplicates preserving first
/// LLM: occurrence. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn constraints_discard_exact_duplicates_preserving_first_occurrence() {
    // LLM: Prepare `a` for the following computation; it stores the a for this stage.
    let a = CondensedConfiguration::new(vec![singleton(0, 3)]).unwrap();
    let b = CondensedConfiguration::new(vec![singleton(1, 3)]).unwrap();
    let split_a = CondensedConfiguration::new(vec![singleton(0, 1), singleton(0, 2)]).unwrap();
    let empty = CondensedConfiguration::new(vec![]).unwrap();
    // LLM: Prepare `constraint` for the following computation; it stores the constraint for
    // LLM: this stage.
    let constraint = Constraint::new(vec![
        b.clone(),
        a.clone(),
        b.clone(),
        empty.clone(),
        split_a.clone(),
        a.clone(),
        empty.clone(),
    ]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(constraint.configurations_at_degree(0), &[empty]);
    // LLM: The split form remains distinct until configuration normalization exists.

    assert_eq!(constraint.configurations_at_degree(3), &[b, a, split_a]);
}

/// LLM: Regression test: verify that construction enforces multiplicities degrees and
/// LLM: references. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn construction_enforces_multiplicities_degrees_and_references() {
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(Part::<LabelId>::new(LabelSet::default(), 1).is_err());
    assert!(CondensedConfiguration::new(vec![singleton(0, u32::MAX), singleton(0, 1)]).is_err());

    // LLM: Prepare `constraint` for the following computation; it stores the constraint for
    // LLM: this stage.
    let constraint = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(0, 2), singleton(0, 1)]).unwrap(),
        CondensedConfiguration::new(vec![singleton(0, 1)]).unwrap(),
        CondensedConfiguration::new(vec![singleton(0, 3)]).unwrap(),
    ]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        constraint
            .degrees()
            .iter()
            .map(DegreeGroup::degree)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(constraint.degrees()[1].configurations().len(), 2);
    for labels in [vec![], vec!["A".into(), "A".into()], vec!["".into()]] {
        assert!(ConstraintPair::new(labels, constraint.clone(), Constraint::new(vec![])).is_err());
    }
    assert!(ConstraintPair::new(vec!["A".into()], constraint, Constraint::new(vec![])).is_ok());
}

/// LLM: Regression test: verify that configuration construction discards zero count parts.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn configuration_construction_discards_zero_count_parts() {
    // LLM: Prepare `configuration` for the following computation; it stores the configuration
    // LLM: for this stage.
    let configuration =
        CondensedConfiguration::new(vec![singleton(0, 0), singleton(1, 2), singleton(2, 0)])
            .unwrap();
    assert_eq!(configuration.parts(), &[singleton(1, 2)]);
    assert_eq!(configuration.degree(), 2);
    let empty = CondensedConfiguration::new(vec![singleton(0, 0)]).unwrap();
    assert!(empty.parts().is_empty());
    assert_eq!(empty.degree(), 0);
}

/// LLM: Regression test: verify that graph class can require degrees with no output
/// LLM: configurations. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn graph_class_can_require_degrees_with_no_output_configurations() {
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output =
        ConstraintPair::new(vec![], Constraint::new(vec![]), Constraint::new(vec![])).unwrap();
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = Problem::new(
        GraphClass::new(vec![3, 1, 3], vec![2]),
        Input::Trivial,
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap();
    assert_eq!(problem.graph_class().active(), [1, 3]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(problem.output().active().degrees().is_empty());
}

/// LLM: Regression test: verify that empty multiset and empty constraint have different
/// LLM: meanings. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn empty_multiset_and_empty_constraint_have_different_meanings() {
    // LLM: Prepare `empty` for the following computation; it stores the empty for this stage.
    let empty = Constraint::<LabelId>::new(vec![]);
    let zero = Constraint::<LabelId>::new(vec![CondensedConfiguration::new(vec![]).unwrap()]);
    assert_ne!(empty, zero);
    assert_eq!(zero.degrees()[0].degree(), 0);
    assert_eq!(zero.degrees()[0].configurations().len(), 1);
}

/// LLM: Regression test: verify that mapping must be total but may disallow every output. Used
/// LLM: by the test suite to protect this behavior while implementations change.
#[test]
fn mapping_must_be_total_but_may_disallow_every_output() {
    // LLM: Retain or prepare the original promised input semantics for this stage.
    let input = ConstraintPair::new(
        vec!["a".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(
        vec!["X".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    let graph = GraphClass::new(vec![1], vec![1]);
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for mapping in [vec![], vec![LabelSet::new(vec![LabelId(1)])]] {
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
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
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
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

/// LLM: Regression test: verify that ordinary constraint pairs validate passive references and
/// LLM: keep separate tables. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn ordinary_constraint_pairs_validate_passive_references_and_keep_separate_tables() {
    // LLM: Prepare `valid` for the following computation; it stores the valid for this stage.
    let valid = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(0, 1)]).unwrap(),
    ]);
    let invalid = Constraint::new(vec![
        CondensedConfiguration::new(vec![singleton(1, 1)]).unwrap(),
    ]);
    assert!(ConstraintPair::new(vec!["a".into()], Constraint::new(vec![]), invalid).is_err());
    // LLM: Separate input/output tables may use the same names.

    let input =
        ConstraintPair::new(vec!["A".into()], Constraint::new(vec![]), valid.clone()).unwrap();
    let output = ConstraintPair::new(vec!["A".into()], valid, Constraint::new(vec![])).unwrap();
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = Problem::new(
        GraphClass::new(vec![1], vec![1]),
        Input::Explicit(input),
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap();
    // LLM: Prepare `Input` for the following computation; it stores the input for this stage.
    let Input::Explicit(input) = problem.input() else {
        panic!()
    };
    assert_eq!(input.labels(), ["A"]);
    assert_eq!(problem.output().labels(), ["A"]);
}

/// LLM: Regression test: verify that serialized problems round trip without changing ids or
/// LLM: graph classes. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn serialized_problems_round_trip_without_changing_ids_or_graph_classes() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
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

/// LLM: Regression test: verify that restricted trivial input has one implicit entry. Used by
/// LLM: the test suite to protect this behavior while implementations change.
#[test]
fn restricted_trivial_input_has_one_implicit_entry() {
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(
        vec!["A".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    let graph = GraphClass::new(vec![1], vec![1]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        Problem::new(
            graph.clone(),
            Input::Trivial,
            output.clone(),
            OutputMapping::Restricted(vec![])
        )
        .is_err()
    );
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = Problem::new(
        graph,
        Input::Trivial,
        output,
        OutputMapping::Restricted(vec![LabelSet::default()]),
    )
    .unwrap();
    assert!(!problem.output_allowed(None, LabelId(0)));
}
