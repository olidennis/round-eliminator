use std::collections::{HashMap, HashSet};

use bincode::config;
use itertools::{iproduct, Itertools};
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{algorithms::parallel::CollectWithProgress, constraint::Constraint, group::{Exponent, Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem, serial::fix_problem};

use super::event::EventHandler;




impl Problem {
    
    pub fn compute_demisifiable<F>(&mut self, mut f : F, old : bool, eh : &mut EventHandler) where F : FnMut(Vec<Label>) {
        if old {
            self.compute_demisifiable_old(eh);
            return;
        }
        
        let labels = self.labels();
        let mut result = vec![];

        if self.is_set_reversible_merge(&labels) {
            f(labels.clone());
        } else {
            println!("full set does not work");
        }

        for subset in labels.into_iter().powerset() {
            if subset.len() >= 2 {
                if self.is_set_reversible_merge(&subset) {
                    f(subset.clone());
                    result.push((subset,vec![]));
                }
            }
        }

        self.demisifiable = Some(result);
    }

    pub fn is_set_reversible_merge(&mut self, labels : &Vec<Label>) -> bool {
        if self.passive.degree != Degree::Finite(2) {
            panic!("this feature is only implemented for passive degree 2");
        }
        let m = labels[0];
        let merged = self.relax_merge_group(labels, m);
        

        let mut fresh_label = merged.labels().into_iter().max().unwrap() +1;
        let d = merged.active.finite_degree();
        let mut color_labels = HashMap::new();

        for i in 1..=d+1 {
            for j in 0..=d+1 {
                if i != j {
                    color_labels.insert((i,j), fresh_label);
                    fresh_label += 1;
                }
            }
        }

        let mut new_active = Constraint{
            lines: vec![],
            is_maximized: false,
            degree: merged.active.degree,
        };
        for line in &merged.active.all_choices(false) {
            let ms = line.groups().filter(|g|g.first() == m).count();
            if ms == 0 {
                new_active.lines.push(line.clone());
            } else {
                for i in 1..=ms+1 {
                    let mut j = 1;
                    let newline = line.edited(|g|{
                        let l = g.first();
                        if l == m {
                            let g = if j < i {
                                Group::from(vec![color_labels[&(i,j)]])
                            } else {
                                Group::from((0..=d+1).filter(|&j|j!=i).map(|j|color_labels[&(i,j)]).sorted().collect())
                            };
                            j += 1;
                            g
                        } else {
                            g.clone()
                        }
                    });
                    new_active.lines.push(newline);
                }
            }
        }


        let mut new_passive = Constraint{
            lines: vec![],
            is_maximized: false,
            degree: merged.passive.degree,
        };
        for line in merged.passive.all_choices(false) {
            let l1 = line.parts[0].group.first();
            let l2 = line.parts[1].group.first();
            if l1 != m && l2 != m {
                new_passive.lines.push(line);
            } else if l1 == m && l2 == m {
                for c1 in 1..=d+1 {
                    for c2 in 1..=d+1 {
                        if c1 != c2 {
                            let newline = Line{
                                parts : vec![
                                    Part{ group : Group::from(vec![color_labels[&(c1,c2)]]), gtype : GroupType::Many(1) },
                                    Part{ group : Group::from(vec![color_labels[&(c2,c1)]]), gtype : GroupType::Many(1) }
                                ]
                            };
                            new_passive.lines.push(newline);
                        }
                    }
                }
            } else {
                let l = if l1 != m { l1 } else { l2 };
                for c in 1..=d+1 {
                    let newline = Line{
                        parts : vec![
                            Part{ group : Group::from(vec![color_labels[&(c,0)]]), gtype : GroupType::Many(1) },
                            Part{ group : Group::from(vec![l]), gtype : GroupType::Many(1) }
                        ]
                    };
                    new_passive.lines.push(newline);
                }
            }
        }

        let mut mapping_label_text = merged.mapping_label_text.clone();
        for i in 1..=d+1 {
            for j in 0..=d+1 {
                if i != j {
                    mapping_label_text.push((color_labels[&(i,j)],format!("(c_{}_{})",i,j)));
                }
            }
        }

        let mut input = Problem {             
            active : new_active,
            passive : new_passive,
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
        };

        input.discard_useless_stuff(false, &mut EventHandler::null());
        self.compute_triviality_with_input_with_sat(input);
        let r = self.is_trivial_with_input.unwrap();
        self.is_trivial_with_input = None;
        self.triviality_with_input = None;
        r
    }


    pub fn labels_compatible_with_label(&self, label : Label) -> HashSet<Label> {
        if self.passive.degree != Degree::Finite(2) {
            panic!("can only be computed when the passive degree is 2");
        }

        self.passive.lines.iter().flat_map(|line|{
            let cur_a = &line.parts[0].group;
            let cur_b = if line.parts.len() == 1 {
                &line.parts[0].group
            } else {
                &line.parts[1].group
            };
            std::iter::once((cur_a, cur_b)).chain(std::iter::once((cur_b, cur_a)))
        })
        .filter_map(|(g1,g2)|{
            if g1.contains(&label) {
                Some(g2.as_set())
            } else {
                None
            }
        })
        .fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1})
    }


    pub fn generic_demisifiable(&mut self, eh : &mut EventHandler, active : &Constraint, passive : &Constraint, external : &HashSet<Label>, exclude : &HashSet<(Label,Label)>) -> Vec<(Vec<Label>, Vec<Label>)> {
        if self.demisifiable.is_some() {
            panic!("demisifiable sets have been already computed");
        }
        if self.passive.degree != Degree::Finite(2) {
            panic!("can only be computed when the passive degree is 2");
        }
        if self.active.degree == Degree::Star {
            panic!("infinite active degree is not implemented");
        }
        
        let labels = self.labels();
        let other_labels = active.labels_appearing();

        //println!("{:?}",self.mapping_label_text);
        
        let degree = self.active.finite_degree();
        //println!("degree is {}",degree);

        let total = labels.len()*labels.len()*labels.len();

        let step1 = |subset : &Vec<Label>| -> bool {
            if subset.len() != subset.iter().cloned().unique().count() {
                return false;
            }

            let mut subproblem = self.harden_keep(&HashSet::from_iter(subset.iter().cloned()), false);
            subproblem.passive.maximize(&mut EventHandler::null());
            subproblem.compute_diagram(&mut EventHandler::null()); 

            for line in passive.all_choices(true) {
                let line = line.edited(|g|{
                    let new_label = subset[g.first() as usize];
                    Group::from(vec![new_label])
                });
                if !subproblem.passive.includes(&line) {
                    return false;
                }
            }

            let predecessors_subproblem = subproblem.diagram_indirect_to_inverse_reachability_adj();
            let mut subproblem_active_with_predecessors = subproblem.active.edited(|g|{
                let h = g.iter().map(|label|&predecessors_subproblem[label]).fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1});
                Group::from_set(&h)
            });
            subproblem_active_with_predecessors.is_maximized = true;


            for line in &active.lines {
                let mut line = line.edited(|g|{
                    let new_label = subset[g.first() as usize];
                    Group::from(vec![new_label])
                });
                let min_degree = line.degree_without_star();
                let max_degree = if let Degree::Finite(d) = line.degree() {
                    d
                } else {
                    degree
                };
                if degree < min_degree {
                    continue;
                }
                if degree != max_degree {
                    return false;
                }
                let replace_star_with = degree - min_degree;
                for part in line.parts.iter_mut() {
                    if part.gtype == GroupType::Star {
                        part.gtype = GroupType::Many(replace_star_with as u8);
                    }
                }
                //println!("checking line {:#?}",line);
                //println!("in {:#?}",subproblem_active_with_predecessors);

                if !subproblem_active_with_predecessors.includes(&line) {
                    return false;
                }
            }

            true
        };
        
        #[cfg(not(target_arch = "wasm32"))]
        let result1 : Vec<_> = (0..other_labels.len()).map(|_|labels.iter().cloned()).multi_cartesian_product()
            .par_bridge()
            .map(|x|
                if step1(&x) {
                    Some(x)
                } else {
                    None
                }
            )
            .collect_with_progress(eh, "demisifiable",total)
            .into_iter()
            .map(|v|(v,vec![]))
            .collect();

        #[cfg(target_arch = "wasm32")]
        let result1 : Vec<_> = (0..other_labels.len()).map(|_|labels.iter().cloned()).multi_cartesian_product()
            .enumerate()
            .filter_map(|(i,x)|{
                eh.notify("demisifiable",i,total);
                if step1(&x) {
                    Some(x)
                } else {
                    None
                }
            })
            .map(|v|(v,vec![]))
            .collect();

        if !result1.is_empty() {
            return result1;
        }

        let step2 = |subset : &Vec<Label>| -> Option<(Vec<Label>,Vec<Label>)> {
            if subset.len() != subset.iter().cloned().unique().count() {
                return None;
            }

            let mut compatible_with = subset.iter().map(|&x|(x,self.labels_compatible_with_label(x))).collect::<HashMap<_,_>>();


            for line in passive.all_choices(true) {
                let line = line.edited(|g|{
                    let new_label = subset[g.first() as usize];
                    Group::from(vec![new_label])
                });
                let l1 = line.parts[0].group.first();
                let l2 = if line.parts[0].gtype.value() > 1 {
                    l1
                } else {
                    line.parts[1].group.first()
                };
                if !compatible_with[&l1].contains(&l2) {
                    return None;
                }
            }


            for (_,comp) in compatible_with.iter_mut() {
                for l2 in subset {
                    comp.remove(l2);
                }
            }

            let mut toremove = HashSet::new();
            for &l1 in subset {
                for &l2 in external {
                    let l2 = subset[l2 as usize];
                    if l1 == l2 {
                        continue;
                    }
                    if exclude.contains(&(l1,l2)) {
                        continue;
                    }
                    let r = compatible_with[&l1].difference(&compatible_with[&l2]);
                    toremove.extend(r);
                }
            }

            let tokeep : HashSet<_> = HashSet::from_iter(labels.iter().cloned()).difference(&toremove).cloned().collect();

            let mut after_remove = self.harden_keep(&tokeep, false);
            after_remove.discard_useless_stuff(true, &mut EventHandler::null());
            let after_remove = after_remove.merge_subdiagram(&String::new(), true, &mut EventHandler::null()).unwrap();

            let new_labels = HashSet::<Label>::from_iter(after_remove.labels().into_iter());
            if subset.iter().any(|l|!new_labels.contains(l)){
                return None;
            }

            let predecessors = after_remove.diagram_indirect_to_inverse_reachability_adj();
            let mut active_with_predecessors = after_remove.active.edited(|g|{
                let h = g.iter().map(|label|&predecessors[label]).fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1});
                Group::from_set(&h)
            });
            active_with_predecessors.is_maximized = true;

            let subset_as_set = HashSet::from_iter(subset.iter().cloned());
        
            for configuration in after_remove.active.all_choices(false) {
                if configuration.groups().any(|g|subset.iter().any(|l|g.contains(l))) {


                    let subline = Line{
                        parts : configuration.parts.iter().filter(|p|{
                            p.group.as_set().intersection(&subset_as_set).next().is_none()
                        }).cloned().collect()
                    };
                    let subline_degree = subline.degree_without_star();

                    for line in &active.lines {
                        let line = line.edited(|g|{
                            let new_label = subset[g.first() as usize];
                            Group::from(vec![new_label])
                        });

                        if let Degree::Finite(d) = line.degree() {
                            let mut newline = subline.clone();
                            let mut missing = degree - subline_degree;
                            if missing > d {
                                return None;
                            }
                            for part in line.parts {
                                if missing > 0 {
                                    let exponent = std::cmp::min(missing,part.gtype.value());
                                    let mut newpart = part.clone();
                                    newpart.gtype = GroupType::Many(exponent as u8);
                                    newline.parts.push(newpart);
                                    missing -= exponent;
                                }
                            }
                            newline.normalize();
                            if !active_with_predecessors.includes(&newline) {
                                return None;
                            }
                        } else {
                            let line_degree_without_star = line.degree_without_star();
                            if line_degree_without_star > degree - subline_degree {
                                continue;
                            }
                            
                            let mut parts = subline.parts.clone();
                            parts.extend(line.parts.into_iter());
                            let mut newline = Line{
                                parts
                            };
                            let replace_star_with = degree - subline_degree - line_degree_without_star;
                            for part in newline.parts.iter_mut() {
                                if part.gtype == GroupType::Star {
                                    part.gtype = GroupType::Many(replace_star_with as u8);
                                }
                            }
                            newline.normalize();
                            if !active_with_predecessors.includes(&newline) {
                                return None;
                            }
                        }
                    }

                }
            }

            Some((subset.clone(),toremove.into_iter().sorted().collect()))
        };

        #[cfg(not(target_arch = "wasm32"))]
        let result2 : Vec<_> = (0..other_labels.len()).map(|_|labels.iter().cloned()).multi_cartesian_product()
            .par_bridge()
            .map(|x|
                step2(&x)
            )
            .collect_with_progress(eh, "demisifiable",total);
        
        #[cfg(target_arch = "wasm32")]
        let result2 : Vec<_> = (0..other_labels.len()).map(|_|labels.iter().cloned()).multi_cartesian_product()
            .enumerate()
            .filter_map(|(i,x)|{
                eh.notify("demisifiable",i,total);
                step2(&x)
            })
            .collect();

        return result2;    
    }

    pub fn compute_demisifiable_old(&mut self, eh : &mut EventHandler) {
        let mut r = vec![];

        let p = Problem::from_string("M*\nP U*\n\nM UP\nU U").unwrap();
        let labels : HashMap<_,_> = p.mapping_label_text.iter().cloned().map(|(l,s)|(s,l)).collect();
        let external = HashSet::from([labels["M"],labels["U"]]);
        let exclude = HashSet::from([]);
        r.extend(self.generic_demisifiable(eh, &p.active, &p.passive, &external, &exclude).into_iter());
 
        let p = Problem::from_string("M U*\nP*\n\nM M\nUP U").unwrap();
        let labels : HashMap<_,_> = p.mapping_label_text.iter().cloned().map(|(l,s)|(s,l)).collect();
        let external = HashSet::from([labels["U"],labels["P"]]);
        let exclude = HashSet::new();
        r.extend(self.generic_demisifiable(eh, &p.active, &p.passive, &external, &exclude).into_iter());

        let p = Problem::from_string("A\nB\n\nA B").unwrap();
        let external = HashSet::new();
        let exclude = HashSet::new();
        r.extend(self.generic_demisifiable(eh, &p.active, &p.passive, &external, &exclude).into_iter());

        let p = Problem::from_string("A A\nB B\nC C\n\nA BC\nB C").unwrap();
        let labels : HashMap<_,_> = p.mapping_label_text.iter().cloned().map(|(l,s)|(s,l)).collect();
        let external = HashSet::from([labels["A"],labels["B"],labels["C"]]);
        let exclude = HashSet::new();
        r.extend(self.generic_demisifiable(eh, &p.active, &p.passive, &external, &exclude).into_iter());

        let r = r.into_iter().map(|(mut v, mut u)|{v.sort(); u.sort(); (v,u)}).unique().collect();

        self.demisifiable = Some(r);
    }

    /* 
    pub fn compute_demisifiable(&mut self, eh : &mut EventHandler) {
        if self.demisifiable.is_some() {
            panic!("demisifiable sets have been already computed");
        }
        if self.passive.degree != Degree::Finite(2) {
            panic!("can only be computed when the passive degree is 2");
        }
        if self.active.degree == Degree::Star {
            panic!("infinite active degree is not implemented");
        }

        let labels = self.labels();

        //println!("{:?}",self.mapping_label_text);
        
        let degree = self.active.finite_degree();
        //println!("degree is {}",degree);

        let total = labels.len()*labels.len()*labels.len();

        let step1 = |(m,p,u) : (Label,Label,Label)| -> bool {
            if m == p || m == u || p == u {
                return false;
            }
            //println!("M={}, P={}, U={}",m,p,u);

            let compatible_with_u : HashSet<Label> = self.labels_compatible_with_label(u);
            let compatible_with_m : HashSet<Label> = self.labels_compatible_with_label(m);

            //println!("compatible with u: {:?}",compatible_with_u);
            //println!("compatible with m: {:?}",compatible_with_m);

            if !compatible_with_m.contains(&u) || !compatible_with_m.contains(&p) ||  !compatible_with_u.contains(&u){
                return false;
            }


            // this is a special case, the label merging would not be reversible, but since it gives 0 round solvability we consider it anyways
            let mut subproblem = self.harden_keep(&HashSet::from_iter([m,p,u].into_iter()), true);
            subproblem.passive.maximize(&mut EventHandler::null());
            subproblem.compute_diagram(&mut EventHandler::null()); 
            let predecessors_subproblem = subproblem.diagram_indirect_to_inverse_reachability_adj();
            let mut subproblem_active_with_predecessors = subproblem.active.edited(|g|{
                let h = g.iter().map(|label|&predecessors_subproblem[label]).fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1});
                Group::from_set(&h)
            });
            subproblem_active_with_predecessors.is_maximized = true;

            let l1 = Line{ parts : vec![Part{ gtype : GroupType::Many(degree as Exponent), group : Group::from(vec![m]) }] };
            let l2 = Line{ parts : vec![
                Part{gtype : GroupType::Many(1), group : Group::from(vec![p]) },
                Part{gtype : GroupType::Many((degree -1) as Exponent), group : Group::from(vec![u]) }
            ]};

            //let mapping : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();

            if subproblem_active_with_predecessors.includes(&l1) && subproblem_active_with_predecessors.includes(&l2) {
                return true;
            }
            false
        };
        
        #[cfg(not(target_arch = "wasm32"))]
        let result1 : Vec<_> = iproduct!(labels.iter().cloned(),labels.iter().cloned(),labels.iter().cloned())
            .par_bridge()
            .map(|x|
                if step1(x) {
                    Some(x)
                } else {
                    None
                }
            )
            .collect_with_progress(eh, "demisifiable",total)
            .into_iter()
            .map(|(m,p,u)|(vec![m,p,u],vec![]))
            .collect();

        #[cfg(target_arch = "wasm32")]
        let result1 : Vec<_> = iproduct!(labels.iter().cloned(),labels.iter().cloned(),labels.iter().cloned())
            .enumerate()
            .filter_map(|(i,x)|{
                eh.notify("demisifiable",i,total);
                if step1(x) {
                    Some(x)
                } else {
                    None
                }
            })
            .map(|(m,p,u)|(vec![m,p,u],vec![]))
            .collect();

        if !result1.is_empty() {
            self.demisifiable = Some(result1);
            return;
        }

        let step2 = |(m,p,u) : (Label,Label,Label)| -> Option<(Vec<Label>,Vec<Label>)> {
            if m == p || m == u || p == u {
                return None;
            }
            //println!("M={}, P={}, U={}",m,p,u);

            let mut compatible_with_u : HashSet<Label> = self.labels_compatible_with_label(u);
            let mut compatible_with_m : HashSet<Label> = self.labels_compatible_with_label(m);

            //println!("compatible with u: {:?}",compatible_with_u);
            //println!("compatible with m: {:?}",compatible_with_m);

            if !compatible_with_m.contains(&u) || !compatible_with_m.contains(&p) ||  !compatible_with_u.contains(&u){
                return None;
            }

            let mut compatible_with_p : HashSet<Label> = self.labels_compatible_with_label(p);
            //println!("compatible with p: {:?}",compatible_with_p);
            compatible_with_u.remove(&m);
            compatible_with_u.remove(&p);
            compatible_with_u.remove(&u);
            compatible_with_m.remove(&m);
            compatible_with_m.remove(&p);
            compatible_with_m.remove(&u);
            compatible_with_p.remove(&m);
            compatible_with_p.remove(&p);
            compatible_with_p.remove(&u);


            let d1 : HashSet<_> = compatible_with_m.difference(&compatible_with_u).cloned().collect();
            let d2 : HashSet<_> = compatible_with_u.difference(&compatible_with_m).cloned().collect();
            let d3 : HashSet<_> = compatible_with_p.difference(&compatible_with_m).cloned().collect();
            //let d4 : HashSet<_> = compatible_with_p.difference(&compatible_with_u).cloned().collect();

            let mut toremove = d1;
            toremove.extend(d2.into_iter());
            toremove.extend(d3.into_iter());
            //toremove.extend(d4.into_iter());
            let tokeep : HashSet<_> = HashSet::from_iter(labels.iter().cloned()).difference(&toremove).cloned().collect();

            let mut after_remove = self.harden_keep(&tokeep, true);
            after_remove.discard_useless_stuff(true, &mut EventHandler::null());
            let after_remove = after_remove.merge_subdiagram(&String::new(), true, &mut EventHandler::null()).unwrap();

            let new_labels = HashSet::<Label>::from_iter(after_remove.labels().into_iter());
            if !new_labels.contains(&m) || !new_labels.contains(&p) || !new_labels.contains(&u) {
                return None;
            }

            let predecessors = after_remove.diagram_indirect_to_inverse_reachability_adj();
            let mut active_with_predecessors = after_remove.active.edited(|g|{
                let h = g.iter().map(|label|&predecessors[label]).fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1});
                Group::from_set(&h)
            });
            active_with_predecessors.is_maximized = true;

            //if compatible_with_m != compatible_with_u || !compatible_with_p.is_subset(&compatible_with_m) {
            //    continue;
            //}

            for configuration in after_remove.active.all_choices(false) {
                if configuration.groups().any(|g|g.contains(&m) || g.contains(&u) || g.contains(&p)) {
                    let c1 = configuration.edited(|g|{
                        if g.contains(&m) || g.contains(&u) || g.contains(&p) {
                            Group::from(vec![m])
                        } else {
                            g.clone()
                        }
                    });
                    if !active_with_predecessors.includes(&c1) {
                        return None;
                    }

                    let mut c2 = Line{ parts : vec![]};
                    let mut p_used = false;
                    for part in &configuration.parts {
                        if !part.group.contains(&m) && !part.group.contains(&p) && !part.group.contains(&u) {
                            c2.parts.push(part.clone());
                        } else {
                            if !p_used {
                                let exponent = part.gtype.value();
                                assert!(exponent > 0);
                                c2.parts.push(Part { gtype: GroupType::Many(1), group: Group::from(vec![p]) });
                                if exponent > 1 {
                                    c2.parts.push(Part { gtype: GroupType::Many((exponent - 1) as Exponent), group: Group::from(vec![u]) });
                                }
                                p_used = true;
                            } else {
                                c2.parts.push(part.edited(|_|Group::from(vec![u])));
                            }
                        }
                    }

                    if !active_with_predecessors.includes(&c2) {
                        return None;
                    }
                    
                }
            }

            Some((vec![m,p,u],toremove.into_iter().sorted().collect()))
        };

        #[cfg(not(target_arch = "wasm32"))]
        let result2 : Vec<_> = iproduct!(labels.iter().cloned(),labels.iter().cloned(),labels.iter().cloned())
            .par_bridge()
            .map(|x|
                step2(x)
            )
            .collect_with_progress(eh, "demisifiable",total);
        
        #[cfg(target_arch = "wasm32")]
        let result2 : Vec<_> = iproduct!(labels.iter().cloned(),labels.iter().cloned(),labels.iter().cloned())
            .enumerate()
            .filter_map(|(i,x)|{
                eh.notify("demisifiable",i,total);
                step2(x)
            })
            .collect();

        //println!("sets: {:?}",sets);
        self.demisifiable = Some(result2);
    }*/
}