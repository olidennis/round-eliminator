mod algorithms;
mod constraint;
mod group;
mod line;
mod part;
mod problem;

use std::collections::{HashSet};

use crate::{problem::Problem, algorithms::event::EventHandler};

fn main() {
    env_logger::init();
    let eh = EventHandler::null();

    let mut p = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD").unwrap();
    p.discard_useless_stuff(true, &eh);
    p.compute_triviality(&eh);
    p.compute_coloring_solvability(&eh);
    println!("{}", p);

    let mut p = p.relax_merge(0, 1);
    p.discard_useless_stuff(true, &eh);
    p.compute_triviality(&eh);
    p.compute_coloring_solvability(&eh);
    println!("{}", p);

    let mut p = p.harden(&HashSet::from([1, 2]), true);
    p.discard_useless_stuff(true, &eh);
    p.compute_triviality(&eh);
    p.compute_coloring_solvability(&eh);
    println!("{}", p);

    let p = p.relax_addarrow(1, 2);
    println!("{}", p);

    let mut p = p.relax_addarrow(2, 1);
    p.discard_useless_stuff(true, &eh);
    println!("{}", p);

    let mut p = p.merge_equivalent_labels();
    println!("{}", p);

    p.compute_set_inclusion_diagram();
    p.rename(&[]).unwrap();
    p.sort_active_by_strength();
    
    /* 
    let s = std::fs::read_to_string("test.txt").unwrap();
    let mut p = Problem::from_string(s).unwrap();
    let label : HashMap<_,_> = p.mapping_label_text.iter().map(|(l,s)|(s.clone(),*l)).collect();
    let p = p.relax_merge(label["S"],label["0"]);
    let p = p.relax_merge(label["2"],label["5"]);
    let p = p.relax_merge(label["T"],label["5"]);
    let p = p.relax_merge(label["4"],label["5"]);
    let p = p.relax_merge(label["p"],label["5"]);
    let p = p.relax_merge(label["u"],label["5"]);
    let p = p.relax_merge(label["l"],label["5"]);
    let p = p.relax_merge(label["z"],label["5"]);
    let p = p.relax_merge(label["1"],label["5"]);
    let p = p.relax_merge(label["o"],label["5"]);
    let p = p.relax_merge(label["t"],label["5"]);
    let p = p.relax_merge(label["k"],label["5"]);
    let p = p.relax_merge(label["y"],label["5"]);


    let mut p = p;
    p.discard_useless_stuff(false);


    println!("{}",p);*/

    
}
