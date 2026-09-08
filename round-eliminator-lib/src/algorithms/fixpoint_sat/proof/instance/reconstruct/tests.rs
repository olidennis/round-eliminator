use super::*;

fn atom(label: Label) -> Term {
    Term::Terminal(label)
}
fn expr(a: Term, b: Term, op: Operation) -> Term {
    Term::Expr(Box::new(a), Box::new(b), op)
}

#[test]
fn restores_idempotent_columns_and_independently_swapped_children() {
    let inputs = vec![
        vec![atom(0), atom(0), atom(1)],
        vec![atom(0), atom(1), atom(2)],
    ];
    let supplied = vec![
        atom(0),
        expr(atom(0), atom(2), Operation::Intersection),
        atom(1),
    ];
    // Pivot 0 was collapsed; the two meet projections also have different
    // printed shapes. Repeated labels must preserve separate occurrences.
    let restored = restore(&inputs, &supplied, &mut |_| true).unwrap().unwrap();
    assert!(synchronized(&restored));
    assert!(
        supplied.iter().map(canonical).collect::<Vec<_>>()
            == restored.iter().map(canonical).collect::<Vec<_>>()
    );
    KnownPlan::new(&inputs).derive(&restored).unwrap();
}

#[test]
fn exhaustive_small_raw_derivations_survive_reduction_and_reconstruction() {
    use itertools::Itertools;
    let inputs = vec![vec![atom(0), atom(1)], vec![atom(1), atom(2)]];
    for left in &inputs {
        for right in &inputs {
            for perm in (0..2).permutations(2) {
                for pivot in 0..2 {
                    let raw: Vec<_> = (0..2)
                        .map(|i| {
                            expr(
                                left[i].clone(),
                                right[perm[i]].clone(),
                                if i == pivot {
                                    Operation::Union
                                } else {
                                    Operation::Intersection
                                },
                            )
                        })
                        .collect();
                    let reduced: Vec<_> = raw.iter().map(canonical).collect();
                    let restored = restore(&inputs, &reduced, &mut |_| true).unwrap().unwrap();
                    assert!(synchronized(&restored));
                    KnownPlan::new(&inputs).derive(&restored).unwrap();
                    assert!(reduced == restored.iter().map(canonical).collect::<Vec<_>>());
                }
            }
        }
    }
}

#[test]
fn reconstruction_never_invents_input_lines_and_budget_stop_is_not_rejection() {
    let inputs = vec![vec![atom(0), atom(1)]];
    assert!(restore(&inputs, &[atom(0), atom(0)], &mut |_| true).is_err());
    let terms = vec![expr(atom(0), atom(1), Operation::Union), atom(0)];
    assert!(restore(&inputs, &terms, &mut |_| false).unwrap().is_none());
}
