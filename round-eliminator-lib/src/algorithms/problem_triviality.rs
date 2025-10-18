use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use permutator::Permutation;
use rustsat::types::{Lit, TernaryVal};
use rustsat::{instances::SatInstance, types::constraints::CardConstraint};
use rustsat::solvers::{Solve, SolverResult};

use crate::{
    algorithms::mapping_problem::mapping_problem::MappingProblem, constraint::Constraint, group::{Exponent, Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem
};

use super::event::EventHandler;

impl Problem {
    pub fn compute_triviality(&mut self, eh: &mut EventHandler) {
        if self.trivial_sets.is_some() {
            panic!("triviality has been computed already");
        }

        if self.passive.degree != Degree::Finite(2) {
            self.passive.maximize(eh);
        }

        if self.passive.lines.is_empty() {
            self.trivial_sets = Some(vec![]);
            return;
        }

        let passive_degree = match self.passive.lines[0].degree() {
            Degree::Finite(x) => GroupType::Many(x as Exponent),
            Degree::Star => GroupType::Star,
        };

        let active_sets = self.active.minimal_sets_of_all_choices();

        let mut trivial_sets = vec![];
        let num_active_sets = active_sets.len();

        for (i, set) in active_sets.into_iter().enumerate() {
            eh.notify("triviality", i, num_active_sets);

            let group = Group::from(set.into_iter().sorted().collect());
            let part = Part {
                gtype: passive_degree,
                group,
            };
            let mut line = Line { parts: vec![part] };
            if self.passive.includes(&line) {
                trivial_sets.push(line.parts[0].group.as_vec());
            }
        }

        self.trivial_sets = Some(trivial_sets);
    }

    pub fn compute_triviality_with_input(&mut self, other:Problem, sat : bool) {
        if other.labels().len() == 0 {
            self.is_trivial_with_input = Some(true);
            self.triviality_with_input = Some((vec![],vec![]));
            return;
        }
        if sat {
            self.compute_triviality_with_input_with_sat(other);
        } else {
            self.compute_triviality_with_input_without_sat(other);
        }
    }

    pub fn compute_triviality_with_input_without_sat(&mut self, other:Problem) {
        
        let mut mapping = MappingProblem::new(
            other.clone(),
            self.clone()
        );

        mapping.maximize_out_problem();

        if let Some(mapping) = mapping.search_for_mapping() {
            let other_label_to_text = other.mapping_label_text;


            let mapping : Vec<_> = mapping.into_iter().map(|(l,h)|{
                let v : Vec<_> = h.into_iter().sorted().collect();
                (l,v)
            }).collect();
            self.is_trivial_with_input = Some(true);
            self.triviality_with_input = Some((other_label_to_text,mapping));
        } else {
            self.is_trivial_with_input = Some(false);
        }
    }

    pub fn compute_triviality_with_input_with_sat(&mut self, input:Problem) {
        let (newinput,mapping) = input.make_all_labels_different();

        let squish_labels : HashMap<_,_> = self.labels().iter().enumerate().map(|(i,&l)|(l,i as Label)).collect();

        let all_lines = |c : &Constraint, permutations : bool, squish : Option<&HashMap<Label,Label>>| -> Vec<Vec<Label>> {
            c.all_choices(false).into_iter().flat_map(|line|{
                let parts_to_labels = |parts : &Vec<Part>|{
                    parts.iter().map(|part|{
                        let l = part.group.first();
                        if let Some(squish) = squish {
                            squish[&l]
                        } else {
                            l
                        }
                    }).collect() 
                };
                let mut parts = line.parts.clone();
                if !permutations {
                    vec![parts_to_labels(&parts)].into_iter()
                } else {
                    parts.permutation().map(|perm|{
                        parts_to_labels(&perm)
                    }).collect::<Vec<_>>().into_iter()
                }
            }).unique().collect()
        };

        let input_active_lines = all_lines(&newinput.active, false, None);
        let input_passive_lines = all_lines(&newinput.passive, false, None);
        let problem_active_lines = all_lines(&self.active, true, Some(&squish_labels));
        let problem_passive_lines = all_lines(&self.passive, true, Some(&squish_labels));

        let problem_num_labels = squish_labels.len();
        let input_num_labels = newinput.labels().len();

        //println!("{:?}\n{:?}",input_active_lines,input_passive_lines);
        //println!("{:?}\n{:?}",problem_active_lines,problem_passive_lines);

        let mut instance: SatInstance = SatInstance::new();

        let all_node_configs_mapped = instance.new_lit();
        let all_edge_configs_mapped = instance.new_lit();
        instance.add_unit(all_node_configs_mapped);
        instance.add_unit(all_edge_configs_mapped);

        let ith_node_config_mapped : Vec<_> = (0..input_active_lines.len()).map(|_|instance.new_lit()).collect();
        let ith_edge_config_mapped : Vec<_> = (0..input_passive_lines.len()).map(|_|instance.new_lit()).collect();
        instance.add_lit_impl_cube(all_node_configs_mapped, &ith_node_config_mapped);
        instance.add_lit_impl_cube(all_edge_configs_mapped, &ith_edge_config_mapped);

        let ith_node_mapped_to_jth_node : Vec<Vec<_>> = (0..input_active_lines.len()).map(|_|
                (0..problem_active_lines.len()).map(|_|instance.new_lit()).collect()
            ).collect();
        let ith_edge_mapped_to_jth_edge : Vec<Vec<_>> = (0..input_passive_lines.len()).map(|_|
            (0..problem_passive_lines.len()).map(|_|instance.new_lit()).collect()
        ).collect();

        for i in 0..ith_node_config_mapped.len() {
            instance.add_lit_impl_clause(ith_node_config_mapped[i], &ith_node_mapped_to_jth_node[i]);
        }
        for i in 0..ith_edge_config_mapped.len() {
            instance.add_lit_impl_clause(ith_edge_config_mapped[i], &ith_edge_mapped_to_jth_edge[i]);
        }

        let ith_label_mapped_to_jth_label : Vec<Vec<_>> = (0..input_num_labels).map(|_|
            (0..problem_num_labels).map(|_|instance.new_lit()).collect()
        ).collect();

        for i in 0..input_active_lines.len() {
            for j in 0..problem_active_lines.len() {
                let d = input_active_lines[i].len();
                let cube : Vec<_> = (0..d).map(|k|{
                    let input_label = input_active_lines[i][k];
                    let output_label = problem_active_lines[j][k];
                    ith_label_mapped_to_jth_label[input_label as usize][output_label as usize]
                }).collect();
                instance.add_lit_impl_cube(ith_node_mapped_to_jth_node[i][j],&cube);
            }
        }

        for i in 0..input_passive_lines.len() {
            for j in 0..problem_passive_lines.len() {
                let d = input_passive_lines[i].len();
                let cube : Vec<_> = (0..d).map(|k|{
                    let input_label = input_passive_lines[i][k];
                    let output_label = problem_passive_lines[j][k];
                    ith_label_mapped_to_jth_label[input_label as usize][output_label as usize]
                }).collect();
                instance.add_lit_impl_cube(ith_edge_mapped_to_jth_edge[i][j],&cube);
            }
        }

        for i in 0..input_num_labels {
            instance.add_card_constr(CardConstraint::new_eq(ith_label_mapped_to_jth_label[i].iter().cloned(),1));
        }

        let instance = instance.sanitize();
        let lits : Vec<_> = ith_label_mapped_to_jth_label.iter().flat_map(|v|v.into_iter().cloned()).collect();
        if let Some(solution) = solve_sat(instance, &lits) {
            let true_lits : HashSet<_> = solution.into_iter().collect();
            let mut label_mapping = vec![];

            for i in 0..input_num_labels {
                for j in 0..problem_num_labels {
                    if true_lits.contains(&ith_label_mapped_to_jth_label[i][j]) {
                        label_mapping.push((i as Label,j as Label));
                    }
                }
            }

            self.is_trivial_with_input = Some(true);

            let input_reverse_mapping : HashMap<_,_> = mapping.iter().flat_map(|(l,v)|v.iter().map(|x|(*x,*l))).collect();
            let problem_reverse_mapping : HashMap<_,_> = squish_labels.iter().map(|(a,b)|(*b,*a)).collect();
            let mut input_mapping : HashMap<Label, HashSet<Label>>= HashMap::new();
            for (i,j) in label_mapping {
                let i = input_reverse_mapping[&i];
                let j = problem_reverse_mapping[&j];
                input_mapping.entry(i).or_default().insert(j);
            }

            let input_mapping = input_mapping.into_iter().map(|(l,h)|(l,h.into_iter().collect())).collect();

            self.triviality_with_input = Some((input.mapping_label_text.clone(),input_mapping));

        } else {
            self.is_trivial_with_input = Some(false);
        }
        



    }

    pub fn make_all_labels_different(&self) -> (Self,HashMap<Label,Vec<Label>>) {
        self.make_some_labels_different(&self.labels(),false)
    }


    pub fn make_some_labels_different(&self, labels : &Vec<Label>, same_on_same_line : bool) -> (Self,HashMap<Label,Vec<Label>>) {
        let mut next_label = if labels == &self.labels() {
            0
        } else {
            *self.labels().iter().max().unwrap_or(&0) + 1
        };
        let labels : HashSet<_> = labels.into_iter().cloned().collect();

        let mut map : HashMap<Label, Vec<Label>>= HashMap::new();

        let mut active = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.active.degree,
        };
        for line in self.active.all_choices(false) {
            let mut already_dup : HashMap<Label, Label> = HashMap::new();
            let mut line = line.edited(|g|{
                let mut new_g = vec![];
                for &l in g.iter() {
                    if labels.contains(&l) {
                        if same_on_same_line {
                            if already_dup.contains_key(&l) {
                                new_g.push(already_dup[&l]);
                            } else {
                                map.entry(l).or_default().push(next_label);
                                new_g.push(next_label);
                                already_dup.insert(l,next_label);
                                next_label += 1;
                            }
                        } else {
                            map.entry(l).or_default().push(next_label);
                            new_g.push(next_label);
                            next_label += 1;
                        }
                    } else {
                        new_g.push(l);
                    }
                }
                Group::from(new_g)
            });
            line.normalize();
            active.lines.push(line);
        }

        let passive = self.passive.edited(|g|{
            Group::from(g.iter().flat_map(|l|{
                if map.contains_key(l) {
                    map[l].iter().cloned().collect_vec().into_iter()
                } else {
                    vec![*l].into_iter()
                }
            }).collect())
        });

        let old_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let mut mapping_label_text : Vec<_> = map.iter().flat_map(|(l,v)|{
            let old = old_text[l].replace("(", "").replace(")", "");
            v.iter().enumerate().map(move |(i,new)|(*new,format!("({}_{})",old,i)))
        }).collect();
        mapping_label_text.extend(old_text.into_iter());
        (Problem {             
            active,
            passive,
            passive_gen : None,
            mapping_label_text,
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: None,
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_indirect_old: None,
            diagram_direct: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: None,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None,
            is_trivial_with_input : None,
            triviality_with_input : None,
            expressions : None
        },map)
    }
    
    pub fn compute_subinput_that_gives_nontriviality_aux<F>(&mut self, input : Problem, sat : bool, smallest : Label, seen : &mut HashMap<Problem,Label>, f : &mut F) -> Option<Problem> where F : FnMut(Problem) {
        if let Some(x) = seen.get(&input) {
            if *x <= smallest {
                return None;
            }
        }
        seen.insert(input.clone(), smallest);

        self.compute_triviality_with_input(input.clone(), sat);
        if self.is_trivial_with_input.unwrap() {
            return None;
        }

        let mut labels = input.labels();
        labels.sort();
        let mut best = input.clone();

        f(input.clone());

        for &l in &labels {
            if l < smallest {
                continue;
            }
            let subset : HashSet<_> = labels.iter().cloned().filter(|&x|x!=l).collect();
            let mut subinput = input.harden_keep(&subset, true);
            subinput.discard_useless_stuff(false, &mut EventHandler::null());
            if let Some(p) = self.compute_subinput_that_gives_nontriviality_aux(subinput.clone(),sat,l+1, seen, f) {    
                if p.labels().len() < best.labels().len() {
                    best = p;
                }   
            }
        }

        Some(best)
    }

    pub fn compute_subinput_that_gives_nontriviality<F>(&mut self, input : Problem, sat : bool, mut f : F) -> Option<Problem> where F : FnMut(Problem){
        let mut seen = HashMap::new();
        self.compute_subinput_that_gives_nontriviality_aux(input, sat, 0, &mut seen,&mut f)
    }
}

pub fn solve_sat(instance : SatInstance, lits : &[Lit]) -> Option<Vec<Lit>> {


    #[cfg(feature = "all")]
    {
        let mut solver = rustsat_minisat::core::Minisat::default();
        solver.add_cnf(instance.into_cnf().0).unwrap();
        let res = solver.solve().unwrap();
        if res != SolverResult::Sat {
            return None;
        }
        let solution = solver.full_solution().unwrap();
        let true_vars = lits.iter().filter(|lit|{
            solution.lit_value(**lit) == TernaryVal::True
        }).cloned().collect_vec();
        return Some(true_vars);        
    }

    #[cfg(feature = "onlyrust")]
    {
        let mut instance = instance;
        let mut dimacs = std::io::BufWriter::new(Vec::new());
        instance.convert_to_cnf();
        instance.write_dimacs(&mut dimacs).unwrap();
        let dimacs = dimacs.into_inner().unwrap();
        let mut solver = varisat::solver::Solver::new();
        solver.add_dimacs_cnf(&dimacs[..]).unwrap();
        let solution = solver.solve().unwrap();
        if !solution {
            return None;
        }
        let lits = solver.model().unwrap().into_iter().filter(|lit|lit.is_positive());
        let lits = lits.map(|lit|Lit::new(lit.to_dimacs() as u32, false)).collect();
        
        return Some(lits);
    }
}


#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn triviality() {
        let mut p = Problem::from_string("M U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A AB AB\n\nA A\nB B").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B\nAB AB").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());
    }

    #[test]
    fn triviality_sat(){
        let mut p = Problem::from_string("M M M\nP U U\n\nM UP\nU U").unwrap();
        let input = Problem::from_string("A A A\nB B B\n\nA B").unwrap();
        p.compute_triviality_with_input_with_sat(input);

        let mut p = Problem::from_string("M M M\nP U U\n\nM UP\nU U").unwrap();
        let input = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C").unwrap();
        p.compute_triviality_with_input_with_sat(input);
    }

}
