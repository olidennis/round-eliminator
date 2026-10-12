//! Reproduce whole-diagram timings, including context preparation and reduction.
//! Both fixtures are maximized by construction: complete bipartite compatibility,
//! and multisets with at least one occurrence from the first half of the labels.

use round_eliminator_3::{
    algorithms::diagram::{DiagramOptions, OutputDiagramMethod, compute},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, GraphClass, Input, LabelId, LabelSet,
        OutputMapping, Part, Problem,
    },
};
use std::{hint::black_box, time::Instant};

fn fixture(labels: u32, degree: u32, bipartite: bool) -> Problem {
    let first = LabelSet::new((0..labels / 2).map(LabelId).collect());
    let second = LabelSet::new((labels / 2..labels).map(LabelId).collect());
    let parts = if bipartite {
        vec![Part::new(first, 1).unwrap(), Part::new(second, 1).unwrap()]
    } else {
        vec![
            Part::new(first, 1).unwrap(),
            Part::new(
                second.union(&LabelSet::new((0..labels / 2).map(LabelId).collect())),
                degree - 1,
            )
            .unwrap(),
        ]
    };
    let output = ConstraintPair::new(
        (0..labels).map(|i| format!("L{i}")).collect(),
        Constraint::new(vec![]),
        Constraint::new(vec![CondensedConfiguration::new(parts).unwrap()]),
    )
    .unwrap();
    Problem::new(
        GraphClass::new(vec![degree], vec![degree]),
        Input::Trivial,
        output,
        OutputMapping::Unrestricted,
    )
    .unwrap()
}

fn main() {
    let repetitions = 100;
    println!("pool_threads,fixture,method,repetitions,milliseconds");
    for (name, problem) in [
        ("bipartite_64_degree_2", fixture(64, 2, true)),
        ("bipartite_256_degree_2", fixture(256, 2, true)),
        ("at_least_one_64_degree_13", fixture(64, 13, false)),
    ] {
        let reference = compute(&problem, DiagramOptions::default())
            .unwrap()
            .diagram;
        for method in [OutputDiagramMethod::Maximized, OutputDiagramMethod::Exact] {
            let options = DiagramOptions {
                output: method,
                ..DiagramOptions::default()
            };
            assert_eq!(compute(&problem, options).unwrap().diagram, reference);
            let start = Instant::now();
            for _ in 0..repetitions {
                black_box(compute(&problem, options).unwrap());
            }
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
