mod algorithms;
mod constraint;
mod group;
mod line;
mod part;
mod problem;

use std::collections::HashSet;

use crate::problem::Problem;

fn main() {
    env_logger::init();

    let mut p = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD").unwrap();
    p.discard_useless_stuff(true);
    p.compute_triviality();
    p.compute_coloring_solvability();
    println!("{}",p);

    let mut p = p.relax_merge(0,1);
    p.discard_useless_stuff(true);
    p.compute_triviality();
    p.compute_coloring_solvability();
    println!("{}",p);

    let mut p = p.harden(&HashSet::from([1,2]), true);
    p.discard_useless_stuff(true);
    p.compute_triviality();
    p.compute_coloring_solvability();
    println!("{}",p);

    let p = p.relax_addarrow(1, 2);
    println!("{}",p);

    let mut p = p.relax_addarrow(2, 1);
    p.discard_useless_stuff(true);
    println!("{}",p);

    let mut p = p.merge_equivalent_labels();
    println!("{}",p);

    p.compute_set_inclusion_diagram();

}
