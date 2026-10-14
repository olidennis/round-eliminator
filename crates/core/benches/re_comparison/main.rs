//! LLM: Native release comparison. Build through run.py, not the production workspace.
//! LLM: Old library is a benchmark-only dependency; neither implementation is patched.

mod fixed_point;

#[cfg(all(feature = "jemalloc", feature = "mimalloc"))]
compile_error!("Select only one global allocator");

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use round_eliminator_3::{
    algorithms::{
        maximize::{MaximizationResult, maximize_constraint_with},
        round_elimination,
    },
    computation::{Cancellation, Computation},
    model::{
        CondensedConfiguration, Configuration, Constraint, ConstraintPair, GraphClass, Input,
        LabelId, LabelSet, OutputMapping, Part, Problem,
    },
    parse_problem,
};
use round_eliminator_lib::{
    algorithms::event::EventHandler, constraint::Constraint as OldConstraint, group::GroupType,
    problem::Problem as OldProblem,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    time::Instant,
};

/// LLM: A sorted set of counted configurations whose values are label-choice sets. The
/// LLM: benchmark aligns label IDs before comparing these rows, so stored row order and
/// LLM: duplicate rows cannot distort equivalence checks.
type Canonical = BTreeSet<Configuration<LabelSet>>;

/// LLM: Translate the original implementation's rows to the rewrite's condensed model using
/// LLM: the supplied ID map. Used outside timing to compare equivalent benchmark results.
fn converted_constraint(old: &OldConstraint, ids: &BTreeMap<u32, LabelId>) -> Constraint {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Constraint::new(
        old.lines
            .iter()
            .map(|line| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                CondensedConfiguration::new(
                    line.parts
                        .iter()
                        .map(|part| {
                            // LLM: Prepare `GroupType` for the following computation; it
                            // LLM: stores the group type for this stage.
                            let GroupType::Many(count) = part.gtype else {
                                panic!("fixture must have finite degrees")
                            };
                            Part::new(
                                LabelSet::new(part.group.iter().map(|id| ids[id]).collect()),
                                u32::from(count),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect(),
    )
}

/// LLM: Build the rewrite fixture from an original problem and retain the original-to-new ID
/// LLM: map. Used by RE comparison benchmarks before measurement.
fn converted_problem(old: &OldProblem) -> (Problem, BTreeMap<u32, LabelId>) {
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let names: BTreeMap<_, _> = old.mapping_label_text.iter().cloned().collect();
    let ids: BTreeMap<_, _> = names
        .keys()
        .enumerate()
        .map(|(index, &id)| (id, LabelId(index as u32)))
        .collect();
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(
        names.into_values().collect(),
        converted_constraint(&old.active, &ids),
        converted_constraint(&old.passive, &ids),
    )
    .unwrap();
    // LLM: Update the current working state before continuing this operation.
    (
        Problem::new(
            GraphClass::new(
                vec![match old.active.degree {
                    round_eliminator_lib::line::Degree::Finite(d) => d as u32,
                    _ => panic!("finite benchmark degree"),
                }],
                vec![match old.passive.degree {
                    round_eliminator_lib::line::Degree::Finite(d) => d as u32,
                    _ => panic!("finite benchmark degree"),
                }],
            ),
            Input::Trivial,
            output,
            OutputMapping::Unrestricted,
        )
        .unwrap(),
        ids,
    )
}

/// LLM: Convert condensed rows to a deterministic set of counted choice sets. Used by
/// LLM: benchmark equivalence checks independently of stored row order.
fn canonical(constraint: &Constraint) -> Canonical {
    // LLM: Update the current working state before continuing this operation.
    constraint
        .configurations()
        .map(|configuration| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            Configuration::new(
                configuration
                    .parts()
                    .iter()
                    .map(|part| (part.labels().clone(), part.multiplicity()))
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

/// LLM: Collect distinct output sets appearing in a computed maximum in sorted order. Used to
/// LLM: align the two implementations' RE set-label alphabets.
fn output_sets(maximum: &Constraint) -> Vec<LabelSet> {
    // LLM: Update the current working state before continuing this operation.
    maximum
        .configurations()
        .flat_map(|row| row.parts())
        .map(|part| part.labels().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// LLM: Verify that an original and rewritten RE result have the same active/passive
/// LLM: constraints after aligning new set labels. Used outside benchmark timing.
fn check_step(
    old: &OldProblem,
    new: &Problem,
    maximum: &Constraint,
    source_ids: &BTreeMap<u32, LabelId>,
) {
    // LLM: Collect canonical working sets before assigning deterministic local IDs.
    let sets = output_sets(maximum);
    let ids: BTreeMap<_, _> = old
        .mapping_label_oldlabels
        .as_ref()
        .unwrap()
        .iter()
        .map(|(id, old_set)| {
            let set = LabelSet::new(old_set.iter().map(|id| source_ids[id]).collect());
            (
                *id,
                LabelId(sets.binary_search(&set).expect("same set-label alphabet") as u32),
            )
        })
        .collect();
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(ids.len(), sets.len());
    assert_eq!(
        canonical(&converted_constraint(&old.active, &ids)),
        canonical(new.output().active())
    );
    // LLM: Check the expected result and invariants for this fixture, including the relevant
    // LLM: edge case.
    assert_eq!(
        canonical(&converted_constraint(&old.passive, &ids)),
        canonical(new.output().passive())
    );
}

/// LLM: Describe the rewrite fixture's alphabet, row counts, and degree lists as JSON. Used
/// LLM: alongside timings to identify benchmark output size.
fn stats(problem: &Problem) -> serde_json::Value {
    // LLM: Update the current working state before continuing this operation.
    serde_json::json!({"labels":problem.output().labels().len(),
        "active_rows":problem.output().active().configurations().count(),
        "passive_rows":problem.output().passive().configurations().count(),
        "active_degrees":problem.graph_class().active(), "passive_degrees":problem.graph_class().passive()})
}

/// LLM: Describe the original fixture's alphabet and row counts as JSON. Used alongside
/// LLM: comparison timings without affecting either algorithm.
fn old_stats(problem: &OldProblem) -> serde_json::Value {
    // LLM: Update the current working state before continuing this operation.
    serde_json::json!({"labels":problem.mapping_label_text.len(),
        "active_rows":problem.active.lines.len(), "passive_rows":problem.passive.lines.len()})
}

/// LLM: Write and flush one benchmark JSON record. Used by the comparison runner so
/// LLM: long-running measurements expose completed samples immediately.
fn emit(value: serde_json::Value) {
    // LLM: Update the current working state before continuing this operation.
    println!("{value}");
    std::io::stdout().flush().unwrap();
}

/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Match the old non-Linux server's setting using its locked mimalloc build.

    #[cfg(feature = "mimalloc")]
    // LLM: Update the current working state before continuing this operation.
    unsafe {
        libmimalloc_sys::mi_option_set(26, 0)
    }
    let args: Vec<_> = std::env::args().collect();
    let case = &args[1];
    let mode = &args[2];
    let steps: usize = args[3].parse().unwrap();
    let fixture = std::fs::read_to_string(&args[4]).unwrap();
    // LLM: Initialise the shared Rayon pool outside timing. Old maximization creates
    // LLM: its own per-frontier threads, which remains part of that implementation.

    rayon::ThreadPoolBuilder::new().build_global().unwrap();
    if mode == "fixed-point-new" || mode == "fixed-point-old" {
        fixed_point::run(&fixture, mode, steps, &args[5]);
        return;
    }
    // LLM: Reject an infeasible capacity state before exploring its completions.
    if mode == "resource-new" || mode == "resource-old" {
        // LLM: Prepare `saved` for the following computation; it stores the saved for this
        // LLM: stage.
        let saved: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let problem: Problem = serde_json::from_value(saved["new"].clone()).unwrap();
        let old: OldProblem = serde_json::from_value(saved["old"].clone()).unwrap();
        let (_, source_ids) = converted_problem(&old);
        let mut final_maximum = None;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for iteration in 1..=steps {
            // LLM: Connect phase/work updates to the caller while retaining cooperative
            // LLM: cancellation.
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            // LLM: Update the current working state before continuing this operation.
            let (maximum, milliseconds) = if mode == "resource-old" {
                // LLM: Establish the feasible bound for this local search before exploring or
                // LLM: allocating alternatives.
                let mut maximum = old.passive.clone();
                maximum.is_maximized = false;
                let start = Instant::now();
                maximum.maximize(&mut EventHandler::null());
                let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
                (converted_constraint(&maximum, &source_ids), milliseconds)
            } else {
                let start = Instant::now();
                let maximum =
                    maximize_constraint_with(problem.output().passive(), &mut computation).unwrap();
                (maximum, start.elapsed().as_secs_f64() * 1000.0)
            };
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            emit(
                serde_json::json!({"case":case,"mode":mode,"iteration":iteration,
                "milliseconds":milliseconds,"maxima":maximum.configurations().count()}),
            );
            final_maximum = Some(canonical(&maximum));
        }
        // LLM: Result dumping is outside kernel timing. The resource runner compares
        // LLM: complete maxima across worker counts, rather than just row counts.

        std::fs::write(
            &args[5],
            serde_json::to_vec(&final_maximum.unwrap()).unwrap(),
        )
        .unwrap();
        return;
    }
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if mode == "profile-old" || mode == "profile-new" {
        // LLM: Prepare `saved` for the following computation; it stores the saved for this
        // LLM: stage.
        let saved: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let old: OldProblem = serde_json::from_value(saved["old"].clone()).unwrap();
        let new: Problem = serde_json::from_value(saved["new"].clone()).unwrap();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for iteration in 1..=steps {
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let mut input = old.passive.clone();
            input.is_maximized = false;
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            let start = Instant::now();
            // LLM: Prepare `count` for the following computation; it stores the count for this
            // LLM: stage.
            let count = if mode == "profile-old" {
                input.maximize(&mut EventHandler::null());
                std::hint::black_box(input).lines.len()
            } else {
                std::hint::black_box(
                    maximize_constraint_with(new.output().passive(), &mut computation).unwrap(),
                )
                .configurations()
                .count()
            };
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            emit(
                serde_json::json!({"case":case,"mode":mode,"iteration":iteration,
                "milliseconds":start.elapsed().as_secs_f64()*1000.0,"maxima":count}),
            );
        }
        // LLM: Update the current working state before continuing this operation.
        return;
    }
    // LLM: Update the current working state before continuing this operation.
    let (active, passive) = fixture.trim().split_once("\n\n").unwrap();
    let mut old = OldProblem::from_string(&fixture).unwrap();
    let mut new = parse_problem(
        serde_json::from_value(serde_json::json!({"kind":"plain",
        "constraints":{"active":active,"passive":passive}}))
        .unwrap(),
    )
    .unwrap();
    // LLM: Share the event consumer only for the duration of each synchronous callback.
    let mut events = EventHandler::null();
    if mode == "snapshot" {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for _ in 1..steps {
            old = old.speedup(&mut events);
        }
        let (converted, _) = converted_problem(&old);
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        std::fs::write(
            &args[5],
            serde_json::to_vec(&serde_json::json!({"old":old,"new":converted})).unwrap(),
        )
        .unwrap();
        return;
    }
    // LLM: Prepare `elapsed` for the following computation; it stores the elapsed for this
    // LLM: stage.
    let mut elapsed = 0.0;
    if mode == "old-button" || mode == "maximize-cleaned" {
        // LLM: The old Start action prepares the original card before any click.

        round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
    }
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for step in 1..=steps {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        emit(serde_json::json!({"event":"starting", "case":case,"mode":mode,"step":step}));
        if mode.starts_with("maximize") {
            // LLM: Both kernels receive the SAME passive family, same label IDs,
            // LLM: same repetitions. Conversion/input cloning are outside timing.

            let (input, source_ids) = converted_problem(&old);
            let mut old_maximum = old.passive.clone();
            old_maximum.is_maximized = false;
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            // LLM: Prepare `sample` for the following computation; it stores the sample for
            // LLM: this stage.
            let sample: usize = std::env::var("RE_BENCH_SAMPLE")
                .unwrap_or_else(|_| "1".into())
                .parse()
                .unwrap();
            // LLM: Prepare `measure_old` for the following computation; it stores the measure
            // LLM: old for this stage.
            let mut measure_old = || {
                let start = Instant::now();
                old_maximum.maximize(&mut events);
                start.elapsed().as_secs_f64() * 1000.0
            };
            // LLM: Prepare `measure_new` for the following computation; it stores the measure
            // LLM: new for this stage.
            let mut measure_new = || {
                let start = Instant::now();
                let maximum =
                    maximize_constraint_with(input.output().passive(), &mut computation).unwrap();
                (maximum, start.elapsed().as_secs_f64() * 1000.0)
            };
            // LLM: Update the current working state before continuing this operation.
            let (old_ms, (new_maximum, new_ms)) = if sample % 2 == 1 {
                (measure_old(), measure_new())
            } else {
                let new = measure_new();
                (measure_old(), new)
            };
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                canonical(&converted_constraint(&old_maximum, &source_ids)),
                canonical(&new_maximum)
            );
            // LLM: Also validate both COMPLETE steps after set-label alignment, outside
            // LLM: timing. The old reference determines the next shared input.

            let mut prepared = old.clone();
            prepared.passive = old_maximum;
            let old_result = prepared.speedup(&mut events);
            // LLM: Prepare `new_result` for the following computation; it stores the new
            // LLM: result for this stage.
            let new_result = round_elimination::apply_with(
                &input,
                &MaximizationResult::OutputOnly(new_maximum),
                &mut computation,
            )
            .unwrap();
            let old_maximum = converted_constraint(&prepared.passive, &source_ids);
            check_step(&old_result, &new_result, &old_maximum, &source_ids);
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            emit(
                serde_json::json!({"event":"measured", "case":case,"mode":mode,"step":step,
                "old_maximize_ms":old_ms,"new_maximize_ms":new_ms,"input":stats(&input),
                "output":stats(&new_result),"equivalent":true}),
            );
            old = old_result;
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if mode == "maximize-cleaned" {
                round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
            }
            continue;
        }
        // LLM: Retain or prepare the original promised input semantics for this stage.
        let input = if mode == "new-re" {
            stats(&new)
        } else {
            old_stats(&old)
        };
        let start = Instant::now();
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if mode == "new-re" {
            new = round_elimination::apply(std::hint::black_box(&new)).unwrap();
        } else {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if mode == "old-button" && old.diagram_indirect.is_none() {
                old.compute_partial_diagram(&mut events);
            }
            old = old.speedup(&mut events);
            if mode == "old-button" {
                round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
            }
        }
        // LLM: Prepare `ms` for the following computation; it stores the ms for this stage.
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        elapsed += ms;
        let output = if mode == "new-re" {
            stats(&new)
        } else {
            old_stats(&old)
        };
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        emit(
            serde_json::json!({"event":"measured", "case":case,"mode":mode,"step":step,
            "milliseconds":ms,"cumulative_ms":elapsed,"input":input,"output":output}),
        );
    }
}
