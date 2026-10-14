//! Seven requested investigations. Generated copies expose only benchmark helpers.
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
fn sample<T>(mut run: impl FnMut() -> T) -> f64 {
    let started = Instant::now();
    let mut n = 0;
    loop {
        black_box(run());
        n += 1;
        if started.elapsed() >= Duration::from_millis(60) {
            break;
        }
    }
    started.elapsed().as_secs_f64() / n as f64
}
fn compare<A, B>(group: &str, name: &str, mut old: impl FnMut() -> A, mut new: impl FnMut() -> B) {
    let mut a = Vec::new();
    let mut b = Vec::new();
    for i in 0..3 {
        if i % 2 == 0 {
            a.push(sample(&mut old));
            b.push(sample(&mut new));
        } else {
            b.push(sample(&mut new));
            a.push(sample(&mut old));
        }
    }
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
 pub fn parse(value:&serde_json::Value)->Problem { $lib::parse_problem(serde_json::from_value(value.clone()).unwrap()).unwrap() }
 pub fn plain(active:&str,passive:&str)->Problem { parse(&serde_json::json!({"kind":"plain","constraints":{"active":active,"passive":passive}})) }
 pub fn visit(case:&str)->usize {
 let parts=match case {
 "single"=>vec![(vec![0,1,2,3,4,5],30)],
 "disjoint"=>vec![(vec![0,1,2,3],12),(vec![4,5,6,7],12)],
 "overlapping"=>vec![(vec![0,1,2],8),(vec![1,2,3],8)],
 _=>unreachable!()};
 let family=CondensedConfiguration::new(parts.into_iter().map(|(ids,n)| Part::new(LabelSet::new(ids.into_iter().map(LabelId).collect()),n).unwrap()).collect()).unwrap();
 configurations::benchmark_visit(&family)
 }
 pub fn diagram(case:&str)->Problem {
 let labels:String=(0..if case=="all256"{256}else{64}).map(|i|format!("(L{i})")).collect();
 let passive=if case=="sparse64" {(0..64).map(|i|format!("(L{i}) (L{})",(i+1)%64)).collect::<Vec<_>>().join("\n")}else if case=="required64" { format!("{} {labels}^3",(0..32).map(|i|format!("(L{i})")).collect::<String>()) }
 else {format!("{labels}^2")};plain(&labels,&passive)
 }
 pub fn compute_diagram(problem:&Problem)->DiagramResult {
 compute(problem,DiagramOptions{kind:DiagramKind::OutputOnly,output:OutputDiagramMethod::Exact,input_output:InputOutputDiagramMethod::Cheap}).unwrap()
 }
 pub fn zero(problem:&Problem,algorithm:&str)->zero_round::ZeroRoundResult {
 let result=zero_round::benchmark_check(problem,algorithm).unwrap();
 result
 }
 pub fn graph_eligible(problem:&Problem)->bool {matches!(problem.input(),Input::Trivial)
    && matches!(problem.mapping(),OutputMapping::Unrestricted) && problem.graph_class().passive().iter().all(|&degree|degree<=2)}
 pub fn zero_valid(problem:&Problem,result:&zero_round::ZeroRoundResult)->bool {
 match result {zero_round::ZeroRoundResult::Unsolvable=>true,zero_round::ZeroRoundResult::Solvable{strategy}=>zero_round::verify_strategy(problem,strategy)}
 }
 pub fn prepare_coloring(case:&serde_json::Value)->Problem {
 let mut p=parse(&case["definition"]);
 for _ in 0..case["steps"].as_u64().unwrap_or(0){p=round_elimination::apply(&p).unwrap();}p
 }
 pub fn color(problem:&Problem,case:&serde_json::Value)->coloring::ColoringResult {
 let kind=serde_json::from_value(case["kind"].clone()).unwrap();
 let search=match case["palette"].as_u64(){Some(n)=>coloring::PaletteSearch::Fixed{colors:n as u32},None=>coloring::PaletteSearch::Maximum};
 coloring::check(problem,kind,search,PassiveMode::General).unwrap()
 }
 pub fn color_valid(problem:&Problem,case:&serde_json::Value,result:&coloring::ColoringResult)->bool {
 let kind=serde_json::from_value(case["kind"].clone()).unwrap();
 match result {coloring::ColoringResult::Solvable{witness,..}|coloring::ColoringResult::Maximum{witness,..}=>coloring::verify_witness(problem,kind,witness,false),
 coloring::ColoringResult::Unbounded{witness,..}=>coloring::verify_witness(problem,kind,witness,true),_=>true}
 }
 pub fn prepare_resume(case:&serde_json::Value)->$lib::api::ProblemState {
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
 pub fn resume(state:&$lib::api::ProblemState,case:&serde_json::Value,cold:bool)->coloring::ColoringResult {
    let state=if cold {$lib::api::ProblemState::from(state.problem.clone())}else{state.clone()};
    let packet=$lib::api::execute($lib::api::Request::Compute {state,operation:$lib::api::Operation::CheckColoring {
        kind:serde_json::from_value(case["kind"].clone()).unwrap(),search:coloring::PaletteSearch::Maximum,passive_mode:PassiveMode::General}});
    let $lib::api::Response::Coloring(result)=packet.response else {panic!("unexpected resume response")};result
 }
 pub fn lattice(bits:usize)->(usize,u128,u128,u64){fixed_point::benchmark_lattice(bits,10000)}
 pub fn basic()->Problem{plain("A^5 X^2\nB^5 Y^2\nC^5 Z^2","AX BYCZ\nBY CZ\nXYZ^2")}
 pub fn larger_basic(bits:usize)->Problem {plain("(L0)",&(0..bits).map(|i|format!("(L{i})^2")).collect::<Vec<_>>().join("\n"))}
 pub fn construct(p:&Problem)->Problem {fixed_point::benchmark_construct(p).unwrap()}
 pub fn fixed(p:&Problem)->Problem {fixed_point::basic_with(p,&mut $lib::computation::Computation::new($lib::computation::Cancellation::default(),&mut |_|{})).unwrap()}
 }};
}
runner!(old, before);
runner!(new, after);
fn status(value: serde_json::Value) -> serde_json::Value {
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.remove("witness");
        object.remove("strategy");
    }
    value
}
fn main() {
    unsafe { libmimalloc_sys::mi_option_set(26, 0) };
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str).unwrap_or("enumerate") {
        "enumerate" => {
            for case in ["single", "disjoint", "overlapping"] {
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
            let case = &args[3];
            let started = Instant::now();
            let count = if args[2] == "before" {
                old::visit(case)
            } else {
                new::visit(case)
            };
            println!(
                "{}",
                serde_json::json!({"variant":args[2],"case":case,"count":count,"seconds":started.elapsed().as_secs_f64()})
            );
        }
        "diagram" => {
            for case in ["all64", "all256", "required64", "sparse64"] {
                if args.get(2).is_some_and(|requested| requested != case) {
                    continue;
                }
                let a = old::diagram(case);
                let b = new::diagram(case);
                assert_eq!(
                    serde_json::to_value(old::compute_diagram(&a)).unwrap(),
                    serde_json::to_value(new::compute_diagram(&b)).unwrap()
                );
                compare(
                    "2-diagram",
                    case,
                    || old::compute_diagram(&a),
                    || new::compute_diagram(&b),
                );
            }
        }
        "zero" => {
            let fixtures: Vec<serde_json::Value> =
                serde_json::from_str(include_str!("../support/zero_round_cases.json")).unwrap();
            for case in fixtures {
                let a = old::parse(&case["definition"]);
                let b = new::parse(&case["definition"]);
                for algorithm in ["membership", "degree_two"] {
                    if args.get(2).is_some_and(|selected| selected != algorithm) {
                        continue;
                    }
                    if args.get(2).is_some()
                        && algorithm == "degree_two"
                        && !new::graph_eligible(&b)
                    {
                        continue;
                    }
                    let x = old::zero(&a, algorithm);
                    let y = new::zero(&b, algorithm);
                    assert!(old::zero_valid(&a, &x));
                    assert!(new::zero_valid(&b, &y));
                    assert_eq!(
                        status(serde_json::to_value(x).unwrap()),
                        status(serde_json::to_value(y).unwrap())
                    );
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
            let fixtures: serde_json::Value =
                serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
            for (name, case) in fixtures
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, case)| case["kind"] == "distance_two")
            {
                let a = old::prepare_coloring(case);
                let b = new::prepare_coloring(case);
                let x = old::color(&a, case);
                let y = new::color(&b, case);
                assert!(old::color_valid(&a, case, &x));
                assert!(new::color_valid(&b, case, &y));
                assert_eq!(
                    status(serde_json::to_value(x).unwrap()),
                    status(serde_json::to_value(y).unwrap())
                );
                compare(
                    "5-distance-symmetry",
                    name,
                    || old::color(&a, case),
                    || new::color(&b, case),
                );
            }
        }
        "lattice" => {
            for bits in [6, 8, 10, 12] {
                let mut rows = Vec::new();
                for i in 0..3 {
                    let (a, b) = if i % 2 == 0 {
                        (old::lattice(bits), new::lattice(bits))
                    } else {
                        let b = new::lattice(bits);
                        (old::lattice(bits), b)
                    };
                    assert_eq!(a.0, b.0);
                    assert_eq!(a.3, b.3);
                    rows.push(serde_json::json!({"before_prepare_ns":a.1,"after_prepare_ns":b.1,"before_operations_ns":a.2,"after_operations_ns":b.2}));
                }
                println!(
                    "{}",
                    serde_json::json!({"group":"6-lattice","bits":bits,"values":1usize<<bits,"samples":rows})
                );
            }
        }
        "basic_large" => {
            let bits = args[2].parse().unwrap();
            let a = old::larger_basic(bits);
            let b = new::larger_basic(bits);
            assert_eq!(
                serde_json::to_value(old::fixed(&a)).unwrap(),
                serde_json::to_value(new::fixed(&b)).unwrap()
            );
            compare(
                "6-basic-large",
                &format!("{bits} independent labels"),
                || old::fixed(&a),
                || new::fixed(&b),
            );
        }
        "basic_construct" => {
            let bits = args[2].parse().unwrap();
            let a = old::larger_basic(bits);
            let b = new::larger_basic(bits);
            assert_eq!(
                serde_json::to_value(old::construct(&a)).unwrap(),
                serde_json::to_value(new::construct(&b)).unwrap()
            );
            compare(
                "6-basic-construction",
                &format!("{bits} independent labels"),
                || old::construct(&a),
                || new::construct(&b),
            );
        }
        "resume" => {
            let cases: serde_json::Value =
                serde_json::from_str(include_str!("../coloring/cases.json")).unwrap();
            let mut selected = vec![
                ("incidence RE(BMM)", cases["bmm-after"].clone()),
                ("graph edges", cases["matching-edges"].clone()),
            ];
            selected.push(("vertex",serde_json::json!({"kind":"vertex","palette":"maximum","definition":{"kind":"plain","constraints":{"active":"A^3\nB^3\nC^3","passive":"A BC\nB C"}}})));
            selected.push(("distance two",serde_json::json!({"kind":"distance_two","palette":"maximum","definition":{"kind":"plain","constraints":{"active":"O I","passive":"O I"}}})));
            for (name, case) in selected {
                let state = new::prepare_resume(&case);
                let cold = new::resume(&state, &case, true);
                let warm = new::resume(&state, &case, false);
                assert!(new::color_valid(&state.problem, &case, &cold));
                assert!(new::color_valid(&state.problem, &case, &warm));
                assert_eq!(
                    status(serde_json::to_value(cold).unwrap()),
                    status(serde_json::to_value(warm).unwrap())
                );
                compare(
                    "10-resume",
                    name,
                    || new::resume(&state, &case, true),
                    || new::resume(&state, &case, false),
                );
            }
        }
        "lattice_memory" => {
            let bits = args[3].parse().unwrap();
            let result = if args[2] == "before" {
                old::lattice(bits)
            } else {
                new::lattice(bits)
            };
            println!(
                "{}",
                serde_json::json!({"values":result.0,"prepare_ns":result.1,"operations_ns":result.2,"checksum":result.3})
            );
        }
        "basic" => {
            let a = old::basic();
            let b = new::basic();
            assert_eq!(
                serde_json::to_value(old::fixed(&a)).unwrap(),
                serde_json::to_value(new::fixed(&b)).unwrap()
            );
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
