use indicatif::ProgressBar;
use indicatif::ProgressStyle;
#[cfg(not(target_os = "linux"))]
use mimalloc::MiMalloc;
#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[test]
fn pgo_quick_test() {               
    assert!(std::hint::black_box(round_eliminator_lib::test_all_short()) > 0);
}

use std::io::BufRead;
use std::io::Write;
use itertools::Itertools;
use round_eliminator_lib::serial::Request;
use round_eliminator_lib::serial::Response;
use round_eliminator_lib::problem::Problem;
use std::sync::Mutex;
use round_eliminator_lib::group::Label;
use std::collections::HashMap;

fn make_request(request : Request, problems : &mut Vec<Problem>) {
    let serialized = serde_json::to_string(&request).unwrap();
    let problems = Mutex::new(problems);

    let pb = ProgressBar::no_length();
    pb.set_style(ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    round_eliminator_lib::serial::request_json(&serialized, |s, send_to_client| {
        if send_to_client {
            let response: Response = serde_json::from_str(&s).unwrap();
            match response {
                Response::Done => {},
                Response::Pong => {},
                Response::Event(x,a,b) => {
                    pb.set_length(b as u64);
                    pb.set_position(a as u64);
                    pb.set_message(x);
                    //println!("{} {} {}",x,a,b);
                },
                Response::P(p) => {
                    let mut problems = problems.lock().unwrap();
                    problems.push(p.clone());
                    //println!("Obtained problem:\n{}",p);
                },
                Response::E(e) => { println!("ERROR: {}",e); }
                Response::W(e) => { println!("WARNING: {}",e); }
                Response::AutoUb(_,_) => { todo!("autoub"); }
                Response::AutoLb(_,_) => { todo!("autolb"); }
            }
        }
    });
}

fn help(){
    println!("Supported commands:");
    println!("status : show the number of problems in the queue");
    println!("pop : delete last problem");
    println!("clear : delete all problems");
    println!("last : show last problem");
    println!("all : show all problems");
    println!("newproblem : create a new problem");
    println!("merge : relax by merging two labels");
    println!("addarrow : relax by adding an arrow between two labels");
    println!("mergegroup : relax by merging multiple labels");
    println!("sd : relax by subdiagram merge");
    println!("speedup : apply RE");
    println!("hardenremove : harden by removing a label");
    println!("hardenkeep : harden by keeping specific labels");
    println!("mergeequivalent : merge equivalent labels");
    println!("criticalrelax : relax by critical sets");


}

fn status(problems : &Vec<Problem>) {
    println!("There are {} problems in the queue", problems.len());
}

fn pop(problems : &mut Vec<Problem>) {
    println!("Removing last problem");
    problems.pop();
}

fn clear(problems : &mut Vec<Problem>) {
    println!("Removing all problems");
    problems.clear();
}

fn labels_of_problem(p : &Problem) -> HashMap<Label,String> {
    p.mapping_label_text.iter().cloned().collect()
}

fn labelset_to_string(v : &Vec<Label>, mapping : &HashMap<Label,String>) -> String {
    v.iter().map(|l|&mapping[l]).join("")
}

fn retor_link(base : &str,p : &Problem) -> String {
    let s = p.to_string();
    let mut s = s.split("\n\n");
    let active = s.next().unwrap();
    let passive = s.next().unwrap();
    let record = serde_json::json!({
        "v": 2,
        "d": {
            "active": active,
            "passive": passive,
            "stuff": []
        }
    });
    let compressed = miniz_oxide::deflate::compress_to_vec(record.to_string().as_bytes(), 10);
    let encoded = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, compressed);
    format!(
        "{}/#{}",
        base,
        encoded
    )
}

fn show_diagram(p : &Problem){
    let labels = labels_of_problem(p);
    let config = termgraph::Config::new(termgraph::ValueFormatter::new(), 10).default_colors();
    
    let display_graph = {
        let mut tmp: termgraph::DirectedGraph<usize, &str> = termgraph::DirectedGraph::new();
        let edges = p.diagram_direct.clone().unwrap().1.into_iter().map(|(a,b)|(a as usize, b as usize));
        let nodes = p.diagram_direct.clone().unwrap().0.into_iter().map(|(l,_)|(l as usize,labels[&l].as_str()));
        tmp.add_nodes(nodes);
        tmp.add_edges(edges);

        tmp
    };
    termgraph::display(&display_graph, &config);
}


fn info(p : &Problem){
    println!("---------------------------------------------------------------------------------------");
    println!("REtor link: {}",retor_link("https://roundeliminator.github.io/re-experimental",p));
    println!("REtor link: {}",retor_link("http://127.0.0.1:8080/server",p));

    println!("\n{} Labels.\n",p.labels().len());

    let mapping = labels_of_problem(&p);
    println!("Labels:");
    for l in p.labels() {
        println!("{} -> {}",l,mapping[&l]);
    }

    let mut is_trivial = false;
    if let Some(trivial_sets) = p.trivial_sets.as_ref() {
        if trivial_sets.is_empty() {
            println!("The problem is NOT zero round solvable.");
        } else {
            is_trivial = true;
            println!("The problem IS zero round solvable: {}",trivial_sets.iter().map(|ts|labelset_to_string(&ts,&mapping)).join(" "));
        }
    }

    if let Some(coloring_sets) = p.coloring_sets.as_ref() {
        if coloring_sets.len() >= 2 {
            println!("The problem is solvable in zero rounds given a {} coloring: {}",coloring_sets.len(),coloring_sets.iter().map(|ts|labelset_to_string(&ts,&mapping)).join(" "));
        } else if !is_trivial {
            println!("The problem is NOT solvable even if given a 2-coloring.");
        }
    }

    let mergeable : Vec<_> = p.diagram_direct.clone().unwrap().0.into_iter().filter(|(_,v)|v.len() > 1).collect();
    if !mergeable.is_empty() {
        println!("\nThe following labels can be merged:");
        for (_,v) in mergeable {
            println!("{}",v.iter().map(|l|&mapping[l]).join(" "));
        }
    }
    println!("\n\nProblem:\n{}",p);

    println!("Diagram:\n");
    show_diagram(p);

    println!("\n\n---------------------------------------------------------------------------------------");

}

fn last(problems : &Vec<Problem>) {
    println!("Last problem in the queue:\n");

    let p = problems.last().unwrap();
    info(p);

}

fn all(problems : &Vec<Problem>) {
    println!("All problems in the queue:");
    for p in problems {
        info(p);
    }
}

fn new_problem(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the active constraints, then an empty line, then the passive constraints, then an empty line.");
    let active = stdin.take_while(|line|!line.is_empty()).collect_vec();
    let passive = stdin.take_while(|line|!line.is_empty()).collect_vec();
    let request = Request::NewProblem(active.join("\n"), passive.join("\n"));
    make_request(request, problems);
}

fn merge(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the first label number, newline, second label number");
    let from : Label = stdin.next().unwrap().parse().unwrap();
    let to : Label = stdin.next().unwrap().parse().unwrap();
    let last = problems.last().unwrap();
    let request = Request::SimplifyMerge(last.clone(),from,to);
    make_request(request, problems);
}

fn mergegroup(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the label numbers separated by newlines. They will be merged to the last one");
    let mut labels = stdin.take_while(|line|!line.is_empty())
        .map(|x|x.parse().unwrap())
        .collect_vec();
    let to = labels.pop().unwrap();
    let last = problems.last().unwrap();
    let request = Request::SimplifyMergeGroup(last.clone(),labels,to);
    make_request(request, problems);
}

fn addarrow(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the first label number, newline, second label number");
    let from : Label = stdin.next().unwrap().parse().unwrap();
    let to : Label = stdin.next().unwrap().parse().unwrap();
    let last = problems.last().unwrap();
    let request = Request::SimplifyAddarrow(last.clone(),from,to);
    make_request(request, problems);
}


fn simplifysd(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the SD rules without newlines, then an empty line, then true or false depending on whether the diagram should be recomputed at each step");
    let sd = stdin.take_while(|line|!line.is_empty()).join("\n");
    let recompute : bool = stdin.next().unwrap().parse().unwrap();
    let last = problems.last().unwrap();
    let request = Request::SimplifySD(last.clone(),sd,recompute);
    make_request(request, problems);
}


fn speedup(_stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    let last = problems.last().unwrap();
    let request = Request::Speedup(last.clone());
    make_request(request, problems);
}

fn merge_equivalent_labels(_stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    let last = problems.last().unwrap();
    let request = Request::MergeEquivalentLabels(last.clone());
    make_request(request, problems);
}

fn hardenremove(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the label number, then true or false depending on whether the removed label should be replaced by predecessors");
    let label : Label = stdin.next().unwrap().parse().unwrap();
    let replace : bool = stdin.next().unwrap().parse().unwrap();
    let last = problems.last().unwrap();
    let request = Request::HardenRemove(last.clone(),label, replace);
    make_request(request, problems);
}

fn hardenkeep(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type the label numbers separated by newlines, then a newline, then true or false depending on whether the removed label should be replaced by predecessors");
    let labels = stdin.take_while(|line|!line.is_empty())
        .map(|x|x.parse().unwrap())
        .collect_vec();
    let replace : bool = stdin.next().unwrap().parse().unwrap();
    let last = problems.last().unwrap();
    let request = Request::HardenKeep(last.clone(),labels,replace);
    make_request(request, problems);
}

fn critical_relax(stdin : &mut impl Iterator<Item=String>, problems : &mut Vec<Problem>){
    println!("type: b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, b_maximize_rename");
    let b_coloring : bool = stdin.next().unwrap().parse().unwrap();
    let coloring : usize = stdin.next().unwrap().parse().unwrap();
    let b_coloring_passive : bool = stdin.next().unwrap().parse().unwrap();
    let coloring_passive : usize = stdin.next().unwrap().parse().unwrap();
    let zerosteps : usize = stdin.next().unwrap().parse().unwrap();
    let b_maximize_rename : bool = stdin.next().unwrap().parse().unwrap();

    let last = problems.last().unwrap();
    let request = Request::CriticalRelax(last.clone(),b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, b_maximize_rename);
    make_request(request, problems);
}

fn prompt(){
    print!("> ");
    std::io::stdout().flush().unwrap();
}

fn load_state() -> Vec<Problem> {
    let file = std::env::args().nth(1).unwrap_or("re_shell_state".into());
    let state = std::fs::read_to_string(file).unwrap_or("".into());
    serde_json::from_str(&state).unwrap_or(vec![])
}

fn save_state(problems : &Vec<Problem>) {
    let file = std::env::args().nth(1).unwrap_or("re_shell_state".into());
    let serialized = serde_json::to_string(&problems).unwrap();
    std::fs::write(file, serialized).unwrap()
}

fn main() {
    #[cfg(not(target_os = "linux"))]
    unsafe{ libmimalloc_sys::mi_option_set(26, 0) }


    let stdin = std::io::stdin();
    let lines = stdin.lock().lines();
    let mut lines = lines.map(|x|x.unwrap());

    let mut problems = load_state();

    prompt();
    while let Some(line) = lines.next() {
        match line.to_lowercase().chars().filter(|c|c.is_alphabetic()).collect::<String>().as_str() {
            "help" => { help(); }
            "status" => { status(&problems); }
            "pop" => { pop(&mut problems); }
            "clear" => { clear(&mut problems); }
            "last" => { last(&problems); }
            "all" => { all(&problems); }
            "newproblem" => { new_problem(&mut lines, &mut problems) }
            "merge" => { merge(&mut lines,&mut problems) }
            "addarrow" => { addarrow(&mut lines,&mut problems) }
            "mergegroup" => { mergegroup(&mut lines,&mut problems) }
            "sd" => { simplifysd(&mut lines,&mut problems) }
            "speedup" => { speedup(&mut lines,&mut problems) }
            "hardenremove" => { hardenremove(&mut lines,&mut problems) }
            "hardenkeep" => { hardenkeep(&mut lines,&mut problems) }
            "mergeequivalent" => { merge_equivalent_labels(&mut lines, &mut problems) }
            "criticalrelax" => { critical_relax(&mut lines, &mut problems) }
            _ => { println!("unrecognized command"); }
        }

        save_state(&problems);
        prompt();
    }
    
}
