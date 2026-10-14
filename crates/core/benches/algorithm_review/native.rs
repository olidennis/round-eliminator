//! LLM: Seven requested investigations. Generated copies expose only benchmark helpers.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
/// LLM: Time repeated invocations and return a representative elapsed sample. Used by isolated
/// LLM: comparison benchmarks, with result checking kept separate from timing where specified.
fn sample<T>(mut run: impl FnMut() -> T) -> f64 {
    // LLM: Prepare `started` for the following computation; it stores the started for this
    // LLM: stage.
    let started = Instant::now();
    let mut n = 0;
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    loop {
        black_box(run());
        n += 1;
        if started.elapsed() >= Duration::from_millis(60) {
            break;
        }
    }
    started.elapsed().as_secs_f64() / n as f64
}
/// LLM: Measure baseline and candidate implementations on the same fixture and record
/// LLM: comparable timing/results. Used by the retained native benchmark runner.
fn compare<A, B>(group: &str, name: &str, mut old: impl FnMut() -> A, mut new: impl FnMut() -> B) {
    // LLM: Prepare `a` for the following computation; it stores the a for this stage.
    let mut a = Vec::new();
    let mut b = Vec::new();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for i in 0..3 {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if i % 2 == 0 {
            a.push(sample(&mut old));
            b.push(sample(&mut new));
        } else {
            b.push(sample(&mut new));
            a.push(sample(&mut old));
        }
    }
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"group":group,"case":name,"before_s":a[1],"after_s":b[1],"speedup":a[1]/b[1],"before_samples_s":a,"after_samples_s":b})
    );
}
macro_rules! runner {
 ($module:ident,$lib:ident)=>{ mod $module {
 use $lib::{model::*, algorithms::{configurations, diagram::*, fixed_point, round_elimination, solvability::{zero_round,coloring,passive::PassiveMode}}};
 /// LLM: Parse the same JSON fixture using this macro-selected core implementation. Used to
 /// LLM: prepare paired baseline/candidate benchmarks.
 pub fn parse(value:&serde_json::Value)->Problem {
     // LLM: Parse the same JSON fixture using this macro-selected core implementation.
     $lib::parse_problem(serde_json::from_value(value.clone()).unwrap()).unwrap() }
 /// LLM: Build a plain problem fixture with inferred degrees through this implementation's
 /// LLM: parser. Used by the benchmark case constructors.
 pub fn plain(active:&str,passive:&str)->Problem {
     // LLM: Build a plain problem fixture with inferred degrees through this implementation's
     // LLM: parser.
     parse(&serde_json::json!({"kind":"plain","constraints":{"active":active,"passive":passive}})) }
 /// LLM: Build the selected unique or overlapping condensed family and count its concrete
 /// LLM: choices. Used to compare traversal work and peak memory.
 pub fn visit(case:&str)->usize {
 // LLM: Build the selected unique or overlapping condensed family and count its concrete
 // LLM: choices.
 let parts=match case {
 "single"=>vec![(vec![0,1,2,3,4,5],30)],
 "disjoint"=>vec![(vec![0,1,2,3],12),(vec![4,5,6,7],12)],
 "overlapping"=>vec![(vec![0,1,2],8),(vec![1,2,3],8)],
 _=>unreachable!()};
 let family=CondensedConfiguration::new(parts.into_iter().map(|(ids,n)| Part::new(LabelSet::new(ids.into_iter().map(LabelId).collect()),n).unwrap()).collect()).unwrap();
 configurations::benchmark_visit(&family)
 }
 /// LLM: Construct the selected dense, sparse, or shared-profile output constraint. Used to
 /// LLM: prepare exact diagram comparisons.
 pub fn diagram(case:&str)->Problem {
 // LLM: Construct the selected dense, sparse, or shared-profile output constraint.
 let labels:String=(0..if case=="all256"{256}else{64}).map(|i|format!("(L{i})")).collect();
 let passive=if case=="sparse64" {(0..64).map(|i|format!("(L{i}) (L{})",(i+1)%64)).collect::<Vec<_>>().join("\n")}else if case=="required64" { format!("{} {labels}^3",(0..32).map(|i|format!("(L{i})")).collect::<String>()) }
 else {format!("{labels}^2")};plain(&labels,&passive)
 }
 /// LLM: Compute only the exact output-only diagram. Used as the timed relation operation in
 /// LLM: the baseline/candidate comparison.
 pub fn compute_diagram(problem:&Problem)->DiagramResult {
 // LLM: Compute only the exact output-only diagram.
 compute(problem,DiagramOptions{kind:DiagramKind::OutputOnly,output:OutputDiagramMethod::Exact,input_output:InputOutputDiagramMethod::Cheap}).unwrap()
 }
 /// LLM: Run the selected zero-round encoding or specialized prototype on the prepared
 /// LLM: problem. Used to compare equivalent decision algorithms.
 pub fn zero(problem:&Problem,algorithm:&str)->zero_round::ZeroRoundResult {
 // LLM: Run the selected zero-round encoding or specialized prototype on the prepared problem.
 let result=zero_round::benchmark_check(problem,algorithm).unwrap();
 result
 }
 /// LLM: Recognize plain unrestricted problems with passive degrees at most two. Used to
 /// LLM: select fixtures eligible for the specialized support-search comparison.
 pub fn graph_eligible(problem:&Problem)->bool {
     // LLM: Recognize plain unrestricted problems with passive degrees at most two.
     matches!(problem.input(),Input::Trivial)
    && matches!(problem.mapping(),OutputMapping::Unrestricted) && problem.graph_class().passive().iter().all(|&degree|degree<=2)}
 /// LLM: Independently verify a positive zero-round result; negative results are compared
 /// LLM: separately between implementations. Used outside benchmark timing.
 pub fn zero_valid(problem:&Problem,result:&zero_round::ZeroRoundResult)->bool {
 // LLM: Independently verify a positive zero-round result; negative results are compared
 // LLM: separately between implementations.
 match result {zero_round::ZeroRoundResult::Unsolvable=>true,zero_round::ZeroRoundResult::Solvable{strategy}=>zero_round::verify_strategy(problem,strategy)}
 }
 /// LLM: Parse a coloring fixture and perform the requested RE steps. Used outside the solver
 /// LLM: timer so transformed cases are comparable.
 pub fn prepare_coloring(case:&serde_json::Value)->Problem {
 // LLM: Parse a coloring fixture and perform the requested RE steps.
 let mut p=parse(&case["definition"]);
 for _ in 0..case["steps"].as_u64().unwrap_or(0){p=round_elimination::apply(&p).unwrap();}p
 }
 /// LLM: Run the selected fixed-palette or maximum coloring query. Used as the timed solver
 /// LLM: operation.
 pub fn color(problem:&Problem,case:&serde_json::Value)->coloring::ColoringResult {
 // LLM: Run the selected fixed-palette or maximum coloring query.
 let kind=serde_json::from_value(case["kind"].clone()).unwrap();
 let search=match case["palette"].as_u64(){Some(n)=>coloring::PaletteSearch::Fixed{colors:n as u32},None=>coloring::PaletteSearch::Maximum};
 coloring::check(problem,kind,search,PassiveMode::General).unwrap()
 }
 /// LLM: Verify numerical or unbounded strategy witnesses independently of SAT. Used outside
 /// LLM: timing for every positive coloring result.
 pub fn color_valid(problem:&Problem,case:&serde_json::Value,result:&coloring::ColoringResult)->bool {
 // LLM: Verify numerical or unbounded strategy witnesses independently of SAT.
 let kind=serde_json::from_value(case["kind"].clone()).unwrap();
 match result {coloring::ColoringResult::Solvable{witness,..}|coloring::ColoringResult::Maximum{witness,..}=>coloring::verify_witness(problem,kind,witness,false),
 coloring::ColoringResult::Unbounded{witness,..}=>coloring::verify_witness(problem,kind,witness,true),_=>true}
 }
 /// LLM: Interrupt a maximum search after its first proved lower bound and retain the
 /// LLM: resulting shared analysis state. Used to prepare a genuine cancelled-search
 /// LLM: resumption fixture.
 pub fn prepare_resume(case:&serde_json::Value)->$lib::api::ProblemState {
    // LLM: Interrupt a maximum search after its first proved lower bound and retain the
    // LLM: resulting shared analysis state.
    let p=prepare_coloring(case);let kind=serde_json::from_value(case["kind"].clone()).unwrap();
    let state=$lib::api::ProblemState::from(p);
    let request=$lib::api::Request::Compute {state:state.clone(),operation:$lib::api::Operation::CheckColoring {
        kind,search:coloring::PaletteSearch::Maximum,passive_mode:PassiveMode::General}};
    let cancellation=$lib::computation::Cancellation::default();
    let packet=$lib::api::execute_text_with_events(&serde_json::to_string(&request).unwrap(),cancellation.clone(),&mut |event| {
        if let $lib::api::ComputationEvent::PartialResult(response)=event
            && let $lib::api::Response::ColoringBounds(bounds)=response.as_ref()
            && bounds.lower.is_some() && bounds.testing.is_some() {cancellation.cancel();}
    });
    assert!(matches!(packet.response,$lib::api::Response::Cancelled));
    assert!(packet.cache.coloring_bounds.iter().any(|entry|entry.kind==kind && entry.bounds.lower.is_some()));
    $lib::api::ProblemState {analyses:packet.cache,..state}
 }
 /// LLM: Continue maximum search with the saved proofs, or clear them for the cold comparison.
 /// LLM: Used to measure the work saved by client-owned proof reuse.
 pub fn resume(state:&$lib::api::ProblemState,case:&serde_json::Value,cold:bool)->coloring::ColoringResult {
    // LLM: Continue maximum search with the saved proofs, or clear them for the cold
    // LLM: comparison.
    let state=if cold {$lib::api::ProblemState::from(state.problem.clone())}else{state.clone()};
    let packet=$lib::api::execute($lib::api::Request::Compute {state,operation:$lib::api::Operation::CheckColoring {
        kind:serde_json::from_value(case["kind"].clone()).unwrap(),search:coloring::PaletteSearch::Maximum,passive_mode:PassiveMode::General}});
    let $lib::api::Response::Coloring(result)=packet.response else {panic!("unexpected resume response")};result
 }
 /// LLM: Measure lattice preparation and a fixed operation sample, returning size, times, and
 /// LLM: checksum. Used to compare table and direct-set representations.
 pub fn lattice(bits:usize)->(usize,u128,u128,u64){
     // LLM: Measure lattice preparation and a fixed operation sample, returning size, times,
     // LLM: and checksum.
     fixed_point::benchmark_lattice(bits,10000)}
 /// LLM: Construct the supplied counted Basic FP fixture. Used for whole-construction and
 /// LLM: whole-button comparisons.
 pub fn basic()->Problem{
     // LLM: Construct the supplied counted Basic FP fixture.
     plain("A^5 X^2\nB^5 Y^2\nC^5 Z^2","AX BYCZ\nBY CZ\nXYZ^2")}
 /// LLM: Construct an independent-label lattice fixture of the requested width. Used to
 /// LLM: isolate larger-lattice preparation and construction costs.
 pub fn larger_basic(bits:usize)->Problem {
     // LLM: Construct an independent-label lattice fixture of the requested width.
     plain("(L0)",&(0..bits).map(|i|format!("(L{i})^2")).collect::<Vec<_>>().join("\n"))}
 /// LLM: Run Basic FP construction without discard-useless simplification. Used to measure the
 /// LLM: construction stage separately.
 pub fn construct(p:&Problem)->Problem {
     // LLM: Run Basic FP construction without discard-useless simplification.
     fixed_point::benchmark_construct(p).unwrap()}
 /// LLM: Run the complete Basic FP operation, including cleanup. Used for the whole-button
 /// LLM: baseline/candidate comparison.
 pub fn fixed(p:&Problem)->Problem {
     // LLM: Run the complete Basic FP operation, including cleanup.
     fixed_point::basic_with(p,&mut $lib::computation::Computation::new($lib::computation::Cancellation::default(),&mut |_|{})).unwrap()}
 }};
}
runner!(old, before);
runner!(new, after);
/// LLM: Reduce a benchmark result to its decision category for comparison. Used when baseline
/// LLM: and candidate witnesses can differ while their solvability must agree.
fn status(value: serde_json::Value) -> serde_json::Value {
    // LLM: Prepare `value` for the following computation; it stores the value for this stage.
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.remove("witness");
        object.remove("strategy");
    }
    value
}
/// LLM: Run this benchmark's selected fixtures and report timings/comparison results. This is
/// LLM: the standalone benchmark entry point, not an application API.
fn main() {
    // LLM: Update the current working state before continuing this operation.
    unsafe { libmimalloc_sys::mi_option_set(26, 0) };
    let args: Vec<_> = std::env::args().collect();
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    match args.get(1).map(String::as_str).unwrap_or("enumerate") {
        "enumerate" => {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for case in ["single", "disjoint", "overlapping"] {
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(old::visit(case), new::visit(case));
                compare(
                    "1-enumeration",
                    case,
                    || old::visit(case),
                    || new::visit(case),
                );
            }
        }
        "memory" => {
            // LLM: Prepare `case` for the following computation; it stores the case for this
            // LLM: stage.
            let case = &args[3];
            let started = Instant::now();
            let count = if args[2] == "before" {
                old::visit(case)
            } else {
                new::visit(case)
            };
            // LLM: Update the current working state before continuing this operation.
            println!(
                "{}",
                serde_json::json!({"variant":args[2],"case":case,"count":count,"seconds":started.elapsed().as_secs_f64()})
            );
        }
        "diagram" => {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for case in ["all64", "all256", "required64", "sparse64"] {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if args.get(2).is_some_and(|requested| requested != case) {
                    continue;
                }
                let a = old::diagram(case);
                let b = new::diagram(case);
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    serde_json::to_value(old::compute_diagram(&a)).unwrap(),
                    serde_json::to_value(new::compute_diagram(&b)).unwrap()
                );
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                compare(
                    "2-diagram",
                    case,
                    || old::compute_diagram(&a),
                    || new::compute_diagram(&b),
                );
            }
        }
        "zero" => {
            // LLM: Prepare `fixtures` for the following computation; it stores the fixtures
            // LLM: for this stage.
            let fixtures: Vec<serde_json::Value> =
                serde_json::from_str(include_str!("../support/zero_round_cases.json")).unwrap();
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for case in fixtures {
                // LLM: Prepare `a` for the following computation; it stores the a for this
                // LLM: stage.
                let a = old::parse(&case["definition"]);
                let b = new::parse(&case["definition"]);
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for algorithm in ["membership", "degree_two"] {
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if args.get(2).is_some_and(|selected| selected != algorithm) {
                        continue;
                    }
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if args.get(2).is_some()
                        && algorithm == "degree_two"
                        && !new::graph_eligible(&b)
                    {
                        continue;
                    }
                    let x = old::zero(&a, algorithm);
                    let y = new::zero(&b, algorithm);
                    // LLM: Check the expected result and invariants for this fixture,
                    // LLM: including the relevant edge case.
                    assert!(old::zero_valid(&a, &x));
                    assert!(new::zero_valid(&b, &y));
                    assert_eq!(
                        status(serde_json::to_value(x).unwrap()),
                        status(serde_json::to_value(y).unwrap())
                    );
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    compare(
                        algorithm,
                        case["name"].as_str().unwrap(),
                        || new::zero(&b, "ports"),
                        || new::zero(&b, algorithm),
                    );
                }
            }
        }
        "distance" => {
            // LLM: Prepare `fixtures` for the following computation; it stores the fixtures
            // LLM: for this stage.
            let fixtures: serde_json::Value =
                serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for (name, case) in fixtures
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, case)| case["kind"] == "distance_two")
            {
                // LLM: Prepare `a` for the following computation; it stores the a for this
                // LLM: stage.
                let a = old::prepare_coloring(case);
                let b = new::prepare_coloring(case);
                let x = old::color(&a, case);
                let y = new::color(&b, case);
                assert!(old::color_valid(&a, case, &x));
                assert!(new::color_valid(&b, case, &y));
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    status(serde_json::to_value(x).unwrap()),
                    status(serde_json::to_value(y).unwrap())
                );
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                compare(
                    "5-distance-symmetry",
                    name,
                    || old::color(&a, case),
                    || new::color(&b, case),
                );
            }
        }
        "lattice" => {
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for bits in [6, 8, 10, 12] {
                // LLM: Prepare counted rows or remaining row capacities for the next exact
                // LLM: comparison.
                let mut rows = Vec::new();
                for i in 0..3 {
                    // LLM: Update the current working state before continuing this operation.
                    let (a, b) = if i % 2 == 0 {
                        (old::lattice(bits), new::lattice(bits))
                    } else {
                        let b = new::lattice(bits);
                        (old::lattice(bits), b)
                    };
                    assert_eq!(a.0, b.0);
                    assert_eq!(a.3, b.3);
                    // LLM: Retain the newly constructed choices in the result for this stage.
                    rows.push(serde_json::json!({"before_prepare_ns":a.1,"after_prepare_ns":b.1,"before_operations_ns":a.2,"after_operations_ns":b.2}));
                }
                // LLM: Update the current working state before continuing this operation.
                println!(
                    "{}",
                    serde_json::json!({"group":"6-lattice","bits":bits,"values":1usize<<bits,"samples":rows})
                );
            }
        }
        "basic_large" => {
            // LLM: Prepare `bits` for the following computation; it stores the bits for this
            // LLM: stage.
            let bits = args[2].parse().unwrap();
            let a = old::larger_basic(bits);
            let b = new::larger_basic(bits);
            assert_eq!(
                serde_json::to_value(old::fixed(&a)).unwrap(),
                serde_json::to_value(new::fixed(&b)).unwrap()
            );
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            compare(
                "6-basic-large",
                &format!("{bits} independent labels"),
                || old::fixed(&a),
                || new::fixed(&b),
            );
        }
        "basic_construct" => {
            // LLM: Prepare `bits` for the following computation; it stores the bits for this
            // LLM: stage.
            let bits = args[2].parse().unwrap();
            let a = old::larger_basic(bits);
            let b = new::larger_basic(bits);
            assert_eq!(
                serde_json::to_value(old::construct(&a)).unwrap(),
                serde_json::to_value(new::construct(&b)).unwrap()
            );
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            compare(
                "6-basic-construction",
                &format!("{bits} independent labels"),
                || old::construct(&a),
                || new::construct(&b),
            );
        }
        "resume" => {
            // LLM: Prepare `cases` for the following computation; it stores the cases for this
            // LLM: stage.
            let cases: serde_json::Value =
                serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
            let mut selected = vec![
                ("incidence RE(BMM)", cases["bmm-after"].clone()),
                ("graph edges", cases["matching-edges"].clone()),
            ];
            selected.push(("vertex",serde_json::json!({"kind":"vertex","palette":"maximum","definition":{"kind":"plain","constraints":{"active":"A^3\nB^3\nC^3","passive":"A BC\nB C"}}})));
            selected.push(("distance two",serde_json::json!({"kind":"distance_two","palette":"maximum","definition":{"kind":"plain","constraints":{"active":"O I","passive":"O I"}}})));
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for (name, case) in selected {
                // LLM: Prepare `state` for the following computation; it stores the state for
                // LLM: this stage.
                let state = new::prepare_resume(&case);
                let cold = new::resume(&state, &case, true);
                let warm = new::resume(&state, &case, false);
                assert!(new::color_valid(&state.problem, &case, &cold));
                assert!(new::color_valid(&state.problem, &case, &warm));
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert_eq!(
                    status(serde_json::to_value(cold).unwrap()),
                    status(serde_json::to_value(warm).unwrap())
                );
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                compare(
                    "10-resume",
                    name,
                    || new::resume(&state, &case, true),
                    || new::resume(&state, &case, false),
                );
            }
        }
        "lattice_memory" => {
            // LLM: Prepare `bits` for the following computation; it stores the bits for this
            // LLM: stage.
            let bits = args[3].parse().unwrap();
            let result = if args[2] == "before" {
                old::lattice(bits)
            } else {
                new::lattice(bits)
            };
            // LLM: Update the current working state before continuing this operation.
            println!(
                "{}",
                serde_json::json!({"values":result.0,"prepare_ns":result.1,"operations_ns":result.2,"checksum":result.3})
            );
        }
        "basic" => {
            // LLM: Prepare `a` for the following computation; it stores the a for this stage.
            let a = old::basic();
            let b = new::basic();
            assert_eq!(
                serde_json::to_value(old::fixed(&a)).unwrap(),
                serde_json::to_value(new::fixed(&b)).unwrap()
            );
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            compare(
                "6-basic-full",
                "given-basic",
                || old::fixed(&a),
                || new::fixed(&b),
            );
        }
        _ => panic!("unknown experiment"),
    }
}
