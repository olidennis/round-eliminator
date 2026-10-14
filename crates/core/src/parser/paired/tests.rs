use super::*;

/// LLM: Regression test: verify that input projection discards duplicates without merging
/// LLM: distinct outputs. Used by the test suite to protect this behavior while
/// LLM: implementations change.
#[test]
fn input_projection_discards_duplicates_without_merging_distinct_outputs() {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use crate::parser::{ConstraintText, DegreeText, ProblemText, parse_problem};

    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let problem = parse_problem(ProblemText::Paired {
        degrees: DegreeText::default(),
        constraints: ConstraintText {
            active: "(r,B)^3\n(r,C)^3\n(g,A)^3\n(g,C)^3\n(b,A)^3\n(b,B)^3".into(),
            passive: "(r,B) (g,A)\n(r,C) (g,C)".into(),
        },
    })
    .unwrap();
    // LLM: Prepare `Input` for the following computation; it stores the input for this stage.
    let Input::Explicit(input) = problem.input() else {
        panic!("paired entry has an explicit input projection");
    };
    assert_eq!(input.active().configurations_at_degree(3).len(), 3);
    assert_eq!(input.passive().configurations_at_degree(2).len(), 1);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        problem.output().active().configurations_at_degree(3).len(),
        6
    );
    assert_eq!(
        problem.output().passive().configurations_at_degree(2).len(),
        2
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(problem.graph_class().active(), &[3]);
    assert_eq!(problem.graph_class().passive(), &[2]);
}

/// LLM: Regression test: verify that pair projection keeps correlations in the original
/// LLM: constraints. Used by the test suite to protect this behavior while implementations
/// LLM: change.
#[test]
fn pair_projection_keeps_correlations_in_the_original_constraints() {
    // LLM: Prepare `pair` for the following computation; it stores the pair for this stage.
    let pair = LabelPair {
        input: LabelId(0),
        output: LabelId(0),
    };
    let zero = Part::new(LabelSet::new(vec![pair]), 0).unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert!(
        CondensedConfiguration::new(vec![zero])
            .unwrap()
            .parts()
            .is_empty()
    );

    // LLM: Queue the parent pairs for this bounded generation batch.
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
    // LLM: Prepare `active` for the following computation; it stores the active for this
    // LLM: stage.
    let active = Constraint::new(vec![
        CondensedConfiguration::new(vec![Part::new(pairs.clone(), 2).unwrap()]).unwrap(),
    ]);
    // LLM: Prepare `constraints` for the following computation; it stores the constraints for
    // LLM: this stage.
    let constraints = InputOutputConstraintPair::new(
        vec!["a".into(), "b".into()],
        vec!["X".into(), "Y".into()],
        active,
        Constraint::new(vec![]),
    )
    .unwrap();
    let input = constraints.input_projection().unwrap();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(input.labels(), constraints.input_labels());
    let projected = &input.active().degrees()[0].configurations()[0];
    assert_eq!(projected.degree(), 2);
    assert_eq!(
        projected.parts()[0].labels().iter().collect::<Vec<_>>(),
        [LabelId(0), LabelId(1)]
    );
    let original = constraints.active().degrees()[0].configurations()[0].parts()[0].labels();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(original, &pairs);
    assert!(!original.contains(LabelPair {
        input: LabelId(1),
        output: LabelId(0)
    }));
}

/// LLM: Regression test: verify that pair ids are checked against their respective tables.
/// LLM: Used by the test suite to protect this behavior while implementations change.
#[test]
fn pair_ids_are_checked_against_their_respective_tables() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
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
        // LLM: Prepare `active` for the following computation; it stores the active for this
        // LLM: stage.
        let active = Constraint::new(vec![
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![pair]), 1).unwrap()])
                .unwrap(),
        ]);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
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

/// LLM: Regression test: verify that input output constraint pairs check both tables and
/// LLM: sides. Used by the test suite to protect this behavior while implementations change.
#[test]
fn input_output_constraint_pairs_check_both_tables_and_sides() {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (inputs, outputs) in [
        (vec!["".into()], vec![]),
        (vec!["a".into(), "a".into()], vec![]),
        (vec![], vec!["".into()]),
        (vec![], vec!["X".into(), "X".into()]),
    ] {
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
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
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
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
        // LLM: Prepare `passive` for the following computation; it stores the passive for this
        // LLM: stage.
        let passive = Constraint::new(vec![
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![pair]), 1).unwrap()])
                .unwrap(),
        ]);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
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
    // LLM: Equal names in separate input and output domains remain valid.

    let constraints = InputOutputConstraintPair::new(
        vec!["A".into()],
        vec!["A".into()],
        Constraint::new(vec![]),
        Constraint::new(vec![]),
    )
    .unwrap();
    assert_eq!(constraints.input_labels(), ["A"]);
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(constraints.output_labels(), ["A"]);
}
