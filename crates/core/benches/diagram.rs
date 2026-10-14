//! LLM: Reproduce whole-diagram timings, including context preparation and reduction.
//! LLM: Both fixtures are maximized by construction: complete bipartite compatibility,
//! LLM: and multisets with at least one occurrence from the first half of the labels.

use round_eliminator_3::{
    algorithms::diagram::{DiagramOptions, OutputDiagramMethod, compute},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, GraphClass, Input, LabelId, LabelSet,
        OutputMapping, Part, Problem,
    },
};
use std::{hint::black_box, time::Instant};

/// LLM: Build the selected diagram benchmark problem and its label alphabet. Used outside
/// LLM: timing to distinguish sparse, dense, and repeated-context workloads.
fn fixture(labels: u32, degree: u32, bipartite: bool) -> Problem {
    // LLM: Prepare `first` for the following computation; it stores the first for this stage.
    let first = LabelSet::new((0..labels / 2).map(LabelId).collect());
    let second = LabelSet::new((labels / 2..labels).map(LabelId).collect());
    // LLM: Prepare `parts` for the following computation; it stores the parts for this stage.
    let parts = if bipartite {
        vec![Part::new(first, 1).unwrap(), Part::new(second, 1).unwrap()]
    } else {
        // LLM: Update the current working state before continuing this operation.
        vec![
            Part::new(first, 1).unwrap(),
            Part::new(
                second.union(&LabelSet::new((0..labels / 2).map(LabelId).collect())),
                degree - 1,
            )
            .unwrap(),
        ]
    };
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(
        (0..labels).map(|i| format!("L{i}")).collect(),
        Constraint::new(vec![]),
        Constraint::new(vec![CondensedConfiguration::new(parts).unwrap()]),
    )
    .unwrap();
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    Problem::new(
        GraphClass::new(vec![degree], vec![degree]),
        Input::Trivial,
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap()
}

/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Prepare `repetitions` for the following computation; it stores the repetitions for
    // LLM: this stage.
    let repetitions = 100;
    println!("pool_threads,fixture,method,repetitions,milliseconds");
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for (name, problem) in [
        ("bipartite_64_degree_2", fixture(64, 2, true)),
        ("bipartite_256_degree_2", fixture(256, 2, true)),
        ("at_least_one_64_degree_13", fixture(64, 13, false)),
    ] {
        // LLM: Prepare `reference` for the following computation; it stores the reference for
        // LLM: this stage.
        let reference = compute(&problem, DiagramOptions::default())
            .unwrap()
            .diagram;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for method in [OutputDiagramMethod::Maximized, OutputDiagramMethod::Exact] {
            // LLM: Prepare `options` for the following computation; it stores the options for
            // LLM: this stage.
            let options = DiagramOptions {
                output: method,
                ..DiagramOptions::default()
            };
            assert_eq!(compute(&problem, options).unwrap().diagram, reference);
            let start = Instant::now();
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for _ in 0..repetitions {
                black_box(compute(&problem, options).unwrap());
            }
            // LLM: Update the current working state before continuing this operation.
            println!(
                "{},{name},{method:?},{repetitions},{:.3}",
                {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        rayon::current_num_threads()
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        1
                    }
                },
                start.elapsed().as_secs_f64() * 1000.0 / repetitions as f64
            );
        }
    }
}
