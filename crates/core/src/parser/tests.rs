use super::*;
use crate::model::LabelId;

fn text(active: &str, passive: &str) -> ConstraintText {
    ConstraintText {
        active: active.into(),
        passive: passive.into(),
    }
}

fn plain(active: &str, passive: &str) -> ProblemText {
    ProblemText::Plain {
        constraints: text(active, passive),
        degrees: DegreeText::default(),
    }
}

#[test]
fn existing_notation_shares_labels_and_preserves_degree_groups() {
    let problem = parse_problem(plain("M U^9\nP^10\n(foo) A", "M UP^9\nU^10")).unwrap();
    assert_eq!(problem.output().labels(), ["M", "U", "P", "foo", "A"]);
    assert_eq!(problem.graph_class().active(), [2, 10]);
    assert_eq!(problem.graph_class().passive(), [10]);
    assert_eq!(
        problem.output().active().degrees()[1]
            .configurations()
            .len(),
        2
    );
    let choices = &problem.output().passive().degrees()[0].configurations()[0].parts()[1];
    assert_eq!(
        choices.labels(),
        &LabelSet::new(vec![LabelId(1), LabelId(2)])
    );
    assert_eq!(choices.multiplicity(), 9);
}

#[test]
fn explicit_degrees_are_separate_from_configuration_degrees() {
    let problem = parse_problem(ProblemText::Plain {
        constraints: text("A\nA^3", "A^2"),
        degrees: DegreeText {
            active: "4, 1 4".into(),
            passive: "".into(),
        },
    })
    .unwrap();
    assert_eq!(problem.graph_class().active(), [1, 4]);
    assert_eq!(problem.graph_class().passive(), [2]);
    assert_eq!(problem.output().inferred_graph_class().active(), [1, 3]);
}

#[test]
fn independent_defaults_come_only_from_input_and_alphabets_are_separate() {
    let problem = parse_problem(ProblemText::Independent {
        input: text("a\na^3", "b^2"),
        output: text("b^4", "a"),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert_eq!(problem.graph_class().active(), [1, 3]);
    assert_eq!(problem.graph_class().passive(), [2]);
    assert_eq!(explicit(&problem).labels(), ["a", "b"]);
    assert_eq!(problem.output().labels(), ["b", "a"]);
}

#[test]
fn paired_notation_projects_inputs_and_deduplicates_pairs() {
    let problem = parse_problem(ProblemText::Paired {
        degrees: DegreeText::default(),
        constraints: text("(red,X)(red,Y)(red,X)^2\n(blue,Y)", "(red,Y)(blue,X)"),
    })
    .unwrap();
    assert_eq!(explicit(&problem).labels(), ["red", "blue"]);
    assert_eq!(
        problem.output().labels(),
        ["red,X", "red,Y", "blue,X", "blue,Y"]
    );
    assert_eq!(explicit(&problem).inferred_graph_class().active(), [1, 2]);
    assert_eq!(
        problem.graph_class(),
        &explicit(&problem).inferred_graph_class()
    );
    let parts = problem.output().active().degrees()[1].configurations()[0].parts();
    assert_eq!(parts[0].labels().len(), 2);
    let projected = &explicit(&problem).active().degrees()[1].configurations()[0].parts()[0];
    assert_eq!(projected.labels().len(), 1);
    assert_eq!(projected.multiplicity(), 2);
}

#[test]
fn mapped_problems_accept_empty_output_sets_and_mapping_only_labels() {
    let problem = parse_problem(ProblemText::Mapped {
        input: text("(red)\n(blue)^3", "(blue)^2"),
        output: text("X^4", "X"),
        mapping: "(blue) ->\n(red) -> X(Yellow)".into(),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert_eq!(problem.graph_class().active(), [1, 3]);
    assert_eq!(problem.graph_class().passive(), [2]);
    assert_eq!(problem.output().labels(), ["X", "Yellow"]);
    assert_eq!(restricted(&problem)[0].len(), 2);
    assert!(restricted(&problem)[1].is_empty());
}

#[test]
fn empty_outputs_and_degree_zero_are_representable() {
    let problem = parse_problem(plain("()", "")).unwrap();
    assert_eq!(problem.graph_class().active(), [0]);
    assert!(problem.graph_class().passive().is_empty());
    let problem = parse_problem(ProblemText::Independent {
        input: text("a^3", "a^2"),
        output: text("", ""),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert_eq!(problem.graph_class().active(), [3]);
    assert!(problem.output().active().degrees().is_empty());
}

#[test]
fn invalid_notation_reports_the_editor_and_line() {
    for invalid in ["A*", "A^", "(missing", "A^4294967296", "A^4294967295 B"] {
        let error = parse_problem(plain("A", &format!("A\n{invalid}"))).unwrap_err();
        assert_eq!(
            error.location,
            Some(Location {
                field: Field::Passive,
                line: Some(2)
            })
        );
    }
    let error = parse_problem(ProblemText::Independent {
        input: text("a*", "a"),
        output: text("A", "A"),
        degrees: DegreeText::default(),
    })
    .unwrap_err();
    assert_eq!(error.location.unwrap().field, Field::InputActive);

    for pair in ["(a)", "(,X)", "(a,)", "(a,X,Y)", "(a,X", "(a,X)*", "a,X"] {
        assert!(
            parse_problem(ProblemText::Paired {
                degrees: DegreeText::default(),
                constraints: text(pair, "(a,X)")
            })
            .is_err(),
            "{pair}"
        );
    }
}

#[test]
fn mapping_errors_have_mapping_locations() {
    for mapping in ["a -> A", "a -> A\na -> B\nb -> A", "ab -> A", "a A"] {
        let error = parse_problem(ProblemText::Mapped {
            input: text("a b", "ab"),
            output: text("A", "A"),
            mapping: mapping.into(),
            degrees: DegreeText::default(),
        })
        .unwrap_err();
        assert_eq!(error.location.unwrap().field, Field::Mapping);
    }
}

#[test]
fn mapping_delimiters_respect_parenthesized_names() {
    let problem = parse_problem(ProblemText::Mapped {
        input: text("(α->β)", "(α->β)"),
        output: text("(X->Y)", "(X->Y)"),
        mapping: "(α->β) -> (X->Y)".into(),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert_eq!(explicit(&problem).labels(), ["α->β"]);
    assert_eq!(problem.output().labels(), ["X->Y"]);
    assert_eq!(restricted(&problem)[0], LabelSet::new(vec![LabelId(0)]));

    let error = parse_problem(ProblemText::Mapped {
        input: text("a", "a"),
        output: text("A", "A"),
        mapping: "a -> A -> B".into(),
        degrees: DegreeText::default(),
    })
    .unwrap_err();
    assert_eq!(error.location.unwrap().field, Field::Mapping);
}

#[test]
fn invalid_degrees_are_errors() {
    for degrees in ["-1", "1.5", "*", "4294967296", ","] {
        let error = parse_problem(ProblemText::Plain {
            constraints: text("A", "A"),
            degrees: DegreeText {
                active: degrees.into(),
                passive: "".into(),
            },
        })
        .unwrap_err();
        assert_eq!(error.location.unwrap().field, Field::ActiveDegrees);
    }
}

#[test]
fn paired_degrees_can_be_explicit_or_inferred_on_each_side() {
    // Omitted degrees remain valid in the JSON protocol, including degree zero
    // and empty constraints. Inference resolves each side independently.
    for (degrees, active, passive) in [
        (serde_json::json!(null), vec![0, 1, 2], vec![2]),
        (serde_json::json!({"active": "4, 1 4"}), vec![1, 4], vec![2]),
        (serde_json::json!({"passive": "0"}), vec![0, 1, 2], vec![0]),
        (
            serde_json::json!({"active": "2", "passive": "3"}),
            vec![2],
            vec![3],
        ),
    ] {
        let mut request = serde_json::json!({
            "kind": "paired",
            "constraints": {"active": "(a,X)^0..2", "passive": "(a,X)^2"},
        });
        if !degrees.is_null() {
            request["degrees"] = degrees;
        }
        let problem = parse_problem(serde_json::from_value(request).unwrap()).unwrap();
        assert_eq!(problem.graph_class().active(), active);
        assert_eq!(problem.graph_class().passive(), passive);
        // Explicit restrictions do not remove configurations or change the promise.
        assert_eq!(
            explicit(&problem).inferred_graph_class().active(),
            [0, 1, 2]
        );
        assert_eq!(problem.output().active().configurations().count(), 3);
    }
    let empty = parse_problem(ProblemText::Paired {
        constraints: text("", "()"),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert!(empty.graph_class().active().is_empty());
    assert_eq!(empty.graph_class().passive(), [0]);
}

#[test]
fn invalid_paired_degrees_identify_the_degree_field() {
    for invalid in ["-1", "1.5", "*", "4294967296", ","] {
        for (degrees, field) in [
            (
                DegreeText {
                    active: invalid.into(),
                    passive: "".into(),
                },
                Field::ActiveDegrees,
            ),
            (
                DegreeText {
                    active: "".into(),
                    passive: invalid.into(),
                },
                Field::PassiveDegrees,
            ),
        ] {
            let error = parse_problem(ProblemText::Paired {
                constraints: text("(a,X)", "(a,X)"),
                degrees,
            })
            .unwrap_err();
            assert_eq!(error.location, Some(Location { field, line: None }));
        }
    }
}

#[test]
fn exponent_ranges_expand_inclusively_and_independently() {
    let ranged = parse_problem(plain("AB^1..2 C^2..3", "A^5..8")).unwrap();
    let explicit = parse_problem(plain(
        "AB C^2\nAB^2 C^2\nAB C^3\nAB^2 C^3",
        "A^5\nA^6\nA^7\nA^8",
    ))
    .unwrap();
    assert_eq!(ranged, explicit);
    let problem = ranged;
    assert_eq!(problem.graph_class().active(), [3, 4, 5]);
    assert_eq!(problem.graph_class().passive(), [5, 6, 7, 8]);
    assert_eq!(problem.output().active().configurations().count(), 4);
}

#[test]
fn zero_and_single_value_exponents_preserve_model_invariants() {
    assert_eq!(
        parse_problem(plain("A^0..2", "A^2..2")).unwrap(),
        parse_problem(plain("()\nA\nA^2", "A^2")).unwrap(),
    );
    let problem = parse_problem(plain("A^0", "A^0..0 B")).unwrap();
    assert_eq!(problem.graph_class().active(), [0]);
    assert!(
        problem
            .output()
            .active()
            .configurations()
            .next()
            .unwrap()
            .parts()
            .is_empty()
    );
    assert_eq!(problem.graph_class().passive(), [1]);
    assert_eq!(
        problem
            .output()
            .passive()
            .configurations()
            .next()
            .unwrap()
            .parts()
            .len(),
        1
    );
    assert!(parse_problem(plain("^0", "")).is_err());
}

#[test]
fn input_ranges_determine_degrees_for_independent_and_mapped_problems() {
    for request in [
        ProblemText::Independent {
            input: text("a^1..3", "a^0..1"),
            output: text("A^4..5", "A^2..3"),
            degrees: DegreeText::default(),
        },
        ProblemText::Mapped {
            input: text("a^1..3", "a^0..1"),
            output: text("A^4..5", "A^2..3"),
            mapping: "a -> A".into(),
            degrees: DegreeText::default(),
        },
    ] {
        let problem = parse_problem(request).unwrap();
        let graph = problem.graph_class();
        assert_eq!(graph.active(), [1, 2, 3]);
        assert_eq!(graph.passive(), [0, 1]);
    }
}

#[test]
fn pair_ranges_preserve_choices_and_input_projection() {
    let ranged = parse_problem(ProblemText::Paired {
        degrees: DegreeText::default(),
        constraints: text("(red,X)(red,Y)^0..2", "(blue,Y)^1..2"),
    })
    .unwrap();
    let explicit = parse_problem(ProblemText::Paired {
        degrees: DegreeText::default(),
        constraints: text(
            "()\n(red,X)(red,Y)\n(red,X)(red,Y)^2",
            "(blue,Y)\n(blue,Y)^2",
        ),
    })
    .unwrap();
    assert_eq!(ranged, explicit);
}

#[test]
fn invalid_ranges_report_the_source_line() {
    for invalid in [
        "A^2..1",
        "A^..2",
        "A^1..",
        "A^1...2",
        "A^1..2..3",
        "A^-1..2",
        "A^1..+2",
        "A^1..2.5",
        "A^1..4294967296",
        "A^4294967294..4294967295 B",
        "A^1..2^3",
        "A^1..*",
    ] {
        let error = parse_problem(plain("A", &format!("A\n{invalid}"))).unwrap_err();
        assert_eq!(
            error.location,
            Some(Location {
                field: Field::Passive,
                line: Some(2)
            }),
            "{invalid}"
        );
    }
    // A large exponent is valid if the range itself is small and the degree fits.
    assert!(parse_problem(plain("A^4294967294..4294967295", "")).is_ok());
}

#[test]
fn expansion_has_no_fixed_configuration_limit() {
    let problem = parse_problem(plain("A^1..101 B^1..100\n()", "")).unwrap();
    assert_eq!(problem.output().active().configurations().count(), 10_101);
}

fn explicit(problem: &Problem) -> &ConstraintPair {
    match problem.input() {
        Input::Explicit(input) => input,
        Input::Trivial => panic!("expected explicit inputs"),
    }
}

fn restricted(problem: &Problem) -> &[LabelSet] {
    match problem.mapping() {
        OutputMapping::Restricted(mapping) => mapping,
        OutputMapping::Unrestricted => panic!("expected a restricted mapping"),
    }
}

#[test]
fn plain_and_independent_use_compact_cases() {
    let problem = parse_problem(plain("A", "A^2")).unwrap();
    assert_eq!(problem.input(), &Input::Trivial);
    assert_eq!(problem.mapping(), &OutputMapping::Unrestricted);
    let independent = parse_problem(ProblemText::Independent {
        input: text("a", "a^2"),
        output: text("A", "A^2"),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert!(matches!(independent.input(), Input::Explicit(_)));
    assert_eq!(independent.mapping(), &OutputMapping::Unrestricted);
}

#[test]
fn paired_conversion_retains_condensed_counts_and_separate_pair_tags() {
    let problem = parse_problem(ProblemText::Paired {
        constraints: text("(a,X)(b,X)^4294967295", "(a,X) (b,X)"),
        degrees: DegreeText::default(),
    })
    .unwrap();
    assert_eq!(problem.output().labels(), ["a,X", "b,X"]);
    assert_eq!(
        restricted(&problem),
        [
            LabelSet::new(vec![LabelId(0)]),
            LabelSet::new(vec![LabelId(1)])
        ]
    );
    let parts = problem
        .output()
        .active()
        .configurations()
        .next()
        .unwrap()
        .parts();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].multiplicity(), u32::MAX);
    assert_eq!(parts[0].labels().len(), 2);
}
