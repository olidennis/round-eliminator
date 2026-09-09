use super::*;

fn hard() -> Problem {
    let mut p = Problem::from_string(include_str!(
        "../../../../../../examples/fixpoint_sat/hard_nonexistence.txt"
    ))
    .unwrap();
    p.compute_diagram(&mut EventHandler::null());
    p
}

fn original_seeds(p: &Problem) -> (Vec<Vec<Term>>, Vec<Derivation>) {
    let terms = input_terms(p);
    let dags = terms
        .iter()
        .map(|row| Derivation {
            steps: vec![Step::Input(
                row.iter()
                    .map(|t| match t {
                        Term::Terminal(l) => *l,
                        _ => unreachable!(),
                    })
                    .collect(),
            )],
        })
        .collect();
    (terms, dags)
}

#[test]
fn profiles_agree_with_existing_oracle_on_known_intermediates() {
    let p = hard();
    let dag = super::super::super::instance::known_test_dag(&p);
    let values = dag
        .replay(&input_terms(&p), 4, &SearchControl::default())
        .unwrap();
    let mut oracle = NonexistenceOracle::new(&p);
    let mut checker = Compatibility::new(&p);
    for terms in values {
        for a in &terms {
            for b in &terms {
                let (x, y) = (checker.intern(a), checker.intern(b));
                assert_eq!(checker.compatible(x, y), oracle.terms_compatible(a, b));
            }
        }
    }
}

#[test]
fn wired_context_replays_nonzero_pivots_and_occurrences() {
    let p = hard();
    let control = SearchControl::default();
    let (seeds, dags) = original_seeds(&p);
    let mut job = Job::build(&p, &seeds, dags, None, &[], 0, &control)
        .unwrap()
        .unwrap();
    job.root = job
        .encoding
        .wired_step([0, 1], [vec![3, 0, 2, 1], vec![1, 3, 0, 2]], 2)
        .unwrap();
    assert_eq!(
        control
            .solve(2, &mut job.encoding.circuit.solver, None)
            .unwrap(),
        SolverResult::Sat
    );
    let model = job.encoding.circuit.solver.full_solution().unwrap();
    let actual = job
        .decode(&model)
        .unwrap()
        .replay(&seeds, 4, &control)
        .unwrap()
        .pop()
        .unwrap();
    let expected: Vec<_> = (0..4)
        .map(|i| {
            combine(
                seeds[0][[3, 0, 2, 1][i]].clone(),
                seeds[1][[1, 3, 0, 2][i]].clone(),
                if i == 2 {
                    Operation::Union
                } else {
                    Operation::Intersection
                },
            )
        })
        .collect();
    assert!(actual == expected);
    assert!(job
        .encoding
        .wired_step([0, 0], [vec![0, 0, 2, 3], vec![0, 1, 2, 3]], 0)
        .is_err());
}

#[test]
fn partial_models_are_retained_and_used_as_new_seeds() {
    let p = hard();
    let control = SearchControl::default();
    let (seeds, dags) = original_seeds(&p);
    let mut job = Job::build(&p, &seeds, dags, None, &[], 1, &control)
        .unwrap()
        .unwrap();
    let results = job
        .solve(3, 0, 2_000, &control, &mut EventHandler::null())
        .unwrap();
    assert!(
        !results.is_empty(),
        "Expected a partial derivation, not a complete certificate"
    );
    let mut engine = Engine::new(&p, &control).unwrap();
    for (dag, _) in &results {
        engine.import(dag, &control).unwrap();
    }
    assert!(engine.accepted > 0);
    let fragment = engine.pool.values().find(|f| !f.pinned).unwrap();
    let second = Job::build(
        &p,
        &[fragment.terms.clone()],
        vec![fragment.dag.clone()],
        None,
        &[],
        1,
        &control,
    )
    .unwrap()
    .unwrap();
    assert_eq!(second.provenance[0].steps.len(), fragment.dag.steps.len());
}

#[test]
fn bounded_feedback_and_cancel_remain_inconclusive() {
    let p = hard();
    let control = SearchControl::default();
    let mut engine = Engine::new(&p, &control).unwrap();
    let options = CertificateSearchOptions {
        max_steps: Some(1),
        conflict_limit: Some(1),
    };
    engine.turn = 8;
    assert!(!engine.enabled(&options));
    engine.turn = 0;
    control.stop();
    assert!(engine
        .tick(
            &p,
            &options,
            &mut NonexistenceOracle::new(&p),
            &control,
            &mut EventHandler::null()
        )
        .is_err());
    let invalid = Derivation {
        steps: vec![Step::Input(vec![999; 4])],
    };
    assert!(engine.import(&invalid, &SearchControl::default()).is_err());
}

#[test]
fn repairs_masked_internal_steps_of_the_supplied_certificate() {
    // A controlled reconstruction test, NOT blind certificate discovery.
    // Only original configurations are SAT leaves. Holes' old parents and
    // permutations are not constrained; ancestors keep their actual wiring.
    let p = hard();
    let control = SearchControl::default();
    let dag = super::super::super::instance::known_test_dag(&p);
    assert_eq!(dag.steps.len(), 26);
    for holes in [vec![24], vec![23, 24], vec![22, 23, 24], vec![5]] {
        let (seeds, dags) = original_seeds(&p);
        let mut job = Job::build(&p, &seeds, dags, Some(&dag), &holes, 0, &control)
            .unwrap()
            .unwrap();
        let pairs = job.encoding.pair_goals(job.root);
        let goal = job.encoding.circuit.and(pairs).unwrap();
        job.encoding
            .circuit
            .solver
            .set_limit(Limit::Conflicts(100_000));
        let started = std::time::Instant::now();
        let result = control
            .solve(2, &mut job.encoding.circuit.solver, Some(&[goal]))
            .unwrap();
        eprintln!(
            "masked {:?}: {:?}, {} variables, {:.3}s",
            holes,
            result,
            job.encoding.circuit.next_var,
            started.elapsed().as_secs_f64()
        );
        assert_eq!(result, SolverResult::Sat);
        let model = job.encoding.circuit.solver.full_solution().unwrap();
        let repaired = job.decode(&model).unwrap();
        let terms = repaired.replay(&seeds, 4, &control).unwrap().pop().unwrap();
        assert!(NonexistenceOracle::new(&p).check(&terms).is_some());
    }
}

#[test]
fn compact_preserves_shared_parents_and_rejects_cycles() {
    let p = hard();
    let control = SearchControl::default();
    let dag = super::super::super::instance::known_test_dag(&p);
    let small = compact(&dag, dag.steps.len() - 1).unwrap();
    let a = dag
        .replay(&input_terms(&p), 4, &control)
        .unwrap()
        .pop()
        .unwrap();
    let b = small
        .replay(&input_terms(&p), 4, &control)
        .unwrap()
        .pop()
        .unwrap();
    assert!(a == b);
    assert!(small.steps.len() <= dag.steps.len());
    let cycle = Derivation {
        steps: vec![Step::Combine {
            parents: [0, 0],
            permutations: [vec![0], vec![0]],
            pivot: 0,
        }],
    };
    assert!(compact(&cycle, 0).is_err());
}

#[test]
fn equal_profiles_do_not_identify_distinct_derivations() {
    let mut p = Problem::from_string("A B\nA C\nA D\n\nA A\nB B\nC C\nD D").unwrap();
    p.compute_diagram(&mut EventHandler::null());
    let (inputs, seeds) = original_seeds(&p);
    let control = SearchControl::default();
    let mut engine = Engine::new(&p, &control).unwrap();
    let mut ids = Vec::new();
    for pairs in [[[0, 1], [3, 2]], [[1, 2], [0, 3]]] {
        let mut dag = Derivation {
            steps: seeds.iter().map(|d| d.steps[0].clone()).collect(),
        };
        for parents in pairs {
            dag.steps.push(Step::Combine {
                parents,
                permutations: [vec![0, 1], vec![0, 1]],
                pivot: 1,
            });
        }
        let terms = dag
            .replay(&inputs, 2, &SearchControl::default())
            .unwrap()
            .pop()
            .unwrap();
        let id = engine.next_id;
        assert!(engine.retain(dag, terms, false));
        ids.push(id);
    }
    assert!(engine.pool[&ids[0]].key != engine.pool[&ids[1]].key);
    assert_eq!(engine.pool[&ids[0]].profile, engine.pool[&ids[1]].profile);
}

#[test]
fn default_archive_rotates_all_roots_instead_of_sampling_only_twenty_four() {
    let p = hard();
    let control = SearchControl::default();
    let dag = super::super::super::super::seeding::collect(&p, &mut EventHandler::null(), &control)
        .unwrap()
        .unwrap();
    let mut engine = Engine::new(&p, &control).unwrap();
    engine.seed_default(&dag, &control).unwrap();
    let count = engine.default_seed.as_ref().unwrap().1.len();
    assert!(count > 256);
    let mut visited = HashSet::new();
    for _ in 0..count.div_ceil(4) {
        let cursor = engine.default_seed.as_ref().unwrap().2;
        for j in 0..4 {
            visited.insert((cursor + j) % count);
        }
        engine.rotate_default_seed(&control).unwrap();
        assert!(engine.pool.len() <= POOL_SIZE);
    }
    assert_eq!(visited.len(), count);
    assert_eq!(engine.default_seed.as_ref().unwrap().1.len(), count);
    assert!(engine.accepted > 24);
}

#[test]
fn feedback_cache_uses_shared_credits_not_the_old_private_cap() {
    let p = hard();
    let control = SearchControl::default();
    let (seeds, dags) = original_seeds(&p);
    let mut engine = Engine::new(&p, &control).unwrap();
    let mut count = 0;
    while engine.saved_variables() <= 150_000 {
        let job = Job::build(&p, &seeds, dags.clone(), None, &[], 2, &control)
            .unwrap()
            .unwrap();
        let done = Completed {
            job: Some(job),
            found: Vec::new(),
            score: 0,
            turn: count,
            budget: 2_000,
        };
        engine
            .accept(
                done,
                &Default::default(),
                &mut NonexistenceOracle::new(&p),
                &control,
                &mut EventHandler::null(),
            )
            .unwrap();
        count += 1;
        assert_eq!(engine.saved_count(), count);
    }
    let task = engine.next_cached(1, &mut EventHandler::null()).unwrap();
    assert!(task.job.is_some());
    assert!(task.recipe.seeds.is_empty());
    assert!(task.recipe.provenance.is_empty());
    assert_eq!(task.budget, 4_000);
    assert_eq!(engine.saved_count(), count - 1);
}
