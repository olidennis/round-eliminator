//! Run with `cargo bench -p round-eliminator-3 --bench configuration_matching`.

#[path = "support/matching.rs"]
mod matching;

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn main() {
    println!("case,algorithm,median_ns_per_check");
    for case in matching::cases() {
        for (algorithm, name) in matching::ALGORITHMS.iter().enumerate() {
            assert_eq!(
                matching::run(&case, algorithm),
                case.expected,
                "{} / {name}",
                case.name
            );
            let mut samples = Vec::new();
            for _ in 0..3 {
                let started = Instant::now();
                let mut iterations = 0;
                while started.elapsed() < Duration::from_millis(100) {
                    black_box(matching::run(black_box(&case), algorithm));
                    iterations += 1;
                }
                samples.push(started.elapsed().as_nanos() as f64 / iterations as f64);
            }
            samples.sort_by(f64::total_cmp);
            println!("\"{}\",{name},{:.1}", case.name, samples[1]);
        }
    }
}
