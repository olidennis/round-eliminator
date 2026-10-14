//! Baseline for closure, condensed multiplicities, and native scheduling.
//! Compare separate runs with RAYON_NUM_THREADS=1 and with your normal CPU count.
//! This measures our implementation, not a speedup over the old implementation.

use round_eliminator_3::{
    algorithms::maximize::{MaximizationResult, compute},
    parse_problem,
};
use std::{hint::black_box, time::Instant};

fn main() {
    let cases = [
        (
            "three-coloring",
            serde_json::json!({"kind":"plain","constraints":{"active":"A^3\nB^3\nC^3","passive":"A BC\nB C"}}),
        ),
        (
            "bipartite-matching",
            serde_json::json!({"kind":"plain","constraints":{"active":"M U^5\nP^6","passive":"M UP^5\nU^6"}}),
        ),
        (
            "split",
            serde_json::json!({"kind":"plain","constraints":{"active":"A B AB^6","passive":"A B AB^6"}}),
        ),
        (
            "mapped-list-coloring",
            serde_json::json!({"kind":"mapped","input":{"active":"ij^3","passive":"ij^2"},"output":{"active":"A^3\nB^3\nC^3","passive":"A BC\nB C"},"mapping":"i -> AB\nj -> BC"}),
        ),
        (
            "mapped-counts",
            serde_json::json!({"kind":"mapped","input":{"active":"ij^100000","passive":"ij^100000"},"output":{"active":"A^100000","passive":"A^100000"},"mapping":"i -> A\nj -> A"}),
        ),
    ];
    println!("case,mean_us,maximal_configurations");
    for (name, text) in cases {
        let problem = parse_problem(serde_json::from_value(text).unwrap()).unwrap();
        let result = compute(&problem).unwrap(); // Warm shared pool and allocations.
        let count = match &result {
            MaximizationResult::OutputOnly(c) => c.configurations().count(),
            MaximizationResult::InputOutput(c) => c.configurations().count(),
        };
        let start = Instant::now();
        for _ in 0..20 {
            black_box(compute(black_box(&problem)).unwrap());
        }
        println!(
            "{name},{:.2},{count}",
            start.elapsed().as_secs_f64() * 1_000_000.0 / 20.0
        );
    }
}
