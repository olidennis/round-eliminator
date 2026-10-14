//! LLM: Diagnostic driver: individual palette decisions, phase times and cancellation.
//! LLM: Built in a temporary package by run.py, outside production dependencies.

use round_eliminator_3::{
    algorithms::{
        maximize, round_elimination,
        solvability::{
            coloring::{self, ColoringKind, PaletteSearch},
            passive::PassiveMode,
        },
    },
    computation::{Cancellation, Computation, Phase, Progress},
    model::{ConstraintPair, Problem},
    parse_problem,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Same pinned v2 setting as the native server, before any worker starts.

    unsafe { libmimalloc_sys::mi_option_set(26, 0) };
    let args: Vec<_> = std::env::args().collect();
    let degree: u32 = args[1].parse().unwrap();
    let steps: usize = args[2].parse().unwrap();
    // LLM: Prepare `search` for the following computation; it stores the search for this
    // LLM: stage.
    let search = match args[3].as_str() {
        "maximum" => PaletteSearch::Maximum,
        value => PaletteSearch::Fixed {
            colors: value.parse().unwrap(),
        },
    };
    let seconds: u64 = args[4].parse().unwrap();
    // LLM: Prepare `definition` for the following computation; it stores the definition for
    // LLM: this stage.
    let definition = if let Ok(json) = std::env::var("RE3_DEFINITION_JSON") {
        serde_json::from_str(&json).unwrap()
    } else if let Ok(path) = std::env::var("RE3_DEFINITION") {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    } else {
        // LLM: Update the current working state before continuing this operation.
        serde_json::json!({
            "kind": "plain", "constraints": {
                "active": format!("M U^{}\nP^{degree}", degree - 1),
                "passive": format!("M UP^{}\nU^{degree}", degree - 1),
            }
        })
    };
    // LLM: Prepare `problem` for the following computation; it stores the problem for this
    // LLM: stage.
    let mut problem = parse_problem(serde_json::from_value(definition).unwrap()).unwrap();
    let kind = match std::env::var("RE3_COLORING_KIND")
        .as_deref()
        .unwrap_or("incidence_edges")
    {
        "vertex" => ColoringKind::Vertex,
        "graph_edges" => ColoringKind::GraphEdges,
        "distance_two" => ColoringKind::DistanceTwo,
        "incidence_edges" => ColoringKind::IncidenceEdges,
        value => panic!("unknown coloring kind {value}"),
    };
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for _ in 0..steps {
        problem = round_elimination::apply(&problem).unwrap();
    }
    let mode_name = args.get(5).map(String::as_str).unwrap_or("general");
    // LLM: Prepare `mode` for the following computation; it stores the mode for this stage.
    let mode = if matches!(mode_name, "maximized" | "premaximized") {
        // LLM: Prepare `started` for the following computation; it stores the started for this
        // LLM: stage.
        let started = Instant::now();
        let maximum = maximize::compute(&problem).unwrap();
        let maximize::MaximizationResult::OutputOnly(passive) = maximum else {
            unreachable!()
        };
        // LLM: Update the current working state before continuing this operation.
        problem = Problem::new(
            problem.graph_class().clone(),
            problem.input().clone(),
            ConstraintPair::new(
                problem.output().labels().to_vec(),
                problem.output().active().clone(),
                passive,
            )
            .unwrap(),
            problem.mapping().clone(),
        )
        .unwrap();
        // LLM: Update the current working state before continuing this operation.
        println!(
            "{}",
            serde_json::json!({"event":"premaximization", "seconds":started.elapsed().as_secs_f64()})
        );
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if mode_name == "maximized" {
            PassiveMode::Maximized
        } else {
            PassiveMode::General
        }
    } else {
        PassiveMode::General
    };
    // LLM: Update the current working state before continuing this operation.
    println!(
        "{}",
        serde_json::json!({"event":"problem", "degree":degree, "steps":steps,
        "labels":problem.output().labels().len(),
        "active_rows":problem.output().active().configurations().count(),
        "passive_rows":problem.output().passive().configurations().count(), "problem":problem})
    );
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if let Some(path) = std::env::var_os("RE3_SAVE_PROBLEM") {
        std::fs::write(path, serde_json::to_vec_pretty(&problem).unwrap()).unwrap();
    }
    let cancellation = Cancellation::default();
    let cancel_worker = cancellation.clone();
    let (done, finished) = std::sync::mpsc::channel::<()>();
    // LLM: Prepare `watchdog` for the following computation; it stores the watchdog for this
    // LLM: stage.
    let watchdog = std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(seconds)).is_err() {
            cancel_worker.cancel();
        }
    });
    let started = Instant::now();
    let mut last = started;
    let mut printed = started;
    // LLM: Prepare `phase` for the following computation; it stores the phase for this stage.
    let mut phase = Phase::Preparing;
    let mut timings = BTreeMap::<String, f64>::new();
    let mut iterations = 0;
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |update: Progress| {
        // LLM: Prepare `now` for the following computation; it stores the now for this stage.
        let now = Instant::now();
        *timings.entry(format!("{phase:?}")).or_default() += (now - last).as_secs_f64();
        phase = update.phase;
        iterations = update.iterations;
        last = now;
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if now.duration_since(printed) >= Duration::from_secs(1) {
            // LLM: Update the current working state before continuing this operation.
            println!(
                "{}",
                serde_json::json!({"event":"progress","seconds":started.elapsed().as_secs_f64(),
                "phase":phase,"iterations":iterations,"phase_seconds":timings})
            );
            printed = now;
        }
    };
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = coloring::check_with(
        &problem,
        kind,
        search,
        mode,
        &mut Computation::new(cancellation, &mut report),
        &mut |bounds| {
            println!(
                "{}",
                serde_json::json!({"event":"bounds","seconds":started.elapsed().as_secs_f64(),"bounds":bounds})
            );
        },
    );
    // LLM: Update the current working state before continuing this operation.
    *timings.entry(format!("{phase:?}")).or_default() += last.elapsed().as_secs_f64();
    let seconds = started.elapsed().as_secs_f64();
    let _ = done.send(());
    watchdog.join().unwrap();
    // LLM: Update the current working state before continuing this operation.
    println!(
        "{}",
        serde_json::json!({"event":"result", "seconds":seconds,"iterations":iterations,
        "phase_seconds":timings,"result":result.as_ref().ok(),"error":result.as_ref().err().map(ToString::to_string)})
    );
    // LLM: Verification is outside the timer and always uses exact General semantics.

    if let Ok(
        coloring::ColoringResult::Solvable { ref witness, .. }
        | coloring::ColoringResult::Maximum { ref witness, .. },
    ) = result
    {
        assert!(coloring::verify_witness(&problem, kind, witness, false));
    }
}
