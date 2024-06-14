use itertools::{Itertools};
use rustsat::{instances::SatInstance, solvers::SolverResult, types::constraints::CardConstraint};

use crate::{constraint::Constraint, group::{Group, GroupType}, line::{Degree, Line}, part::Part, problem::Problem};
use rustsat::solvers::Solve;





impl Problem {

    pub fn apply_marks_technique(&mut self) {
        let r = self.marks();
        self.marks_works = Some(r);
    }

    pub fn marks(&self) -> bool {
        if self.passive.degree != Degree::Finite(2) {
            panic!("only works when the passive degree is 2");
        }
        
        let labels = self.labels();
        let labels_as_group = Group(labels.clone());
        let degree = self.active.finite_degree();
        let passive_degree = self.passive.finite_degree();

        println!("generating subsets");

        let mut subsets = vec![];
        let mut complements = vec![];
        for subset in labels.iter().cloned().powerset() {
            let subset_as_group = Group(subset.clone());
            let complement = labels_as_group.difference(&subset_as_group);
            complements.push(complement.0);
            subsets.push(subset);
        }


        println!("generating variables");


        let mut instance: SatInstance = SatInstance::new();
        let mut table = vec![];
        for _ in 0..degree {
            let mut row = vec![];
            for _ in 0..subsets.len() {
                row.push(instance.new_lit());
            }
            table.push(row);
        }

        println!("setting up node constraints");


        let node_choices = (0..degree).map(|_|0..subsets.len()).multi_cartesian_product();
        for (i,choice) in node_choices.enumerate() {
            if i % 1000000 == 0 { println!("{}",i); }
            let line = Line{parts:choice.iter().map(|j|Part{ 
                gtype: GroupType::Many(1),
                group: Group(subsets[*j].clone())
            }).collect()};
            if !self.active.exists_choice_in_line(&line) {
                let lits = choice.iter().enumerate().map(|(i,&j)|table[i][j]);
                instance.add_card_constr(CardConstraint::new_lb(lits, 1));
            }
        }

        println!("setting up edge constraints");


        for i in 0..degree {
            let edge_choices = (0..passive_degree).map(|_|0..subsets.len()).multi_cartesian_product();
            for choice in edge_choices {
                let line = Line{parts:choice.iter().map(|j|Part{ 
                    gtype: GroupType::Many(1),
                    group: Group(complements[*j].clone())
                }).collect()};
                if !self.passive.exists_choice_in_line(&line) {
                    let lits = choice.iter().map(|&j|table[i][j]);
                    instance.add_card_constr(CardConstraint::new_ub(lits, passive_degree-1));
                }
            }
        }

        println!("sanitizing");


        let instance = instance.sanitize();

        println!("calling the solver");

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut solver = rustsat_minisat::core::Minisat::default();
            solver.add_cnf(instance.into_cnf().0).unwrap();
            let res = solver.solve().unwrap();

            res == SolverResult::Unsat
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut instance = instance;
            let mut dimacs = std::io::BufWriter::new(Vec::new());
            instance.convert_to_cnf();
            instance.write_dimacs(&mut dimacs).unwrap();
            let dimacs = dimacs.into_inner().unwrap();
            let mut solver = varisat::solver::Solver::new();
            solver.add_dimacs_cnf(&dimacs[..]).unwrap();
            let solution = solver.solve().unwrap();
            !solution
        }
    }
}

impl Constraint {
    fn exists_choice_in_line(&self, line : &Line) -> bool {
        self.is_included_with_custom_supersets(line, Some(|g1 : &Group, g2 : &Group|{ !g1.intersection(g2).is_empty() }))
    }
}

#[test]
fn marks_test(){

    let p = Problem::from_string("A A X
B B Y

AX BY
XY XY").unwrap();
    assert!(!p.marks());

    let p = Problem::from_string("1 1 1
2 2 2
3 3 3
4 4 4
5 5 5
6 6 6
7 7 7
8 8 8
9 9 9

1 5689
2 369
3 258
4 789
5 1379
6 1278
7 456
8 1346
9 12456").unwrap();
    assert!(!p.marks());

    let p = Problem::from_string("1 1 1
2 2 2
3 3 3
4 4 4
5 5 5
6 6 6
7 7 7
8 8 8
9 9 9

1 5689
2 369
3 258
4 789
5 1379
6 1278
7 456
8 1346
9 1245").unwrap();
    assert!(p.marks());

    let p = Problem::from_string("A B B\n\nB AB").unwrap();
    assert!(p.marks());


    let p = Problem::from_string("A A A\n\nA A").unwrap();
    assert!(!p.marks());

}