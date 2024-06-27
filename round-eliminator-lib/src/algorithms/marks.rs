use std::{collections::HashMap, time::Instant};

use bit_vec::BitVec;
use itertools::{Itertools};
use rand::Rng;
use rustsat::{instances::SatInstance, solvers::SolverResult, types::{constraints::CardConstraint, Lit}};

use crate::{constraint::Constraint, group::{Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem};
use rustsat::solvers::Solve;

use super::event::EventHandler;

use rand::seq::SliceRandom;

type TableIndex = u32;

fn mark_exists<T,F>(choice : &Vec<TableIndex>, successors : &Vec<Vec<usize>>, handled : &mut BitVec, num_handled : &mut usize, choice_to_index : &T, set_to_string : &F, subsets : &Vec<Vec<Label>>, complements : &Vec<Vec<Label>>) where T : Fn(&Vec<TableIndex>) -> usize, F : Fn(usize, &Vec<u32>) -> String{
    if handled.get(choice_to_index(choice)).unwrap() {
        return;
    }

    handled.set(choice_to_index(choice),true);
    *num_handled += 1;
    //println!("{}",choice.iter().enumerate().map(|(i,set_index)|set_to_string(i,&subsets[*set_index as usize])).join(" "));

    let mut choice = choice.clone();

    for i in 0..choice.len() {
        let orig_i = choice[i];
        for &successor in &successors[orig_i as usize] {
            choice[i] = successor as TableIndex;
            mark_exists(&choice, successors, handled, num_handled, choice_to_index, set_to_string, subsets, complements);
        }
        choice[i] = orig_i;
    }
}

fn mark_no_exists<T,F>(choice : &Vec<TableIndex>, successors : &Vec<Vec<usize>>, handled : &mut BitVec, num_handled : &mut usize, table : &Vec<Vec<Lit>>, instance : &mut SatInstance, choice_to_index : &T, set_to_string : &F, subsets : &Vec<Vec<Label>>, complements : &Vec<Vec<Label>>) where T : Fn(&Vec<TableIndex>) -> usize, F : Fn(usize, &Vec<u32>) -> String{
    if handled.get(choice_to_index(choice)).unwrap() {
        return;
    }

    let lits = choice.iter().enumerate().map(|(i,&j)|table[i][j as usize]);
    instance.add_card_constr(CardConstraint::new_lb(lits, 1));

    handled.set(choice_to_index(choice),true);
    *num_handled += 1;

    let mut choice = choice.clone();

    for i in 0..choice.len() {
        let orig_i = choice[i];
        for &successor in &successors[orig_i as usize] {
            choice[i] = successor as TableIndex;
            mark_no_exists(&choice, successors, handled, num_handled, table, instance, choice_to_index, set_to_string, subsets, complements);
        }
        choice[i] = orig_i;
    }
}

fn handle_choice_nodes<T,F>(active : &Constraint, choice : &Vec<TableIndex>, predecessors : &Vec<Vec<usize>>, successors : &Vec<Vec<usize>>, handled : &mut BitVec, num_handled : &mut usize, table : &Vec<Vec<Lit>>, instance : &mut SatInstance, subsets : &Vec<Vec<Label>>, complements : &Vec<Vec<Label>>, no_marking : bool, choice_to_index : &T, set_to_string : &F) where T : Fn(&Vec<TableIndex>) -> usize, F : Fn(usize, &Vec<u32>) -> String{
    if handled.get(choice_to_index(choice)).unwrap() {
        return;
    }

    let line = Line{parts:choice.iter().map(|j|Part{ 
        gtype: GroupType::Many(1),
        group: Group(subsets[*j as usize].clone())
    }).collect()};

    if no_marking {
        if !active.exists_choice_in_line(&line) {
            let lits = choice.iter().enumerate().map(|(i,&j)|table[i][j as usize]);
            instance.add_card_constr(CardConstraint::new_lb(lits, 1));
            //handled.insert(choice.clone());
        } else {
            //println!("{}",choice.iter().enumerate().map(|(i,set_index)|set_to_string(i,&subsets[*set_index as usize])).join(" "));
        }
    } else {
        if !active.exists_choice_in_line(&line) {
            mark_no_exists(&choice,&predecessors,handled, num_handled, &table, instance, choice_to_index, set_to_string, subsets, complements);
        } else {
            mark_exists(&choice,&successors, handled, num_handled, choice_to_index, set_to_string, subsets, complements);
        }
    }
}

impl Problem {

    pub fn apply_marks_technique(&mut self, eh: &mut EventHandler) {
        let r = self.marks(eh);
        self.marks_works = Some(r);
    }

    pub fn marks(&self, eh: &mut EventHandler) -> bool {
        if self.passive.degree != Degree::Finite(2) {
            panic!("only works when the passive degree is 2");
        }
        
        let labels = self.labels();
        if labels.len() > std::mem::size_of::<TableIndex>() * 8 {
            panic!("too many labels");
        }
        let labels_as_group = Group(labels.clone());
        let degree = self.active.finite_degree();
        let passive_degree = self.passive.finite_degree();

        let h : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let set_to_string = |i : usize, set : &Vec<Label>|{
            format!("({}_{})",i,set.iter().map(|x|&h[x]).join(""))
        };

        //println!("generating subsets");

        let mut subsets = vec![];
        let mut subsets_as_groups = vec![];
        let mut complements = vec![];
        for subset in labels.iter().cloned().powerset() {
            let subset_as_group = Group(subset.clone());
            let complement = labels_as_group.difference(&subset_as_group);
            complements.push(complement.0);
            subsets.push(subset);
            subsets_as_groups.push(subset_as_group);
        }


        let mut predecessors = vec![];
        let mut successors = vec![];
        for subset in 0..subsets.len() {
            let mut v_pred = vec![];
            let mut v_succ = vec![];
            for other in 0..subsets.len() {
                if subsets_as_groups[subset].is_superset(&subsets_as_groups[other]) && subsets_as_groups[subset].len() == subsets_as_groups[other].len()+1 {
                    v_pred.push(other);
                }
                if subsets_as_groups[other].is_superset(&subsets_as_groups[subset]) && subsets_as_groups[other].len() == subsets_as_groups[subset].len()+1 {
                    v_succ.push(other);
                }
            }
            predecessors.push(v_pred);
            successors.push(v_succ);
        }

        //println!("generating variables");


        let mut instance: SatInstance = SatInstance::new();
        let mut table = vec![];
        for _ in 0..degree {
            let mut row = vec![];
            for _ in 0..subsets.len() {
                row.push(instance.new_lit());
            }
            table.push(row);
        }

        let node_choices = (0..degree).map(|_|0 as TableIndex..subsets.len() as TableIndex).multi_cartesian_product();
        let len = node_choices.clone().count();
        let mut last_notify = chrono::Utc::now().time();
        let mut rng = rand::thread_rng();
        let mut sample = node_choices.clone().filter(|_|rng.gen_bool(0.02)).collect_vec();
        sample.shuffle(&mut rng);

        let mut handled = BitVec::from_elem(len, false);
        let mut num_handled = 0;

        let choice_to_index = |v : &Vec<TableIndex>|{
            let mut p = 0;
            for x in v {
                p *= subsets.len();
                p += *x as usize;
            }
            p
        };

        for (i,choice) in sample.iter().enumerate() {
            let now = chrono::Utc::now().time();
            if (now - last_notify).num_milliseconds() > 100 {
                eh.notify("setting up node constraints",num_handled,len);
                last_notify = chrono::Utc::now().time();

                //println!("{} {}",i,num_handled);
            }
            
            handle_choice_nodes(&self.active, &choice, &predecessors, &successors, &mut handled, &mut num_handled, &table, &mut instance, &subsets, &complements, false, &choice_to_index, &set_to_string);
            if num_handled > (0.999 * len as f32) as usize {
                break;
            }
        }
        sample.clear();
        
        //handled.shrink_to_fit();
        //println!("done prefiltering");
        let node_choices = (0..degree).map(|_|0 as TableIndex..subsets.len() as TableIndex).multi_cartesian_product();
        for (i,choice) in node_choices.enumerate() {

            let now = chrono::Utc::now().time();
            if (now - last_notify).num_milliseconds() > 100 {
                eh.notify("setting up node constraints",i,len);
                last_notify = chrono::Utc::now().time();

                //println!("{} {}",i,num_handled);
            }
            
            handle_choice_nodes(&self.active, &choice, &predecessors, &successors, &mut handled, &mut num_handled, &table, &mut instance, &subsets, &complements, true, &choice_to_index, &set_to_string);
        }
        //println!("");
        //println!("setting up edge constraints");

        let edge_choices = (0..passive_degree).map(|_|0..subsets.len()).multi_cartesian_product();
        // .filter(|v|v[0] == v[1]); // in many cases this is sufficient to get a LB, in this case a sat solver would not be even needed
        let len = edge_choices.clone().count();

        for i in 0..degree {
            let edge_choices = edge_choices.clone();
            for (k,choice) in edge_choices.enumerate() {

                let now = chrono::Utc::now().time();
                if (now - last_notify).num_milliseconds() > 100 {
                    eh.notify("setting up edge constraints",i*len + k,degree*len);
                    last_notify = chrono::Utc::now().time();
                }

                let line = Line{parts:choice.iter().map(|j|Part{ 
                    gtype: GroupType::Many(1),
                    group: Group(complements[*j].clone())
                }).collect()};
                if !self.passive.exists_choice_in_line(&line) {
                    let lits = choice.iter().map(|&j|table[i][j]);
                    instance.add_card_constr(CardConstraint::new_ub(lits, passive_degree-1));
                } else {
                    //println!("{}",choice.iter().map(|set_index|set_to_string(i,&subsets[*set_index])).join(" "));
                }
            }
        }

        //println!("");
        eh.notify("sanitizing",0,0);

        let instance = instance.sanitize();

        eh.notify("calling the sat solver",0,0);

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
    let eh = &mut EventHandler::null();

    let p = Problem::from_string("A A X
B B Y

AX BY
XY XY").unwrap();
    println!("1");
    assert!(!p.marks(eh));

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
    println!("2");
    //assert!(!p.marks(eh));

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
    println!("3");
    //assert!(p.marks(eh));

    let p = Problem::from_string("A B B\n\nB AB").unwrap();
    println!("4");
    assert!(p.marks(eh));


    let p = Problem::from_string("A A A\n\nA A").unwrap();
    println!("5");
    assert!(!p.marks(eh));

    let p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C").unwrap();
    println!("6");
    assert!(p.marks(eh));

}