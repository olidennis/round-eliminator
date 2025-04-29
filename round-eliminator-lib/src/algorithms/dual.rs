use std::collections::HashMap;

use itertools::Itertools;
use serde_json::map;

use crate::{constraint::Constraint, group::{Group, GroupType, Label}, line::Line, part::Part, problem::Problem};


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


fn dual_constraint(cp : &Constraint, cf : &Constraint, labels : &Vec<Vec<usize>>, labels_p : &Vec<Label>) -> Constraint {
    let labels_p_to_positions : HashMap<_,_> = labels_p.iter().copied().enumerate().map(|(i,l)|(l,i)).collect();
    let d = cp.finite_degree();

    let lines = (0..d).map(|_|0..labels.len()).multi_cartesian_product()
        .filter(|line_d|line_d.is_sorted())
        .filter(|line_d|{
            cp.all_choices(false).into_iter().all(|line_p|{
                line_d.iter().permutations(line_d.len())
                    .all(|line_d|{
                        let parts_f = line_p.iter_labels().enumerate().map(|(i,l)|{
                            Part{
                                gtype : GroupType::Many(1),
                                group : Group::from(vec![labels[*line_d[i]][labels_p_to_positions[&l]] as Label])
                            }
                        }).collect_vec();
                        let line_f = Line{ parts : parts_f };
                        cf.includes(&line_f)
                    })
            })
        })
        .map(|line|{
            let parts = line.into_iter().map(|l|{
                Part{
                    gtype: GroupType::Many(1),
                    group: Group::from(vec![l as Label]),
                }
            }).collect_vec();
            Line{ parts }
        }).collect_vec();
    Constraint { lines, is_maximized: false, degree: crate::line::Degree::Finite(d)  }
}

impl Problem {




    pub fn dual_problem(&self, f : &Problem) -> Problem {
        let mut f = f.clone();
        f.add_active_predecessors();
        f.active.is_maximized = true;

        let labels_f = f.labels();
        let labels_p = self.labels();

        let dual_labels_v = k_partitions(labels_f.len(), labels_p.len()).collect_vec();
        
        let dual_active = dual_constraint(&self.active, &f.active, &dual_labels_v, &labels_p);
        let dual_passive = dual_constraint(&self.passive, &f.passive, &dual_labels_v, &labels_p);

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
                v.iter().map(|l|&mapping_label_text[l]).join("")
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
    use crate::{algorithms::event::EventHandler, problem::Problem, serial::fix_problem};


    #[test]
    fn dual() {
        /*let eh = &mut EventHandler::with(|(s,a,b)|{
            println!("{} {} {}",s,a,b);
        });*/
        let eh = &mut EventHandler::null();

        let mut p = Problem::from_string("A B C\n\nA A\nB B\nC C\n").unwrap();
        let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C\n").unwrap();
        let mut p = Problem::from_string("A B^2
1 2 3

3B^2
1B^2
2B^2
B A123B").unwrap();

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

        let mut f = Problem::from_string("A B B\n\nB AB").unwrap();

        p.passive.maximize(eh);
        f.passive.maximize(eh);
        p.compute_diagram(eh);
        f.compute_diagram(eh);

        let mut dual = p.dual_problem(&f);
        fix_problem(&mut dual, true, false, eh);
        dual.compute_triviality(eh);
        println!("{}",dual);

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