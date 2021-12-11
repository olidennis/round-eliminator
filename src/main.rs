mod algorithms;
mod constraint;
mod group;
mod line;
mod part;
mod problem;

use crate::problem::Problem;

fn main() {
    env_logger::init();

    let mut p = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD").unwrap();
    p.compute_triviality();
    p.compute_diagram();
    p.compute_coloring_solvability();
    p.remove_weak_active_lines();
}
