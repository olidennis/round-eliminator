use super::*;

fn singleton(id: u32, count: u32) -> Part {
    Part::new(LabelSet::new(vec![LabelId(id)]), count).unwrap()
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

    let pair = LabelPair {
        input: LabelId(0),
        output: LabelId(0),
    };
    let zero = Part::new(LabelSet::new(vec![pair]), 0).unwrap();
    assert!(
        CondensedConfiguration::new(vec![zero])
            .unwrap()
            .parts()
            .is_empty()
    );
}

#[test]
fn graph_class_can_require_degrees_with_no_output_configurations() {
    let output =
        ConstraintPair::new(vec![], Constraint::new(vec![]), Constraint::new(vec![])).unwrap();
    let problem = PlainProblem::new(GraphClass::new(vec![3, 1, 3], vec![2]), output);
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
fn pair_projection_keeps_correlations_in_the_original_constraints() {
    let pairs = LabelSet::new(vec![
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
            output: LabelId(1),
        },
    ]);
    let active = Constraint::new(vec![
        CondensedConfiguration::new(vec![Part::new(pairs.clone(), 2).unwrap()]).unwrap(),
    ]);
    let constraints = InputOutputConstraintPair::new(
        vec!["a".into(), "b".into()],
        vec!["X".into(), "Y".into()],
        active,
        Constraint::new(vec![]),
    )
    .unwrap();
    let graph_class = GraphClass::new(vec![1, 3], vec![2]);
    let problem = PairedProblem::new(graph_class.clone(), constraints).unwrap();
    assert_eq!(problem.graph_class(), &graph_class);
    assert_eq!(
        problem.input().labels(),
        problem.constraints().input_labels()
    );
    let projected = &problem.input().active().degrees()[0].configurations()[0];
    assert_eq!(projected.degree(), 2);
    assert_eq!(
        projected.parts()[0].labels().iter().collect::<Vec<_>>(),
        [LabelId(0), LabelId(1)]
    );
    let original =
        problem.constraints().active().degrees()[0].configurations()[0].parts()[0].labels();
    assert_eq!(original, &pairs);
    assert!(!original.contains(LabelPair {
        input: LabelId(1),
        output: LabelId(0)
    }));
}

#[test]
fn pair_ids_are_checked_against_their_respective_tables() {
    for pair in [
        LabelPair {
            input: LabelId(1),
            output: LabelId(0),
        },
        LabelPair {
            input: LabelId(0),
            output: LabelId(1),
        },
    ] {
        let active = Constraint::new(vec![
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![pair]), 1).unwrap()])
                .unwrap(),
        ]);
        assert!(
            InputOutputConstraintPair::new(
                vec!["a".into()],
                vec!["X".into()],
                active,
                Constraint::new(vec![]),
            )
            .is_err()
        );
    }
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
        assert!(MappedProblem::new(graph.clone(), input.clone(), output.clone(), mapping).is_err());
    }
    assert!(MappedProblem::new(graph, input, output, vec![LabelSet::default()]).is_ok());
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
    let problem = IndependentProblem::new(GraphClass::new(vec![1], vec![1]), input, output);
    assert_eq!(problem.input().labels(), ["A"]);
    assert_eq!(problem.output().labels(), ["A"]);
}

#[test]
fn input_output_constraint_pairs_check_both_tables_and_sides() {
    for (inputs, outputs) in [
        (vec!["".into()], vec![]),
        (vec!["a".into(), "a".into()], vec![]),
        (vec![], vec!["".into()]),
        (vec![], vec!["X".into(), "X".into()]),
    ] {
        assert!(
            InputOutputConstraintPair::new(
                inputs,
                outputs,
                Constraint::new(vec![]),
                Constraint::new(vec![]),
            )
            .is_err()
        );
    }
    for pair in [
        LabelPair {
            input: LabelId(1),
            output: LabelId(0),
        },
        LabelPair {
            input: LabelId(0),
            output: LabelId(1),
        },
    ] {
        let passive = Constraint::new(vec![
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![pair]), 1).unwrap()])
                .unwrap(),
        ]);
        assert!(
            InputOutputConstraintPair::new(
                vec!["a".into()],
                vec!["X".into()],
                Constraint::new(vec![]),
                passive,
            )
            .is_err()
        );
    }
    // Equal names in separate input and output domains remain valid.
    let constraints = InputOutputConstraintPair::new(
        vec!["A".into()],
        vec!["A".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    assert_eq!(constraints.input_labels(), ["A"]);
    assert_eq!(constraints.output_labels(), ["A"]);
}
