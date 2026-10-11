use super::*;

#[test]
fn input_projection_discards_duplicates_without_merging_distinct_outputs() {
    use crate::parser::{ConstraintText, DegreeText, ProblemText, parse_problem};

    let problem = parse_problem(ProblemText::Paired {
        degrees: DegreeText::default(),
        constraints: ConstraintText {
            active: "(r,B)^3\n(r,C)^3\n(g,A)^3\n(g,C)^3\n(b,A)^3\n(b,B)^3".into(),
            passive: "(r,B) (g,A)\n(r,C) (g,C)".into(),
        },
    })
    .unwrap();
    let Input::Explicit(input) = problem.input() else {
        panic!("paired entry has an explicit input projection");
    };
    assert_eq!(input.active().configurations_at_degree(3).len(), 3);
    assert_eq!(input.passive().configurations_at_degree(2).len(), 1);
    assert_eq!(problem.output().active().configurations_at_degree(3).len(), 6);
    assert_eq!(problem.output().passive().configurations_at_degree(2).len(), 2);
    assert_eq!(problem.graph_class().active(), &[3]);
    assert_eq!(problem.graph_class().passive(), &[2]);
}

#[test]
fn pair_projection_keeps_correlations_in_the_original_constraints() {
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
    let input = constraints.input_projection().unwrap();
    assert_eq!(input.labels(), constraints.input_labels());
    let projected = &input.active().degrees()[0].configurations()[0];
    assert_eq!(projected.degree(), 2);
    assert_eq!(
        projected.parts()[0].labels().iter().collect::<Vec<_>>(),
        [LabelId(0), LabelId(1)]
    );
    let original = constraints.active().degrees()[0].configurations()[0].parts()[0].labels();
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
