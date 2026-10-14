//! Uninstrumented serial kernels and full coloring queries, with interleaved runs.
pub use after::computation;
use after::model::LabelSet;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
// Compile the actual production matcher, without exposing a benchmark-only API.
#[path = "../../src/algorithms/configurations/matching.rs"]
mod matching;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn sample<T>(run: &mut impl FnMut() -> T, batch: usize) -> f64 {
    let started = Instant::now();
    let mut iterations = 0usize;
    loop {
        // Batch clock reads so timing overhead does not dominate the tiny kernel.
        for _ in 0..batch {
            black_box(run());
        }
        iterations += batch;
        if started.elapsed() >= Duration::from_millis(50) {
            break;
        }
    }
    started.elapsed().as_secs_f64() * 1e9 / iterations as f64
}
fn compare<A, B>(
    category: &str,
    name: &str,
    mut old: impl FnMut() -> A,
    mut new: impl FnMut() -> B,
) {
    black_box(old());
    black_box(new());
    let batch = if category == "coloring" { 1 } else { 128 };
    let mut before = Vec::new();
    let mut after = Vec::new();
    for round in 0..5 {
        if round % 2 == 0 {
            before.push(sample(&mut old, batch));
            after.push(sample(&mut new, batch));
        } else {
            after.push(sample(&mut new, batch));
            before.push(sample(&mut old, batch));
        }
    }
    before.sort_by(f64::total_cmp);
    after.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"category":category, "case":name,
        "before_ns":before[2], "after_ns":after[2], "speedup":before[2]/after[2],
        "before_samples_ns":before, "after_samples_ns":after})
    );
}
fn overlap() {
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
    for (name, a, b) in cases {
        let a = LabelSet::new(a);
        let b = LabelSet::new(b);
        assert_eq!(!a.intersection(&b).is_empty(), !a.is_disjoint(&b));
        compare(
            "overlap",
            name,
            || !black_box(&a).intersection(black_box(&b)).is_empty(),
            || !black_box(&a).is_disjoint(black_box(&b)),
        );
    }
}
fn matching() {
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
        let cancellation = computation::Cancellation::default();
        let mut old = || {
            matching::matches_counts_with(
                black_box(&offered),
                black_box(&allowed),
                |a, b| !a.intersection(b).is_empty(),
                &cancellation,
            )
            .unwrap()
        };
        let mut new = || {
            matching::matches_counts_with(
                black_box(&offered),
                black_box(&allowed),
                |a, b| !a.is_disjoint(b),
                &cancellation,
            )
            .unwrap()
        };
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
            pub struct Prepared {
                problem: Problem,
                kind: ColoringKind,
                search: PaletteSearch,
            }
            pub fn prepare(case: &serde_json::Value) -> Prepared {
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
            pub fn run(p: &Prepared) -> ColoringResult {
                check(&p.problem, p.kind, p.search, PassiveMode::General).unwrap()
            }
            pub fn validate(p: &Prepared, result: &ColoringResult) {
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
fn coloring() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
    for (name, case) in cases.as_object().unwrap() {
        let old = old_queries::prepare(case);
        let new = new_queries::prepare(case);
        let a = old_queries::run(&old);
        let b = new_queries::run(&new);
        old_queries::validate(&old, &a);
        new_queries::validate(&new, &b);
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap(),
            "{name}"
        );
        compare(
            "coloring",
            name,
            || old_queries::run(black_box(&old)),
            || new_queries::run(black_box(&new)),
        );
    }
}
fn main() {
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
