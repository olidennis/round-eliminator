//! LLM: Uninstrumented serial kernels and full coloring queries, with interleaved runs.

pub use after::computation;
use after::model::LabelSet;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
// LLM: Compile the actual production matcher, without exposing a benchmark-only API.

#[path = "../../src/algorithms/configurations/matching.rs"]
mod matching;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// LLM: Time repeated invocations and return a representative elapsed sample. Used by isolated
/// LLM: comparison benchmarks, with result checking kept separate from timing where specified.
fn sample<T>(run: &mut impl FnMut() -> T, batch: usize) -> f64 {
    // LLM: Prepare `started` for the following computation; it stores the started for this
    // LLM: stage.
    let started = Instant::now();
    let mut iterations = 0usize;
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    loop {
        // LLM: Batch clock reads so timing overhead does not dominate the tiny kernel.

        for _ in 0..batch {
            black_box(run());
        }
        iterations += batch;
        if started.elapsed() >= Duration::from_millis(50) {
            break;
        }
    }
    // LLM: Update the current working state before continuing this operation.
    started.elapsed().as_secs_f64() * 1e9 / iterations as f64
}
/// LLM: Measure baseline and candidate implementations on the same fixture and record
/// LLM: comparable timing/results. Used by the retained native benchmark runner.
fn compare<A, B>(
    category: &str,
    name: &str,
    mut old: impl FnMut() -> A,
    mut new: impl FnMut() -> B,
) {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    black_box(old());
    black_box(new());
    let batch = if category == "coloring" { 1 } else { 128 };
    let mut before = Vec::new();
    let mut after = Vec::new();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for round in 0..5 {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if round % 2 == 0 {
            before.push(sample(&mut old, batch));
            after.push(sample(&mut new, batch));
        } else {
            after.push(sample(&mut new, batch));
            before.push(sample(&mut old, batch));
        }
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    before.sort_by(f64::total_cmp);
    after.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"category":category, "case":name,
        "before_ns":before[2], "after_ns":after[2], "speedup":before[2]/after[2],
        "before_samples_ns":before, "after_samples_ns":after})
    );
}
/// LLM: Benchmark empty/nonempty set intersections and compare baseline allocation with the
/// LLM: early-exit scan. Used by the isolated label-overlap investigation.
fn overlap() {
    // LLM: Prepare `cases` for the following computation; it stores the cases for this stage.
    let cases = [
        ("singleton equal", vec![1], vec![1]),
        ("singleton disjoint", vec![0], vec![1]),
        (
            "128 labels, first overlap",
            (0..128).collect(),
            (0..128).collect(),
        ),
        (
            "128 labels, last overlap",
            (0..128).collect(),
            (127..255).collect(),
        ),
        (
            "128 labels, interleaved disjoint",
            (0..128).map(|i| 2 * i).collect(),
            (0..128).map(|i| 2 * i + 1).collect(),
        ),
        (
            "128 labels, separated",
            (0..128).collect(),
            (128..256).collect(),
        ),
    ];
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (name, a, b) in cases {
        // LLM: Prepare `a` for the following computation; it stores the a for this stage.
        let a = LabelSet::new(a);
        let b = LabelSet::new(b);
        assert_eq!(!a.intersection(&b).is_empty(), !a.is_disjoint(&b));
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        compare(
            "overlap",
            name,
            || !black_box(&a).intersection(black_box(&b)).is_empty(),
            || !black_box(&a).is_disjoint(black_box(&b)),
        );
    }
}
/// LLM: Benchmark counted membership cases using both prepared core implementations. Used to
/// LLM: measure the overlap improvement within real matching kernels.
fn matching() {
    // LLM: Use exact counted matching so repeated occurrences do not need a port-permutation
    // LLM: expansion.
    for (name, width, offered, allowed, expected) in [
        ("8 sparse positions", 8, vec![0], vec![0], true),
        (
            "8 broad positions",
            8,
            (0..128).collect(),
            (0..128).collect(),
            true,
        ),
        (
            "17 broad positions, uncached matrix",
            17,
            (0..128).collect(),
            (0..128).collect(),
            true,
        ),
        (
            "8 broad disjoint positions",
            8,
            (0..128).map(|i| 2 * i).collect(),
            (0..128).map(|i| 2 * i + 1).collect(),
            false,
        ),
    ] {
        // LLM: Update the current working state before continuing this operation.
        let (offered, allowed) = if name == "8 sparse positions" {
            let parts: Vec<_> = (0..width)
                .map(|i| (LabelSet::new(vec![i as u32]), 1))
                .collect();
            (parts.clone(), parts)
        } else {
            (
                vec![(LabelSet::new(offered), 1); width],
                vec![(LabelSet::new(allowed), 1); width],
            )
        };
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = computation::Cancellation::default();
        let mut old = || {
            // LLM: Use exact counted matching so repeated occurrences do not need a
            // LLM: port-permutation expansion.
            matching::matches_counts_with(
                black_box(&offered),
                black_box(&allowed),
                |a, b| !a.intersection(b).is_empty(),
                &cancellation,
            )
            .unwrap()
        };
        // LLM: Use exact counted matching so repeated occurrences do not need a
        // LLM: port-permutation expansion.
        let mut new = || {
            // LLM: Use exact counted matching so repeated occurrences do not need a
            // LLM: port-permutation expansion.
            matching::matches_counts_with(
                black_box(&offered),
                black_box(&allowed),
                |a, b| !a.is_disjoint(b),
                &cancellation,
            )
            .unwrap()
        };
        // LLM: Compare the implementation result with the independently prepared reference for
        // LLM: this fixture.
        assert_eq!(old(), expected);
        assert_eq!(new(), expected);
        compare("matching", name, &mut old, &mut new);
    }
}
macro_rules! runner {
    ($module:ident, $library:ident) => {
        mod $module {
            use $library::{
                algorithms::{
                    round_elimination,
                    solvability::{coloring::*, passive::PassiveMode},
                },
                model::Problem,
            };
            /// LLM: The parsed benchmark problem, coloring kind, and query prepared outside
            /// LLM: the timer. Both implementations receive the same definition and options.
            pub struct Prepared {
                problem: Problem,
                kind: ColoringKind,
                search: PaletteSearch,
            }
            /// LLM: Parse the fixture and apply its requested RE steps before timing. Used by
            /// LLM: the coloring comparison runner to keep preparation outside measured
            /// LLM: queries.
            pub fn prepare(case: &serde_json::Value) -> Prepared {
                // LLM: Parse the fixture and apply its requested RE steps before timing.
                let mut problem = $library::parse_problem(
                    serde_json::from_value(case["definition"].clone()).unwrap(),
                )
                .unwrap();
                for _ in 0..case["steps"].as_u64().unwrap_or(0) {
                    problem = round_elimination::apply(&problem).unwrap();
                }
                let kind = serde_json::from_value(case["kind"].clone()).unwrap();
                let search = match case["palette"].as_u64() {
                    Some(colors) => PaletteSearch::Fixed {
                        colors: colors as u32,
                    },
                    None => PaletteSearch::Maximum,
                };
                Prepared {
                    problem,
                    kind,
                    search,
                }
            }
            /// LLM: Run the prepared coloring query using general passive verification. Used
            /// LLM: as the timed benchmark operation.
            pub fn run(p: &Prepared) -> ColoringResult {
                // LLM: Run the prepared coloring query using general passive verification.
                check(&p.problem, p.kind, p.search, PassiveMode::General).unwrap()
            }
            /// LLM: Independently verify any returned coloring witness after timing. Used to
            /// LLM: reject performance results that changed strategy semantics.
            pub fn validate(p: &Prepared, result: &ColoringResult) {
                // LLM: Independently verify any returned coloring witness after timing.
                match result {
                    ColoringResult::Solvable { witness, .. }
                    | ColoringResult::Maximum { witness, .. } => {
                        assert!(verify_witness(&p.problem, p.kind, witness, false))
                    }
                    ColoringResult::Unbounded { witness, .. } => {
                        assert!(verify_witness(&p.problem, p.kind, witness, true))
                    }
                    _ => {}
                }
            }
        }
    };
}
runner!(old_queries, before);
runner!(new_queries, after);
/// LLM: Prepare, time, and independently validate the selected full coloring fixtures. Used to
/// LLM: distinguish isolated matching gains from whole-query effects.
fn coloring() {
    // LLM: Prepare `cases` for the following computation; it stores the cases for this stage.
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for (name, case) in cases.as_object().unwrap() {
        // LLM: Prepare `old` for the following computation; it stores the old for this stage.
        let old = old_queries::prepare(case);
        let new = new_queries::prepare(case);
        let a = old_queries::run(&old);
        let b = new_queries::run(&new);
        old_queries::validate(&old, &a);
        new_queries::validate(&new, &b);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap(),
            "{name}"
        );
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        compare(
            "coloring",
            name,
            || old_queries::run(black_box(&old)),
            || new_queries::run(black_box(&new)),
        );
    }
}
/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Update the current working state before continuing this operation.
    unsafe { libmimalloc_sys::mi_option_set(26, 0) };
    match std::env::args().nth(1).as_deref() {
        Some("kernel") => {
            overlap();
            matching();
        }
        Some("coloring") => coloring(),
        _ => {
            overlap();
            matching();
            coloring();
        }
    }
}
