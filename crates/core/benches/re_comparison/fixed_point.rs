//! Benchmark adapter for the old Basic procedure, using its exact lattice and seeds.
//! This is not a production fixed-point implementation.

use super::{Configuration, LabelSet, OldConstraint, OldProblem, emit};
use round_eliminator_3::{
    algorithms::maximize::{Operations, maximize_custom_with},
    computation::{Cancellation, Computation},
};
use round_eliminator_lib::{
    algorithms::{diagram::diagram_indirect_to_reachability_adj, event::EventHandler},
    group::{Group, GroupType},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    time::Instant,
};

struct Algebra {
    successors: HashMap<u32, HashSet<u32>>,
    joins: HashMap<(u32, u32), u32>,
    meets: HashMap<(u32, u32), u32>,
}

impl Operations<u32> for Algebra {
    fn union(&self, a: &u32, b: &u32) -> u32 {
        self.joins[&(*a, *b)]
    }
    fn intersection(&self, a: &u32, b: &u32) -> u32 {
        self.meets[&(*a, *b)]
    }
    fn is_superset(&self, a: &u32, b: &u32) -> bool {
        self.successors[b].contains(a)
    }
    // All lattice values, including the ID for the empty closed set, are valid.
}

fn algebra(sets: &BTreeMap<u32, BTreeSet<u32>>, diagram: &Vec<(u32, u32)>, dual: bool) -> Algebra {
    let ids: Vec<_> = sets.keys().copied().collect();
    let lookup: BTreeMap<_, _> = sets.iter().map(|(&id, set)| (set.clone(), id)).collect();
    let edges = if dual {
        diagram.clone()
    } else {
        diagram.iter().map(|&(a, b)| (b, a)).collect()
    };
    let mut joins = HashMap::new();
    let mut meets = HashMap::new();
    for (&a, x) in sets {
        for (&b, y) in sets {
            let intersection: BTreeSet<_> = x.intersection(y).copied().collect();
            let union: BTreeSet<_> = x.union(y).copied().collect();
            joins.insert((a, b), lookup[if dual { &intersection } else { &union }]);
            meets.insert((a, b), lookup[if dual { &union } else { &intersection }]);
        }
    }
    Algebra {
        successors: diagram_indirect_to_reachability_adj(&ids, &edges),
        joins,
        meets,
    }
}

fn initial(source: &OldConstraint, mapping: &HashMap<u32, u32>) -> OldConstraint {
    OldConstraint {
        lines: source.all_choices(true),
        degree: source.degree,
        is_maximized: false,
    }
    .edited(|group| Group::from(vec![mapping[&group.first()]]))
}

fn counted(constraint: &OldConstraint) -> Vec<Configuration<u32>> {
    constraint
        .lines
        .iter()
        .map(|line| {
            Configuration::new(
                line.parts
                    .iter()
                    .map(|part| {
                        assert_eq!(part.group.len(), 1);
                        let GroupType::Many(n) = part.gtype else {
                            panic!("finite fixed-point fixture");
                        };
                        (part.group.first(), u32::from(n))
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn saturate_old(initial: &OldConstraint, algebra: &Algebra) -> (Vec<Configuration<u32>>, f64) {
    let mut result = initial.clone();
    let start = Instant::now();
    result.maximize_custom(
        &mut EventHandler::null(),
        true,
        false,
        None,
        |a, b| algebra.successors[&b.first()].contains(&a.first()),
        |a, b| Group::from(vec![algebra.joins[&(a.first(), b.first())]]),
        |a, b| Group::from(vec![algebra.meets[&(a.first(), b.first())]]),
    );
    let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
    (counted(&result), milliseconds)
}

fn saturate_new(
    initial: &[Configuration<u32>],
    algebra: &Algebra,
) -> (Vec<Configuration<u32>>, f64) {
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    let start = Instant::now();
    let result = maximize_custom_with(initial, algebra, &mut computation, None).unwrap();
    (result, start.elapsed().as_secs_f64() * 1000.0)
}

pub(super) fn run(text: &str, mode: &str, repetitions: usize, output_path: &str) {
    let preparing = Instant::now();
    let mut problem = OldProblem::from_string(text).unwrap();
    problem.compute_diagram(&mut EventHandler::null());
    problem.compute_default_fixpoint_diagram(None, false, Vec::new(), &mut EventHandler::null());
    let fd = serde_json::to_value(&problem.fixpoint_diagram.as_ref().unwrap().1).unwrap();
    let closed: Vec<(Vec<u32>, u32)> =
        serde_json::from_value(fd["mapping_rightclosed_newlabel"].clone()).unwrap();
    let sets: BTreeMap<_, _> = closed
        .into_iter()
        .map(|(set, id)| (id, set.into_iter().collect()))
        .collect();
    assert!(sets.values().any(BTreeSet::is_empty));
    let mapped: Vec<(u32, u32)> =
        serde_json::from_value(fd["mapping_label_newlabel"].clone()).unwrap();
    let mapping: HashMap<_, _> = mapped.into_iter().collect();
    let diagram: Vec<(u32, u32)> = serde_json::from_value(fd["diagram"].clone()).unwrap();
    let active = initial(&problem.active, &mapping);
    let passive = initial(&problem.passive, &mapping);
    let new_active = counted(&active);
    let new_passive = counted(&passive);
    let active_algebra = algebra(&sets, &diagram, true);
    let passive_algebra = algebra(&sets, &diagram, false);
    let preparing_ms = preparing.elapsed().as_secs_f64() * 1000.0;
    emit(
        serde_json::json!({"event":"prepared", "lattice_labels":sets.len(),
        "active_seeds":active.lines.len(),"passive_seeds":passive.lines.len(),
        "preparing_ms":preparing_ms}),
    );
    let mut final_result = None;
    for iteration in 1..=repetitions {
        emit(serde_json::json!({"event":"starting","mode":mode,"iteration":iteration}));
        let ((active, active_ms), (passive, passive_ms)) = if mode == "fixed-point-old" {
            (
                saturate_old(&active, &active_algebra),
                saturate_old(&passive, &passive_algebra),
            )
        } else {
            (
                saturate_new(&new_active, &active_algebra),
                saturate_new(&new_passive, &passive_algebra),
            )
        };
        let decode = |rows: Vec<Configuration<u32>>| -> BTreeSet<Configuration<LabelSet<u32>>> {
            rows.into_iter()
                .map(|row| {
                    Configuration::new(
                        row.multiplicities()
                            .iter()
                            .map(|(id, n)| (LabelSet::new(sets[id].iter().copied().collect()), *n))
                            .collect(),
                    )
                    .unwrap()
                })
                .collect()
        };
        let active = decode(active);
        let passive = decode(passive);
        emit(
            serde_json::json!({"event":"measured","iteration":iteration,"mode":mode,
            "milliseconds":active_ms+passive_ms,"active_ms":active_ms,"passive_ms":passive_ms,
            "active_rows":active.len(),"passive_rows":passive.len()}),
        );
        final_result = Some(serde_json::json!({"active":active,"passive":passive}));
    }
    std::fs::write(
        output_path,
        serde_json::to_vec(&final_result.unwrap()).unwrap(),
    )
    .unwrap();
}
