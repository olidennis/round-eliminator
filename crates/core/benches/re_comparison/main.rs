//! Native release comparison. Build through run.py, not the production workspace.
//! Old library is a benchmark-only dependency; neither implementation is patched.

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

type Canonical = BTreeSet<Configuration<LabelSet>>;

fn converted_constraint(old: &OldConstraint, ids: &BTreeMap<u32, LabelId>) -> Constraint {
    Constraint::new(
        old.lines
            .iter()
            .map(|line| {
                CondensedConfiguration::new(
                    line.parts
                        .iter()
                        .map(|part| {
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

fn converted_problem(old: &OldProblem) -> (Problem, BTreeMap<u32, LabelId>) {
    let names: BTreeMap<_, _> = old.mapping_label_text.iter().cloned().collect();
    let ids: BTreeMap<_, _> = names
        .keys()
        .enumerate()
        .map(|(index, &id)| (id, LabelId(index as u32)))
        .collect();
    let output = ConstraintPair::new(
        names.into_values().collect(),
        converted_constraint(&old.active, &ids),
        converted_constraint(&old.passive, &ids),
    )
    .unwrap();
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

fn canonical(constraint: &Constraint) -> Canonical {
    constraint
        .configurations()
        .map(|configuration| {
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

fn output_sets(maximum: &Constraint) -> Vec<LabelSet> {
    maximum
        .configurations()
        .flat_map(|row| row.parts())
        .map(|part| part.labels().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn check_step(
    old: &OldProblem,
    new: &Problem,
    maximum: &Constraint,
    source_ids: &BTreeMap<u32, LabelId>,
) {
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
    assert_eq!(ids.len(), sets.len());
    assert_eq!(
        canonical(&converted_constraint(&old.active, &ids)),
        canonical(new.output().active())
    );
    assert_eq!(
        canonical(&converted_constraint(&old.passive, &ids)),
        canonical(new.output().passive())
    );
}

fn stats(problem: &Problem) -> serde_json::Value {
    serde_json::json!({"labels":problem.output().labels().len(),
        "active_rows":problem.output().active().configurations().count(),
        "passive_rows":problem.output().passive().configurations().count(),
        "active_degrees":problem.graph_class().active(), "passive_degrees":problem.graph_class().passive()})
}

fn old_stats(problem: &OldProblem) -> serde_json::Value {
    serde_json::json!({"labels":problem.mapping_label_text.len(),
        "active_rows":problem.active.lines.len(), "passive_rows":problem.passive.lines.len()})
}

fn emit(value: serde_json::Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}

fn main() {
    // Match the old non-Linux server's setting using its locked mimalloc build.
    #[cfg(feature = "mimalloc")]
    unsafe {
        libmimalloc_sys::mi_option_set(26, 0)
    }
    let args: Vec<_> = std::env::args().collect();
    let case = &args[1];
    let mode = &args[2];
    let steps: usize = args[3].parse().unwrap();
    let fixture = std::fs::read_to_string(&args[4]).unwrap();
    // Initialise the shared Rayon pool outside timing. Old maximization creates
    // its own per-frontier threads, which remains part of that implementation.
    rayon::ThreadPoolBuilder::new().build_global().unwrap();
    if mode == "fixed-point-new" || mode == "fixed-point-old" {
        fixed_point::run(&fixture, mode, steps, &args[5]);
        return;
    }
    if mode == "resource-new" || mode == "resource-old" {
        let saved: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let problem: Problem = serde_json::from_value(saved["new"].clone()).unwrap();
        let old: OldProblem = serde_json::from_value(saved["old"].clone()).unwrap();
        let (_, source_ids) = converted_problem(&old);
        let mut final_maximum = None;
        for iteration in 1..=steps {
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            let (maximum, milliseconds) = if mode == "resource-old" {
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
            emit(
                serde_json::json!({"case":case,"mode":mode,"iteration":iteration,
                "milliseconds":milliseconds,"maxima":maximum.configurations().count()}),
            );
            final_maximum = Some(canonical(&maximum));
        }
        // Result dumping is outside kernel timing. The resource runner compares
        // complete maxima across worker counts, rather than just row counts.
        std::fs::write(
            &args[5],
            serde_json::to_vec(&final_maximum.unwrap()).unwrap(),
        )
        .unwrap();
        return;
    }
    if mode == "profile-old" || mode == "profile-new" {
        let saved: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let old: OldProblem = serde_json::from_value(saved["old"].clone()).unwrap();
        let new: Problem = serde_json::from_value(saved["new"].clone()).unwrap();
        for iteration in 1..=steps {
            let mut input = old.passive.clone();
            input.is_maximized = false;
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            let start = Instant::now();
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
            emit(
                serde_json::json!({"case":case,"mode":mode,"iteration":iteration,
                "milliseconds":start.elapsed().as_secs_f64()*1000.0,"maxima":count}),
            );
        }
        return;
    }
    let (active, passive) = fixture.trim().split_once("\n\n").unwrap();
    let mut old = OldProblem::from_string(&fixture).unwrap();
    let mut new = parse_problem(
        serde_json::from_value(serde_json::json!({"kind":"plain",
        "constraints":{"active":active,"passive":passive}}))
        .unwrap(),
    )
    .unwrap();
    let mut events = EventHandler::null();
    if mode == "snapshot" {
        for _ in 1..steps {
            old = old.speedup(&mut events);
        }
        let (converted, _) = converted_problem(&old);
        std::fs::write(
            &args[5],
            serde_json::to_vec(&serde_json::json!({"old":old,"new":converted})).unwrap(),
        )
        .unwrap();
        return;
    }
    let mut elapsed = 0.0;
    if mode == "old-button" || mode == "maximize-cleaned" {
        // The old Start action prepares the original card before any click.
        round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
    }
    for step in 1..=steps {
        emit(serde_json::json!({"event":"starting", "case":case,"mode":mode,"step":step}));
        if mode.starts_with("maximize") {
            // Both kernels receive the SAME passive family, same label IDs,
            // same repetitions. Conversion/input cloning are outside timing.
            let (input, source_ids) = converted_problem(&old);
            let mut old_maximum = old.passive.clone();
            old_maximum.is_maximized = false;
            let mut report = |_| {};
            let mut computation = Computation::new(Cancellation::default(), &mut report);
            let sample: usize = std::env::var("RE_BENCH_SAMPLE")
                .unwrap_or_else(|_| "1".into())
                .parse()
                .unwrap();
            let mut measure_old = || {
                let start = Instant::now();
                old_maximum.maximize(&mut events);
                start.elapsed().as_secs_f64() * 1000.0
            };
            let mut measure_new = || {
                let start = Instant::now();
                let maximum =
                    maximize_constraint_with(input.output().passive(), &mut computation).unwrap();
                (maximum, start.elapsed().as_secs_f64() * 1000.0)
            };
            let (old_ms, (new_maximum, new_ms)) = if sample % 2 == 1 {
                (measure_old(), measure_new())
            } else {
                let new = measure_new();
                (measure_old(), new)
            };
            assert_eq!(
                canonical(&converted_constraint(&old_maximum, &source_ids)),
                canonical(&new_maximum)
            );
            // Also validate both COMPLETE steps after set-label alignment, outside
            // timing. The old reference determines the next shared input.
            let mut prepared = old.clone();
            prepared.passive = old_maximum;
            let old_result = prepared.speedup(&mut events);
            let new_result = round_elimination::apply_with(
                &input,
                &MaximizationResult::OutputOnly(new_maximum),
                &mut computation,
            )
            .unwrap();
            let old_maximum = converted_constraint(&prepared.passive, &source_ids);
            check_step(&old_result, &new_result, &old_maximum, &source_ids);
            emit(
                serde_json::json!({"event":"measured", "case":case,"mode":mode,"step":step,
                "old_maximize_ms":old_ms,"new_maximize_ms":new_ms,"input":stats(&input),
                "output":stats(&new_result),"equivalent":true}),
            );
            old = old_result;
            if mode == "maximize-cleaned" {
                round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
            }
            continue;
        }
        let input = if mode == "new-re" {
            stats(&new)
        } else {
            old_stats(&old)
        };
        let start = Instant::now();
        if mode == "new-re" {
            new = round_elimination::apply(std::hint::black_box(&new)).unwrap();
        } else {
            if mode == "old-button" && old.diagram_indirect.is_none() {
                old.compute_partial_diagram(&mut events);
            }
            old = old.speedup(&mut events);
            if mode == "old-button" {
                round_eliminator_lib::serial::fix_problem(&mut old, true, true, &mut events);
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        elapsed += ms;
        let output = if mode == "new-re" {
            stats(&new)
        } else {
            old_stats(&old)
        };
        emit(
            serde_json::json!({"event":"measured", "case":case,"mode":mode,"step":step,
            "milliseconds":ms,"cumulative_ms":elapsed,"input":input,"output":output}),
        );
    }
}
