//! LLM: Run with `cargo bench -p round-eliminator-3 --bench configuration_matching`.

#[path = "support/matching.rs"]
mod matching;

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Update the current working state before continuing this operation.
    println!("case,algorithm,median_ns_per_check");
    for case in matching::cases() {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (algorithm, name) in matching::ALGORITHMS.iter().enumerate() {
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(
                matching::run(&case, algorithm),
                case.expected,
                "{} / {name}",
                case.name
            );
            let mut samples = Vec::new();
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for _ in 0..3 {
                // LLM: Prepare `started` for the following computation; it stores the started
                // LLM: for this stage.
                let started = Instant::now();
                let mut iterations = 0;
                while started.elapsed() < Duration::from_millis(100) {
                    black_box(matching::run(black_box(&case), algorithm));
                    iterations += 1;
                }
                samples.push(started.elapsed().as_nanos() as f64 / iterations as f64);
            }
            // LLM: Restore deterministic canonical ordering before the value is stored or
            // LLM: returned.
            samples.sort_by(f64::total_cmp);
            println!("\"{}\",{name},{:.1}", case.name, samples[1]);
        }
    }
}
