//! LLM: Benchmark adapter for the old Basic procedure, using its exact lattice and seeds.
//! LLM: This is not a production fixed-point implementation.

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

/// LLM: The comparison benchmark's finite lattice operation tables and principal label images.
/// LLM: It supplies the same standard or dual algebra to the original and new fixed-point
/// LLM: engines.
struct Algebra {
    successors: HashMap<u32, HashSet<u32>>,
    joins: HashMap<(u32, u32), u32>,
    meets: HashMap<(u32, u32), u32>,
}

impl Operations<u32> for Algebra {
    /// LLM: Compute the join in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn union(&self, a: &u32, b: &u32) -> u32 {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.joins[&(*a, *b)]
    }
    /// LLM: Compute the meet in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn intersection(&self, a: &u32, b: &u32) -> u32 {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.meets[&(*a, *b)]
    }
    /// LLM: Test domination in this representation's chosen standard or dual order. Called by
    /// LLM: the generic pairwise closure and its pruning proofs.
    fn is_superset(&self, a: &u32, b: &u32) -> bool {
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        self.successors[b].contains(a)
    }
    // LLM: All lattice values, including the ID for the empty closed set, are valid.
}

/// LLM: Construct the finite operation algebra used by the original/new fixed-point
/// LLM: comparison. Used before timing either closure.
fn algebra(sets: &BTreeMap<u32, BTreeSet<u32>>, diagram: &Vec<(u32, u32)>, dual: bool) -> Algebra {
    // LLM: Build the local value-to-ID table so storage order and parallel completion cannot
    // LLM: change identity.
    let ids: Vec<_> = sets.keys().copied().collect();
    let lookup: BTreeMap<_, _> = sets.iter().map(|(&id, set)| (set.clone(), id)).collect();
    let edges = if dual {
        diagram.clone()
    } else {
        diagram.iter().map(|&(a, b)| (b, a)).collect()
    };
    let mut joins = HashMap::new();
    // LLM: Prepare `meets` for the following computation; it stores the meets for this stage.
    let mut meets = HashMap::new();
    for (&a, x) in sets {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (&b, y) in sets {
            let intersection: BTreeSet<_> = x.intersection(y).copied().collect();
            let union: BTreeSet<_> = x.union(y).copied().collect();
            joins.insert((a, b), lookup[if dual { &intersection } else { &union }]);
            meets.insert((a, b), lookup[if dual { &union } else { &intersection }]);
        }
    }
    // LLM: Update the current working state before continuing this operation.
    Algebra {
        successors: diagram_indirect_to_reachability_adj(&ids, &edges),
        joins,
        meets,
    }
}

/// LLM: Build the fixed-point benchmark's seeded active/passive configurations. Used outside
/// LLM: the timed closure to keep fixture preparation comparable.
fn initial(source: &OldConstraint, mapping: &HashMap<u32, u32>) -> OldConstraint {
    // LLM: Update the current working state before continuing this operation.
    OldConstraint {
        lines: source.all_choices(true),
        degree: source.degree,
        is_maximized: false,
    }
    .edited(|group| Group::from(vec![mapping[&group.first()]]))
}

/// LLM: Convert a benchmark fixture to counted algebra configurations. Used to give both
/// LLM: engines equivalent seeds without occurrence-permutation expansion.
fn counted(constraint: &OldConstraint) -> Vec<Configuration<u32>> {
    // LLM: Update the current working state before continuing this operation.
    constraint
        .lines
        .iter()
        .map(|line| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
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

/// LLM: Run the original repository's closure with the benchmark algebra. Used for a fair
/// LLM: fixed-point kernel comparison rather than whole-button timing.
fn saturate_old(initial: &OldConstraint, algebra: &Algebra) -> (Vec<Configuration<u32>>, f64) {
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let mut result = initial.clone();
    let start = Instant::now();
    // LLM: Update the current working state before continuing this operation.
    result.maximize_custom(
        &mut EventHandler::null(),
        true,
        false,
        None,
        |a, b| algebra.successors[&b.first()].contains(&a.first()),
        |a, b| Group::from(vec![algebra.joins[&(a.first(), b.first())]]),
        |a, b| Group::from(vec![algebra.meets[&(a.first(), b.first())]]),
    );
    // LLM: Prepare `milliseconds` for the following computation; it stores the milliseconds
    // LLM: for this stage.
    let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
    (counted(&result), milliseconds)
}

/// LLM: Run the rewrite's closure with the same benchmark algebra and seeds. Used alongside
/// LLM: the original closure for isolated fixed-point timings.
fn saturate_new(
    initial: &[Configuration<u32>],
    algebra: &Algebra,
) -> (Vec<Configuration<u32>>, f64) {
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    let start = Instant::now();
    let result = maximize_custom_with(initial, algebra, &mut computation, None).unwrap();
    (result, start.elapsed().as_secs_f64() * 1000.0)
}

/// LLM: Evaluate the prepared benchmark or operation fixture and return its result data. Used
/// LLM: by this module's harness rather than the application protocol.
pub(super) fn run(text: &str, mode: &str, repetitions: usize, output_path: &str) {
    // LLM: Prepare `preparing` for the following computation; it stores the preparing for this
    // LLM: stage.
    let preparing = Instant::now();
    let mut problem = OldProblem::from_string(text).unwrap();
    problem.compute_diagram(&mut EventHandler::null());
    problem.compute_default_fixpoint_diagram(None, false, Vec::new(), &mut EventHandler::null());
    let fd = serde_json::to_value(&problem.fixpoint_diagram.as_ref().unwrap().1).unwrap();
    let closed: Vec<(Vec<u32>, u32)> =
        serde_json::from_value(fd["mapping_rightclosed_newlabel"].clone()).unwrap();
    // LLM: Collect canonical working sets before assigning deterministic local IDs.
    let sets: BTreeMap<_, _> = closed
        .into_iter()
        .map(|(set, id)| (id, set.into_iter().collect()))
        .collect();
    assert!(sets.values().any(BTreeSet::is_empty));
    let mapped: Vec<(u32, u32)> =
        serde_json::from_value(fd["mapping_label_newlabel"].clone()).unwrap();
    let mapping: HashMap<_, _> = mapped.into_iter().collect();
    // LLM: Prepare `diagram` for the following computation; it stores the diagram for this
    // LLM: stage.
    let diagram: Vec<(u32, u32)> = serde_json::from_value(fd["diagram"].clone()).unwrap();
    let active = initial(&problem.active, &mapping);
    let passive = initial(&problem.passive, &mapping);
    let new_active = counted(&active);
    let new_passive = counted(&passive);
    let active_algebra = algebra(&sets, &diagram, true);
    let passive_algebra = algebra(&sets, &diagram, false);
    let preparing_ms = preparing.elapsed().as_secs_f64() * 1000.0;
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    emit(
        serde_json::json!({"event":"prepared", "lattice_labels":sets.len(),
        "active_seeds":active.lines.len(),"passive_seeds":passive.lines.len(),
        "preparing_ms":preparing_ms}),
    );
    let mut final_result = None;
    // LLM: Check each relevant row independently, preserving its exact degree and condensed
    // LLM: choices.
    for iteration in 1..=repetitions {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
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
        // LLM: Prepare `decode` for the following computation; it stores the decode for this
        // LLM: stage.
        let decode = |rows: Vec<Configuration<u32>>| -> BTreeSet<Configuration<LabelSet<u32>>> {
            // LLM: Update the current working state before continuing this operation.
            rows.into_iter()
                .map(|row| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
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
        // LLM: Prepare `active` for the following computation; it stores the active for this
        // LLM: stage.
        let active = decode(active);
        let passive = decode(passive);
        emit(
            serde_json::json!({"event":"measured","iteration":iteration,"mode":mode,
            "milliseconds":active_ms+passive_ms,"active_ms":active_ms,"passive_ms":passive_ms,
            "active_rows":active.len(),"passive_rows":passive.len()}),
        );
        final_result = Some(serde_json::json!({"active":active,"passive":passive}));
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    std::fs::write(
        output_path,
        serde_json::to_vec(&final_result.unwrap()).unwrap(),
    )
    .unwrap();
}
