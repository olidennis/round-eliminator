use super::*;

fn problem(text: &str) -> Problem {
    let mut p = Problem::from_string(text).unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

#[test]
fn fixed_term_circuit_matches_existing_oracle() {
    let original = problem("A A\nB B\nC C\n\nA AB\nB C");
    let control = SearchControl::default();
    let mut encoding = ProofEncoding::new(&original, &input_terms(&original), &control).unwrap();
    let mut oracle = NonexistenceOracle::new(&original);
    let atoms: Vec<_> = original.labels().into_iter().map(Term::Terminal).collect();
    let mut terms = atoms.clone();
    for a in &atoms {
        for b in &atoms {
            for op in [Operation::Union, Operation::Intersection] {
                terms.push(Term::Expr(Box::new(a.clone()), Box::new(b.clone()), op));
            }
        }
    }
    // Nested terms exercise both decomposition directions and repeated atoms.
    for i in 0..30 {
        let a = terms[(i * 7 + 1) % terms.len()].clone();
        let b = terms[(i * 11 + 2) % terms.len()].clone();
        terms.push(Term::Expr(
            Box::new(a),
            Box::new(b),
            if i % 2 == 0 {
                Operation::Union
            } else {
                Operation::Intersection
            },
        ));
    }
    for a in &terms {
        for b in &terms {
            let x = encoding.fixed_term(a).unwrap();
            let y = encoding.fixed_term(b).unwrap();
            let encoded = encoding.compatible(x, y) == encoding.circuit.truth;
            assert_eq!(encoded, oracle.terms_compatible(a, b));
        }
    }
}

#[test]
fn proof_bounds_are_not_diagram_existence_claims() {
    let original = problem("A A\nB B\n\nA B");
    assert!(matches!(
        original
            .fixpoint_certificate(
                &CertificateSearchOptions {
                    max_steps: Some(2),
                    ..Default::default()
                },
                &mut EventHandler::null()
            )
            .unwrap(),
        CertificateSearchOutcome::Exhausted { steps: 2 }
    ));
    let original = problem("A A\n\nA A");
    assert!(matches!(
        original
            .fixpoint_certificate(
                &CertificateSearchOptions {
                    max_steps: Some(0),
                    ..Default::default()
                },
                &mut EventHandler::null()
            )
            .unwrap(),
        CertificateSearchOutcome::Found { steps: 0, .. }
    ));
}

#[test]
fn synthesizes_nontrivial_certificate_without_a_diagram() {
    let original = problem(include_str!(
        "../../../../examples/fixpoint_sat/maximal_matching.txt"
    ));
    let outcome = original
        .fixpoint_certificate(
            &CertificateSearchOptions {
                max_steps: Some(4),
                ..Default::default()
            },
            &mut EventHandler::null(),
        )
        .unwrap();
    assert!(
        matches!(outcome, CertificateSearchOutcome::Found { steps: 1..=4, certificate, .. }
        if certificate.contains("Original expressions:"))
    );
}

#[test]
fn cancellation_during_encoding_and_solving_is_not_a_proof() {
    let original = problem("A A\nB B\n\nA B");
    let control = SearchControl::default();
    control.stop();
    assert!(run(
        &original,
        &Default::default(),
        &mut EventHandler::null(),
        &control,
        None
    )
    .is_err());
}

#[test]
fn variable_derivations_match_oracle_after_independent_replay() {
    let original = problem("A A B\nB B C\nA C C\n\nAB C\nA B");
    let control = SearchControl::default();
    let mut encoding = ProofEncoding::new(&original, &input_terms(&original), &control).unwrap();
    encoding.step().unwrap();
    encoding.step().unwrap();
    let mut oracle = NonexistenceOracle::new(&original);
    for _ in 0..100 {
        assert_eq!(encoding.circuit.solver.solve().unwrap(), SolverResult::Sat);
        let assignment = encoding.circuit.solver.full_solution().unwrap();
        let terms = encoding
            .replay(&assignment, encoding.tuples.len() - 1)
            .unwrap();
        let nodes = &encoding.tuples.last().unwrap().nodes;
        for a in 0..3 {
            for b in 0..3 {
                assert_eq!(
                    assignment.lit_value(encoding.compatible(nodes[a], nodes[b]))
                        == TernaryVal::True,
                    oracle.terms_compatible(&terms[a], &terms[b])
                );
            }
        }
        let mut block = Vec::new();
        for tuple in &encoding.tuples {
            if let Source::Combine(parents) = &tuple.source {
                for p in parents {
                    for &lit in p.tuple.iter().chain(p.permutation.iter().flatten()) {
                        if assignment.lit_value(lit) == TernaryVal::True {
                            block.push(!lit);
                        }
                    }
                }
            }
        }
        encoding.circuit.clause(block).unwrap();
    }
}

pub(super) fn known_terms(original: &Problem) -> Vec<Term> {
    fn parse(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        names: &HashMap<char, Label>,
    ) -> Term {
        if chars.peek() == Some(&'[') {
            chars.next();
            let a = parse(chars, names);
            let op = match chars.next().unwrap() {
                '→' => Operation::Union,
                '←' => Operation::Intersection,
                _ => panic!("Invalid arrow"),
            };
            let b = parse(chars, names);
            assert_eq!(chars.next(), Some(']'));
            Term::Expr(Box::new(a), Box::new(b), op)
        } else {
            Term::Terminal(names[&chars.next().unwrap()])
        }
    }
    let names: HashMap<_, _> = original
        .mapping_label_text
        .iter()
        .map(|(id, name)| (name.chars().next().unwrap(), *id))
        .collect();
    include_str!("known_certificate.txt")
        .split("Original expressions:\n")
        .nth(1)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let mut chars = line.chars().peekable();
            let term = parse(&mut chars, &names);
            assert!(chars.next().is_none());
            term
        })
        .collect()
}

#[test]
fn supplied_hard_certificate_is_in_the_derivation_grammar_and_passes_both_checkers() {
    let original = problem(include_str!(
        "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let terms = known_terms(&original);
    let inputs = input_terms(&original);
    fn verify(mut terms: Vec<Term>, inputs: &[Vec<Term>], seen: &mut HashSet<Vec<Term>>) {
        terms.sort();
        if inputs.contains(&terms) || !seen.insert(terms.clone()) {
            return;
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut unions = 0;
        for term in terms {
            match term {
                Term::Expr(a, b, op) => {
                    unions += usize::from(op == Operation::Union);
                    left.push(*a);
                    right.push(*b);
                }
                _ => panic!("Invalid active derivation leaf"),
            }
        }
        assert_eq!(
            unions, 1,
            "Each active combination must have exactly one union coordinate"
        );
        verify(left, inputs, seen);
        verify(right, inputs, seen);
    }
    verify(terms.clone(), &inputs, &mut HashSet::new());
    let control = SearchControl::default();
    let mut encoding = ProofEncoding::new(&original, &inputs, &control).unwrap();
    let nodes: Vec<_> = terms
        .iter()
        .map(|t| encoding.fixed_term(t).unwrap())
        .collect();
    for &a in &nodes {
        for &b in &nodes {
            assert_eq!(encoding.compatible(a, b), encoding.circuit.truth);
        }
    }
    assert!(NonexistenceOracle::new(&original).check(&terms).is_some());
}

#[test]
fn synthesizes_hard_certificate_from_two_valid_shared_subderivations() {
    let original = problem(include_str!(
        "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ));
    let terms = known_terms(&original);
    // Share only the two immediate subderivations, NOT the certificate itself.
    // The grammar test above independently checks their entire derivations.
    let mut children = [Vec::new(), Vec::new()];
    for term in terms {
        if let Term::Expr(a, b, _) = term {
            children[0].push(*a);
            children[1].push(*b);
        } else {
            panic!("Expected a compound certificate");
        }
    }
    let mut oracle = NonexistenceOracle::new(&original);
    assert!(children.iter().all(|terms| oracle.check(terms).is_none()));
    let (tx, rx) = std::sync::mpsc::sync_channel(2);
    for terms in children {
        tx.send(terms).unwrap();
    }
    drop(tx);
    let result = run(
        &original,
        &CertificateSearchOptions {
            max_steps: Some(1),
            conflict_limit: Some(100_000),
        },
        &mut EventHandler::null(),
        &SearchControl::default(),
        Some(&rx),
    )
    .unwrap();
    assert!(matches!(
        result,
        CertificateSearchOutcome::Found {
            steps: 1,
            shared_lines: 2,
            ..
        }
    ));
}
