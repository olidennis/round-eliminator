//! Optional clauses appended to the unchanged base benchmark encoding.
use super::*;

pub(super) fn apply(encoding: &mut ProofEncoding, options: &InstanceOptions) -> Result<()> {
    let inputs = encoding
        .tuples
        .iter()
        .take_while(|t| matches!(t.source, Source::Leaf(_)))
        .count();
    let steps = encoding.tuples.len() - inputs;
    let truth = encoding.circuit.truth;
    if let Some(limit) = options.max_depth {
        let limit = limit.min(steps);
        // ge[i][k] is a monotone upper rank: it must be true if tuple i has
        // depth >= k. Selected edges strictly increase the rank. Setting
        // ge[i][limit+1] false forbids long paths, without fixing any wiring.
        // Conversely actual depths provide a satisfying rank assignment for
        // every bounded DAG, so no bounded derivation is lost.
        let mut ge = Vec::<Vec<Lit>>::new();
        for i in 0..encoding.tuples.len() {
            let mut row = vec![truth];
            for k in 1..=limit {
                row.push(if i < inputs || k > i.saturating_sub(inputs) + 1 {
                    !truth
                } else if k == 1 {
                    truth
                } else {
                    encoding.circuit.literal()?
                });
            }
            row.push(!truth);
            for k in 2..=limit {
                encoding.circuit.clause([!row[k], row[k - 1]])?;
            }
            if let Source::Combine(parents) = &encoding.tuples[i].source {
                for parent in parents {
                    for (j, &selected) in parent.tuple.iter().enumerate() {
                        for k in 0..=limit {
                            encoding
                                .circuit
                                .clause([!selected, !ge[j][k], row[k + 1]])?;
                        }
                    }
                }
            }
            ge.push(row);
        }
    }
    if options.symmetry {
        let degree = encoding.degree;
        for i in inputs..encoding.tuples.len() {
            let Source::Combine(parents) = &encoding.tuples[i].source else {
                unreachable!()
            };
            // All coordinates except 0 use meet. Jointly reorder both input
            // permutations so the left parent's nonpivot columns increase.
            // Every later use has its own free permutation, so this loses no
            // proof. There are (degree-1)! such row orders before this cut.
            let left = &parents[0].permutation;
            for row in 1..degree.saturating_sub(1) {
                for a in 0..degree {
                    for b in 0..=a {
                        encoding
                            .circuit
                            .clause([!left[row][a], !left[row + 1][b]])?;
                    }
                }
            }
            // Identical atoms in original configurations are occurrences,
            // not distinguishable proof choices. Assign their source columns
            // in ascending output-row order, conditioned on that whole parent.
            for parent in parents {
                for t in 0..inputs {
                    let nodes = &encoding.tuples[t].nodes;
                    for a in 0..degree {
                        for b in a + 1..degree {
                            if nodes[a] != nodes[b] {
                                continue;
                            }
                            for first in 0..degree {
                                for second in first + 1..degree {
                                    encoding.circuit.clause([
                                        !parent.tuple[t],
                                        !parent.permutation[second][a],
                                        !parent.permutation[first][b],
                                    ])?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    fn assumptions(encoding: &ProofEncoding, steps: &[KnownStep]) -> Vec<Lit> {
        let inputs = encoding.tuples.len() - steps.len();
        let mut result = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let Source::Combine(parents) = &encoding.tuples[inputs + i].source else {
                unreachable!()
            };
            for side in 0..2 {
                result.push(parents[side].tuple[step.parents[side]]);
                for (row, &column) in step.permutations[side].iter().enumerate() {
                    result.push(parents[side].permutation[row][column]);
                }
            }
        }
        result
    }

    #[test]
    fn unary_depth_bound_matches_every_three_step_parent_dag() {
        let original = prepare(&Problem::from_string("A A\n\nA A").unwrap()).unwrap();
        for limit in 0..=3 {
            let options = InstanceOptions {
                max_depth: Some(limit),
                symmetry: false,
            };
            let control = SearchControl::default();
            let mut encoding = build_with_options(&original, 3, &control, &options).unwrap();
            for a in 0..2 {
                for b in a..2 {
                    for c in 0..3 {
                        for d in c..3 {
                            let mut depths = vec![0usize];
                            let steps: Vec<_> = [[0, 0], [a, b], [c, d]]
                                .into_iter()
                                .map(|parents| {
                                    depths.push(1 + depths[parents[0]].max(depths[parents[1]]));
                                    KnownStep {
                                        parents,
                                        permutations: [vec![0, 1], vec![0, 1]],
                                    }
                                })
                                .collect();
                            let choices = assumptions(&encoding, &steps);
                            let result = control
                                .solve(1, &mut encoding.circuit.solver, Some(&choices))
                                .unwrap();
                            assert_eq!(
                                result == SolverResult::Sat,
                                *depths.iter().max().unwrap() <= limit
                            );
                            if result == SolverResult::Sat {
                                let model = encoding.circuit.solver.full_solution().unwrap();
                                check_clauses(&encoding, &model).unwrap();
                                check_depth(&encoding, &model, &options).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_four_coordinate_combination_has_a_symmetric_representative() {
        let original =
            prepare(&Problem::from_string("A A B C\nA B C C\n\nABC ABC").unwrap()).unwrap();
        let inputs = input_terms(&original);
        let options = InstanceOptions {
            max_depth: Some(1),
            symmetry: true,
        };
        let control = SearchControl::default();
        let mut encoding = build_with_options(&original, 1, &control, &options).unwrap();
        let permutations: Vec<_> = (0..4).permutations(4).collect();
        let mut checked = 0;
        for left in 0..inputs.len() {
            for right in left..inputs.len() {
                for lp in &permutations {
                    for rp in &permutations {
                        let terms: Vec<_> = (0..4)
                            .map(|row| {
                                Term::Expr(
                                    Box::new(inputs[left][lp[row]].clone()),
                                    Box::new(inputs[right][rp[row]].clone()),
                                    if row == 0 {
                                        Operation::Union
                                    } else {
                                        Operation::Intersection
                                    },
                                )
                            })
                            .collect();
                        let mut plan = KnownPlan::new(&inputs);
                        plan.symmetry = true;
                        let root = plan.derive(&terms).unwrap();
                        assert!(tuple_key(&plan.values[root]) == tuple_key(&terms));
                        if plan.steps.is_empty() {
                            continue;
                        }
                        assert_eq!(plan.steps.len(), 1);
                        let choices = assumptions(&encoding, &plan.steps);
                        assert_eq!(
                            control
                                .solve(1, &mut encoding.circuit.solver, Some(&choices))
                                .unwrap(),
                            SolverResult::Sat
                        );
                        let model = encoding.circuit.solver.full_solution().unwrap();
                        check_clauses(&encoding, &model).unwrap();
                        let replay = encoding.replay(&model, encoding.tuples.len() - 1).unwrap();
                        assert!(tuple_key(&replay) == tuple_key(&terms));
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 500);
    }
}

/// Independently compute actual parent-chain lengths, not auxiliary ranks.
pub(super) fn check_depth(
    encoding: &ProofEncoding,
    model: &Assignment,
    options: &InstanceOptions,
) -> Result<()> {
    let Some(limit) = options.max_depth else {
        return Ok(());
    };
    let mut depths = Vec::new();
    for tuple in &encoding.tuples {
        let depth = match &tuple.source {
            Source::Leaf(_) => 0,
            Source::Combine(parents) => {
                let mut depth = 0;
                for parent in parents {
                    let selected: Vec<_> = parent
                        .tuple
                        .iter()
                        .enumerate()
                        .filter(|(_, lit)| model.lit_value(**lit) == TernaryVal::True)
                        .map(|(i, _)| i)
                        .collect();
                    if selected.len() != 1 {
                        return Err("Invalid depth-check parent".into());
                    }
                    depth = depth.max(depths[selected[0]] + 1);
                }
                depth
            }
        };
        if depth > limit {
            return Err("Decoded derivation exceeds the depth bound".into());
        }
        depths.push(depth);
    }
    Ok(())
}
