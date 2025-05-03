use std::collections::{HashMap, HashSet};

use env_logger::init;
use itertools::Itertools;
use serde_json::map;

use crate::{algorithms::{choices::left_labels, fixpoint::{parse_diagram, FixpointType}}, constraint::Constraint, group::{Group, GroupType, Label}, line::Line, part::Part, problem::Problem};

use super::{diagram::{compute_direct_diagram, diagram_direct_to_pred_adj, diagram_direct_to_succ_adj, diagram_indirect_to_reachability_adj, diagram_to_indirect}, event::EventHandler};


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


fn k_partitions(k_domain : &Vec<Label>, n : usize) -> impl Iterator<Item=Vec<Label>> + '_ {
    (0..n).map(|_|k_domain.iter().copied())
        .multi_cartesian_product()
}

fn dual_diagram(labels_p : &Vec<Label>, labels_d : &Vec<Vec<Label>>, labels_fp : &Vec<Label>, diagram_fp : &Vec<(Label,Label)>) -> Vec<(Label,Label)> {
    let labels_d : HashMap<_,_> = labels_d.iter().cloned().enumerate().collect();
    let labels_p_to_positions : HashMap<_,_> = labels_p.iter().copied().enumerate().map(|(i,l)|(l,i)).collect();

    let diagram_fp = diagram_to_indirect(labels_fp, diagram_fp);
    let successors_fp = diagram_indirect_to_reachability_adj(labels_fp, &diagram_fp);
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

fn dual_constraint(cp : &Constraint, cf : &Constraint, labels : &Vec<Vec<Label>>, labels_p : &Vec<Label>, all_predecessors : &HashMap<Label, HashSet<Label>>, all_successors : &HashMap<Label, HashSet<Label>>, direct_pred : &HashMap<Label, HashSet<Label>>) -> Result<Constraint, &'static str> {
    let labels_p_to_positions : HashMap<_,_> = labels_p.iter().copied().enumerate().map(|(i,l)|(l,i)).collect();
    let d = cp.finite_degree();
    let labels_d = all_successors.keys().copied().collect_vec();


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
    
    let mut sources = labels_d.iter().filter(|l|direct_pred[l].is_empty());
    let source = *sources.next().unwrap();
    if sources.next().is_some() {
        return Err("non-unique source");
    }
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
                        let old_label_f = labels[configuration[i] as usize][labels_p_to_positions[&bad[i]]];
                        let new_label_f = labels[succ as usize][labels_p_to_positions[&bad[i]]];
                        if old_label_f != new_label_f {
                        //let mut new_configuration = configuration.clone();
                        //new_configuration[i] = succ;
                        //if !is_linep_bad_for_lined(&new_configuration,&bad) {
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
    //    .filter(|line_d|find_bad_linep_for_lined(line_d).is_none())
    let lines = good_configurations.into_iter()
        .map(|line|{to_configuration(&line)}).collect_vec();
    let c = Constraint { lines, is_maximized: false, degree: crate::line::Degree::Finite(d)  };
    let c = c.edited(|g|{
        let g = g.iter().fold(HashSet::new(),|mut a,l|{
            a.extend(all_successors[l].iter().cloned());
            a
        });
        Group::from_set(&g)
    });
    Ok(c)
}

fn labels_for_dual(f_mapping_label_text : &Vec<(Label,String)>, dual_labels : &Vec<Label>, dual_labels_v : &Vec<Vec<Label>>) -> Vec<(Label, String)> {
    let mapping_fp : HashMap<_,_> = f_mapping_label_text.iter().cloned().collect();
    let join_char = if mapping_fp.values().all(|x|x.len()==1) { "" }else{"_"}; 
    dual_labels.into_iter().map(|&l|{
        let v = &dual_labels_v[l as usize];
        let s = v.iter().map(|&x|&mapping_fp[&(x as Label)]).join(join_char).replace("(","[").replace(")","]");
        (l,format!("({})",s))
    }).collect_vec()
}


impl Problem {


    pub fn dual_problem(&self, f : &Problem, eh : &mut EventHandler) -> Result<(Problem,Vec<Vec<Label>>,Vec<(Label,Label)>), &'static str> {
        println!("add active pred");
        let mut f = f.clone();
        f.add_active_predecessors();
        f.active.is_maximized = true;

        let labels_f = f.labels();
        let labels_p = self.labels();

        println!("dual labels");
        let dual_labels_v = k_partitions(&labels_f, labels_p.len()).collect_vec();

        println!("dual diagram");
        let d_diag = dual_diagram(&self.labels(), &dual_labels_v, &f.labels(), &f.diagram_indirect.as_ref().unwrap());
        let d_labels = (0..dual_labels_v.len()).map(|x|x as Label).collect_vec();
        let all_succ = diagram_indirect_to_reachability_adj(&d_labels, &d_diag);
        let inv = d_diag.iter().map(|&(a,b)|(b,a)).collect_vec();
        let all_pred = diagram_indirect_to_reachability_adj(&d_labels, &inv);

        let direct = compute_direct_diagram(&d_labels, &d_diag).1;
        let direct_succ = diagram_direct_to_succ_adj(&direct, &d_labels);
        let direct_pred = diagram_direct_to_pred_adj(&direct, &d_labels);

        println!("dual constraints");
        let dual_active = dual_constraint(&self.active, &f.active, &dual_labels_v, &labels_p, &all_succ, &all_pred, &direct_succ)?;
        let dual_passive = dual_constraint(&self.passive, &f.passive, &dual_labels_v, &labels_p, &all_pred, &all_succ, &direct_pred)?;

        println!("computed constraints, computing label names");

        let active_labels = dual_active.labels_appearing();
        let passive_labels = dual_passive.labels_appearing();
        let dual_labels = active_labels.union(&passive_labels).copied().collect_vec();

        /*let mapping_label_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();

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
        }).collect_vec();*/

        let mapping_label_text = labels_for_dual(&f.mapping_label_text,&dual_labels, &dual_labels_v);

        println!("done");

        Ok((Problem {
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
        },dual_labels_v,d_diag))
    }

    pub fn doubledual_problem(&self, fp : &Problem, eh : &mut EventHandler) -> Result<Problem, &'static str> {
        let (mapping_label_text_fp,diagram_fp) = (fp.mapping_label_text.clone(),fp.diagram_indirect.clone().unwrap());

        let (mut dual,dual_labels_v,orig_dual_diagram) = self.dual_problem(&fp, eh)?;
        dual.passive.maximize(eh);
        dual.compute_diagram(eh);
        let (equiv,_) = dual.diagram_direct.as_ref().unwrap();

        let orig_dual_labels = (0..dual_labels_v.len() as Label).collect_vec();
        let orig_dual_diagram = diagram_to_indirect(&orig_dual_labels,&orig_dual_diagram);
        let orig_dual_diagram = diagram_indirect_to_reachability_adj(&orig_dual_labels, &orig_dual_diagram);
        let equiv_choice = equiv.iter().map(|(_,v)|{
            v.iter().copied().sorted_by_key(|x|orig_dual_diagram[x].len()).last().unwrap()
        }).collect_vec();

        let labels_p = self.labels();

        let dual_labels = equiv.iter().map(|(l,_)|*l).collect_vec();
        let labels_f = mapping_label_text_fp.iter().map(|(l,_)|*l).collect_vec();

        println!("computing dual dual labels");

        let dualdual_labels_v = k_partitions(&labels_f, dual_labels.len()).collect_vec();
        let dualdual_labels = (0..dualdual_labels_v.len() as Label).collect_vec();

        println!("computing dual dual diagram");

        let dualdual_diagram = dual_diagram(&dual_labels, &dualdual_labels_v, &labels_f, &diagram_fp);

        println!("computing mapping");


        let mut mapping = vec![];

        for &l in &labels_p {
            for &dd in &dualdual_labels {
                if dualdual_labels_v[dd as usize].iter().enumerate().all(|(i,&r)|{
                    dual_labels_v[equiv_choice[i] as usize][l as usize] == r
                }) {
                    mapping.push((l,dd));
                    //break;
                }
            }
        }

        println!("computing text dualdual");


        let dualdual_text : HashMap<_,_> = labels_for_dual(&mapping_label_text_fp,&dualdual_labels,&dualdual_labels_v).into_iter().collect();
        let p_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let mut s = String::new();
        for (a,b) in mapping {
            s += &format!("{} = {}\n",p_text[&a],dualdual_text[&b]);
        }
        //println!("{}",s);
        for (a,b) in dualdual_diagram {
            s += &format!("{} -> {}\n",dualdual_text[&a],dualdual_text[&b]);
        }
        
        
        let (p,_,_) = self.fixpoint_generic(None, FixpointType::Custom(s),false, eh)?;
        Ok(p)
    }

    pub fn doubledual_diagram(&self, f_active : &str, f_passive : &str, f_diagram : &str, input_active : &str, input_passive : &str, eh : &mut EventHandler) -> Result<String, &'static str> {
        let (mapping_label_text_fp,diagram_fp) = if f_diagram.is_empty() {
            let mut fp = Problem::from_string_active_passive(f_active, f_passive)?.0;
            fp.passive.maximize(eh);
            fp.compute_diagram(eh);
            (fp.mapping_label_text,fp.diagram_indirect.clone().unwrap())
        } else {
            parse_diagram(f_diagram)
        };
        println!("computed diagram fp");
        if input_active.is_empty() {
            let labels_p = self.labels();
            let labels_f = mapping_label_text_fp.iter().map(|(l,_)|*l).collect_vec();
    
            println!("computing dual labels");

            let dual_labels_v = k_partitions(&labels_f, labels_p.len()).collect_vec();
            let dual_labels = (0..dual_labels_v.len() as Label).collect_vec();

            println!("computing dual dual labels");

            let dualdual_labels_v = k_partitions(&labels_f, dual_labels.len()).collect_vec();
            let dualdual_labels = (0..dualdual_labels_v.len() as Label).collect_vec();

            println!("computing dual dual diagram");

            let dualdual_diagram = dual_diagram(&dual_labels, &dualdual_labels_v, &labels_f, &diagram_fp);

            println!("computing mapping");


            let mut mapping = vec![];

            for &l in &labels_p {
                for &dd in &dualdual_labels {
                    if dualdual_labels_v[dd as usize].iter().enumerate().all(|(i,&r)|{
                        dual_labels_v[i][l as usize] == r
                    }) {
                        mapping.push((l,dd));
                        break;
                    }
                }
            }

            println!("computing text dualdual");


            let dualdual_text : HashMap<_,_> = labels_for_dual(&mapping_label_text_fp,&dualdual_labels,&dualdual_labels_v).into_iter().collect();
            let p_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
            let mut s = String::new();
            for (a,b) in mapping {
                s += &format!("{} = {}\n",p_text[&a],dualdual_text[&b]);
            }
            for (a,b) in dualdual_diagram {
                s += &format!("{} -> {}\n",dualdual_text[&a],dualdual_text[&b]);
            }
            println!("{}",s);
            Ok(s)
        } else {
            let input = Problem::from_string_active_passive(input_active, input_passive)?.0;
            let labels_p = input.labels();
            let labels_f = mapping_label_text_fp.iter().map(|(l,_)|*l).collect_vec();
    
            let dual_labels_v = k_partitions(&labels_f, labels_p.len()).collect_vec();
    
            let dual_diagram = dual_diagram(&labels_p, &dual_labels_v, &labels_f, &diagram_fp);

            //still need to compute the mapping

            unimplemented!()
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


let mut p = Problem::from_string("A B C\n\nA A\nB B\nC C\n").unwrap();

let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C\n").unwrap();


let mut f = Problem::from_string("x O o
r c O
r C o
c R o
X o^2
A B^2

OB OCRXB
ORB^2
OCB^2
xorcB oB
orB ocB
xXAB B").unwrap();

let mut f = Problem::from_string("A B B\n\nB AB").unwrap();


        p.passive.maximize(eh);
        p.compute_diagram(eh);

        f.passive.maximize(eh);
        f.compute_diagram(eh);



        let mut dual = p.dual_problem(&f,eh).unwrap().0;
        println!("Dual before fixing:\n{}\n",dual);

        println!("computed dual, fixing");
        fix_problem(&mut dual, true, false, eh);
        println!("merging equivalent labels");
        let mut dual = dual.merge_subdiagram("",true,eh).unwrap();
        println!("Dual:\n{}\n",dual);

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