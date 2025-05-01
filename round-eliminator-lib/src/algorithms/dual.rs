use std::collections::{HashMap, HashSet};

use env_logger::init;
use itertools::Itertools;
use serde_json::map;

use crate::{algorithms::choices::left_labels, constraint::Constraint, group::{Group, GroupType, Label}, line::Line, part::Part, problem::Problem};

use super::diagram::{compute_direct_diagram, diagram_direct_to_pred_adj, diagram_direct_to_succ_adj, diagram_indirect_to_reachability_adj};


impl Line {
    fn iter_labels(&self) -> impl Iterator<Item=Label> {
        let mut labels = vec![];
        for part in &self.parts {
            assert!(part.group.len() == 1);
            for _ in 0..part.gtype.value() {
                labels.push(part.group.first());
            }
        }
        labels.into_iter()
    }
}


fn k_partitions(k : usize, n : usize) -> impl Iterator<Item=Vec<usize>> {
    (0..n).map(|_|0..k)
        .multi_cartesian_product()
}

fn dual_diagram(labels_p : &Vec<Label>, labels_d : &Vec<Vec<usize>>, labels_fp : &Vec<Label>, diagram_fp : &Vec<(Label,Label)>) -> Vec<(Label,Label)> {
    let labels_d : HashMap<_,_> = labels_d.iter().cloned().enumerate().collect();
    let labels_p_to_positions : HashMap<_,_> = labels_p.iter().copied().enumerate().map(|(i,l)|(l,i)).collect();

    let successors_fp = diagram_indirect_to_reachability_adj(labels_fp, diagram_fp);
    let mut diagram = vec![];

    for (l1,v1) in labels_d.iter() {
        for (l2,v2) in labels_d.iter() {
            let arrow_present = labels_p.iter().all(|l|{
                let fp_l1 = v1[labels_p_to_positions[l]] as Label;
                let fp_l2 = v2[labels_p_to_positions[l]] as Label;
                successors_fp[&fp_l1].contains(&fp_l2)
            });

            if arrow_present {
                diagram.push((*l1 as Label,*l2 as Label));
            }
        }
    }

    diagram
}

fn dual_constraint(cp : &Constraint, cf : &Constraint, labels : &Vec<Vec<usize>>, labels_p : &Vec<Label>, all_predecessors : &HashMap<Label, HashSet<Label>>, all_successors : &HashMap<Label, HashSet<Label>>) -> Constraint {
    let labels_p_to_positions : HashMap<_,_> = labels_p.iter().copied().enumerate().map(|(i,l)|(l,i)).collect();
    let d = cp.finite_degree();
    let labels_d = all_successors.keys().copied().collect_vec();
    let source = *labels_d.iter().filter(|l|all_predecessors[l].len() <= 1 && all_predecessors[l].iter().filter(|x|x!=l).count() == 0).next().unwrap();

    let to_vector = |line : &Line|{
        line.iter_labels().collect_vec()
    };

    let to_configuration = |line : &Vec<Label>|{
        let parts = line.into_iter().map(|l|{
            Part{
                gtype: GroupType::Many(1),
                group: Group::from(vec![*l]),
            }
        }).collect_vec();
        Line{ parts }
    };

    let is_linep_bad_for_lined = |line_d : &Vec<Label>, line_p : &Vec<Label>|{
        let parts_f = line_p.iter().enumerate().map(|(i,&l)|{
            Part{
                gtype : GroupType::Many(1),
                group : Group::from(vec![labels[line_d[i] as usize][labels_p_to_positions[&l]] as Label])
            }
        }).collect_vec();
        let line_f = Line{ parts : parts_f };
        !cf.includes(&line_f)
    };

    let find_bad_linep_for_lined = |line_d : &Vec<Label>|{
        cp.all_choices(false).into_iter().flat_map(|line_p|to_vector(&line_p).into_iter().permutations(d))
                .find(|line_p|{
                    is_linep_bad_for_lined(line_d,line_p)
                })
    };

    let is_right = |l1:Label,l2:Label|{
        all_successors[&l2].contains(&l1)
    };
    
    println!("starting");
    let initial_configuration = std::iter::repeat(source).take(d).collect_vec();
    let mut good_configurations = HashSet::new();
    let mut tofix_configurations : HashSet<_> = HashSet::from_iter(vec![initial_configuration].into_iter());
    let mut seen = HashSet::new();

    while !tofix_configurations.is_empty() {
        println!("size: {}, good: {}",tofix_configurations.len(),good_configurations.len());
        let mut new_tofix_configurations = HashSet::new();
        for mut configuration in tofix_configurations {
            if let Some(bad) = find_bad_linep_for_lined(&configuration) {
                for i in 0..d {
                    let mut set = vec![];
                    for &succ in &all_successors[&configuration[i]] {
                        let mut new_configuration = configuration.clone();
                        new_configuration[i] = succ;
                        if !is_linep_bad_for_lined(&new_configuration,&bad) {
                            set.push(succ);
                        }
                    }
                    let left = left_labels(&set, is_right);
                    for &succ in &left {
                        let mut new_configuration = configuration.clone();
                        new_configuration[i] = succ;
                        new_configuration.sort();
                        if !seen.contains(&new_configuration) {
                            seen.insert(new_configuration.clone());
                            new_tofix_configurations.insert(new_configuration);
                        }
                    }
                }
            } else {
                configuration.sort();
                good_configurations.insert(configuration);
            }
        }
        tofix_configurations = new_tofix_configurations;
    }
    println!("done");

    //let lines = (0..d).map(|_|0..labels.len() as Label).multi_cartesian_product()
    //    .filter(|line_d|line_d.is_sorted())
    //    .filter(|line_d|line_d_is_good(line_d))
    let lines = good_configurations.into_iter()
        .map(|line|to_configuration(&line)).collect_vec();
    let c = Constraint { lines, is_maximized: false, degree: crate::line::Degree::Finite(d)  };
    c.edited(|g|{
        let g = g.iter().fold(HashSet::new(),|mut a,l|{
            a.extend(all_successors[l].iter().cloned());
            a
        });
        Group::from_set(&g)
    })
}

impl Problem {




    pub fn dual_problem(&self, f : &Problem) -> Problem {
        let mut f = f.clone();
        f.add_active_predecessors();
        f.active.is_maximized = true;

        let labels_f = f.labels();
        let labels_p = self.labels();

        let dual_labels_v = k_partitions(labels_f.len(), labels_p.len()).collect_vec();

        let d_diag = dual_diagram(&self.labels(), &dual_labels_v, &f.labels(), &f.diagram_indirect.as_ref().unwrap());
        let d_labels = (0..dual_labels_v.len()).map(|x|x as Label).collect_vec();
        let all_succ = diagram_indirect_to_reachability_adj(&d_labels, &d_diag);
        let inv = d_diag.iter().map(|&(a,b)|(b,a)).collect_vec();
        let all_pred = diagram_indirect_to_reachability_adj(&d_labels, &inv);


        
        let dual_active = dual_constraint(&self.active, &f.active, &dual_labels_v, &labels_p, &all_succ, &all_pred);
        let dual_passive = dual_constraint(&self.passive, &f.passive, &dual_labels_v, &labels_p, &all_pred, &all_succ);

        let active_labels = dual_active.labels_appearing();
        let passive_labels = dual_passive.labels_appearing();
        let dual_labels = active_labels.union(&passive_labels).collect_vec();

        let mapping_label_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();

        let mapping_label_text = dual_labels.into_iter().map(|&l|{
            let v = &dual_labels_v[l as usize];
            let mapped_to_x = |x : Label| {
                labels_p.iter().zip(v.iter())
                    .filter(|(_,&j)|j as Label == x)
                    .map(|(&l,_)|l)
                    .collect_vec()
            };
            let v_to_string = |v : &Vec<Label>|{
                v.iter().map(|l|mapping_label_text[l].replace("(", "[").replace(")","]")).join("")
            };
            let s = labels_f.iter().map(|&x|v_to_string(&mapped_to_x(x))).join("_");
            (l,format!("({})",s))
        }).collect_vec();


        Problem {
            active : dual_active,
            passive : dual_passive,
            passive_gen : None,
            mapping_label_text,
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: None,
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: None,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None,
            is_trivial_with_input : None,
            triviality_with_input : None
        }
    }


}



#[cfg(test)]
mod tests {
    use crate::{algorithms::{dual::dual_diagram, event::EventHandler}, problem::Problem, serial::fix_problem};


    #[test]
    fn dual() {
        /*let eh = &mut EventHandler::with(|(s,a,b)|{
            println!("{} {} {}",s,a,b);
        });*/
        let eh = &mut EventHandler::null();



        let mut p = Problem::from_string("A B^2
(123) X^2
(12)^2 X
(13)^2 X
1^3
(23)^2 X
2^3
3^3

B A(123)(12)(13)1(23)23XB
XB (123)(12)(13)1(23)23XB
2XB (13)13XB
3XB (12)12XB
1XB (23)23XB").unwrap();




let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C\n").unwrap();
let mut p = Problem::from_string("A B C\n\nA A\nB B\nC C\n").unwrap();
let mut p = Problem::from_string("A B^2
1 2 3

3B^2
1B^2
2B^2
B A123B").unwrap();

        let mut f = Problem::from_string("A B B\n\nB AB").unwrap();

        p.passive.maximize(eh);
        f.passive.maximize(eh);
        p.compute_diagram(eh);
        f.compute_diagram(eh);



        let mut dual = p.dual_problem(&f);
        fix_problem(&mut dual, true, false, eh);
        let mut dual = dual.merge_subdiagram("",true,eh).unwrap();
        println!("{}",dual);

        dual.compute_triviality(eh);
        if !dual.trivial_sets.as_ref().unwrap().is_empty() {
            println!(":( trivial");
            return;
        }

        let dual = dual.speedup(eh);
        let dual = dual.speedup(eh);
        f.compute_triviality_with_input(dual);
        if f.triviality_with_input.is_some() {
            println!(":( solves fp");
            return;
        }
        
        println!(":)");

    }
}