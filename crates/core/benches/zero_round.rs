//! Complete structured requests; notation is parsed once outside the timing.

use round_eliminator_3::{
    Request, Response, algorithms::solvability::zero_round, execute_json, parse_problem,
    parser::ProblemText,
};
use serde::Deserialize;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Case {
    name: String,
    definition: ProblemText,
    expected: String,
    #[serde(default)]
    passive_maximized: bool,
}

fn main() {
    let fixtures = std::env::var("RE3_BENCH_CASES")
        .map(|path| std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|_| include_str!("support/zero_round_cases.json").into());
    let cases: Vec<Case> = serde_json::from_str(&fixtures).unwrap();
    let mode = match std::env::var("RE3_PASSIVE_MODE")
        .as_deref()
        .unwrap_or("general")
    {
        "general" => zero_round::PassiveMode::General,
        "maximized" => zero_round::PassiveMode::Maximized,
        other => panic!("Unknown passive mode: {other}"),
    };
    println!("case,median_ms_per_request");
    for case in cases {
        let problem = parse_problem(case.definition).unwrap();
        assert!(
            mode != zero_round::PassiveMode::Maximized || case.passive_maximized,
            "Maximized mode requires a checked fixture: {}",
            case.name
        );
        if case.passive_maximized {
            maximized::assert_maximized(&problem);
        }
        let request = serde_json::to_string(&Request::CheckZeroRound {
            problem: problem.clone(),
            passive_mode: mode,
        })
        .unwrap();
        match serde_json::from_str::<serde_json::Value>(&execute_json(&request)).unwrap()["data"]
            ["status"]
            .as_str()
        {
            Some(status) => assert_eq!(status, case.expected, "{}", case.name),
            None => panic!("{} failed", case.name),
        }
        // Independently check the returned witness, outside the timing.
        if let Response::ZeroRound(zero_round::ZeroRoundResult::Solvable { strategy }) =
            round_eliminator_3::execute(Request::CheckZeroRound {
                problem: problem.clone(),
                passive_mode: mode,
            })
        {
            assert!(zero_round::verify_strategy(&problem, &strategy));
        }
        let mut samples = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            let mut iterations = 0;
            while started.elapsed() < Duration::from_millis(100) {
                black_box(execute_json(black_box(&request)));
                iterations += 1;
            }
            samples.push(started.elapsed().as_secs_f64() * 1000.0 / iterations as f64);
        }
        samples.sort_by(f64::total_cmp);
        println!("\"{}\",{:.6}", case.name, samples[2]);
    }
}

#[path = "support/maximized.rs"]
mod maximized;
