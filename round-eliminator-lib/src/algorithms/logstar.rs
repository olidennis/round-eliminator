use std::{clone, collections::HashMap, sync::{atomic::AtomicBool, Arc}};

use chrono::Duration;
use itertools::Itertools;
use rand::{seq::SliceRandom, Rng};

use crate::{algorithms::event::EventHandler, constraint::Constraint, group::{Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem, serial::AutoOperation};




impl Problem {
    pub fn logstar_dup(&self, labels : &Vec<Label>) -> (Self,HashMap<Label, Vec<Label>>) {
        let (mut p,map) = self.make_some_labels_different(labels, true);
        p.discard_useless_stuff(false, &mut EventHandler::null());
        (p,map)
    }

    pub fn logstar_see_one(&self, label : Label) -> Self {
        if self.passive.degree != Degree::Finite(2) {
            panic!("only passive side 2 supported");
        }
        let compatible_with_l = self.labels_compatible_with_label(label);
        if compatible_with_l.is_empty() {
            return self.clone();
        }
        let fresh_label = self.labels().into_iter().max().unwrap_or(0) + 1;
        let new_ls = (fresh_label..(fresh_label+compatible_with_l.len() as Label)).collect_vec();
        let other_to_newl : HashMap<_,_> = compatible_with_l.iter().cloned().zip(new_ls.iter().cloned()).collect();

        let active = self.active.edited(|g|{
            if !g.contains(&label) {
                g.clone()
            } else {
                let mut g = g.as_set();
                g.remove(&label);
                g.extend(new_ls.iter().cloned());
                Group::from_set(&g)
            }
        });

        let mut passive = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.passive.degree,
        };

        for line in self.passive.all_choices(false) {
            let l1 = line.parts[0].group.first();
            let l2 = line.parts[1].group.first();
            if l1 != label && l2 != label {
                passive.lines.push(line);
            } else if l1 == label && l2 == label {
                let newline = Line {
                    parts : vec![
                        Part{ group : Group::from(vec![other_to_newl[&label]]), gtype : GroupType::Many(1) },
                        Part{ group : Group::from(vec![other_to_newl[&label]]), gtype : GroupType::Many(1) }
                    ]
                };
                passive.lines.push(newline);
            } else {
                let other = if l1 != label { l1 } else { l2 };
                let newline = Line {
                    parts : vec![
                        Part{ group : Group::from(vec![other_to_newl[&other]]), gtype : GroupType::Many(1) },
                        Part{ group : Group::from(vec![other]), gtype : GroupType::Many(1) }
                    ]
                };
                passive.lines.push(newline);
            }
        } 
        
        let old_mapping : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let mut mapping_label_text : Vec<_> = self.mapping_label_text.iter().cloned().filter(|(l,_)|*l != label).collect();
        let old_name = old_mapping[&label].replace("(", "").replace(")", "");
        for other in compatible_with_l.iter() {
            let newother = other_to_newl[other];
            let other_name = old_mapping[&other].replace("(", "").replace(")", "");
            mapping_label_text.push((newother,format!("({}_{})",old_name,other_name)));
        }

        let mut p = Problem {             
            active : active,
            passive : passive,
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
        p.discard_useless_stuff(false, &mut EventHandler::null());
        p
    }

    pub fn logstar_see(&self, labels : &Vec<Label>) -> Self {
        let mut p = self.clone();
        for &l in labels {
            p = p.logstar_see_one(l);
        }
        p
    }

    pub fn logstar_mis(&self, labels : &Vec<Label>) -> Self {
        if self.passive.degree != Degree::Finite(2) {
            panic!("only passive side 2 supported");
        }

        let l = labels[0];
        let p = self.relax_merge_group(labels, l);

        let label_m = l;
        let label_p = p.labels().into_iter().max().unwrap_or(0) + 1;
        let label_u = label_p + 1;

        let mut active = Constraint{
            lines: vec![],
            is_maximized: false,
            degree: p.active.degree,
        };
        for line in &p.active.all_choices(false) {
            active.lines.push(line.clone());
            if line.groups().any(|g|g.first() == l) {
                let mut p_used = false;
                let newline = line.edited(|g|{
                    if g.first() == l {
                        if p_used {
                            Group::from(vec![label_u]) 
                        } else {
                            p_used = true;
                            Group::from(vec![label_p]) 
                        }
                    }else{
                        g.clone()
                    }
                });
                active.lines.push(newline);
            }
        }

        let mut passive = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: p.passive.degree,
        };

        for line in p.passive.all_choices(false) {
            let l1 = line.parts[0].group.first();
            let l2 = line.parts[1].group.first();
            if l1 != l && l2 != l {
                passive.lines.push(line);
            } else if l1 == l && l2 == l {
                let newline = Line {
                    parts : vec![
                        Part{ group : Group::from(vec![label_m]), gtype : GroupType::Many(1) },
                        Part{ group : Group::from(vec![label_u,label_p]), gtype : GroupType::Many(1) }
                    ]
                };
                passive.lines.push(newline);
                let newline = Line {
                    parts : vec![
                        Part{ group : Group::from(vec![label_u]), gtype : GroupType::Many(1) },
                        Part{ group : Group::from(vec![label_u]), gtype : GroupType::Many(1) }
                    ]
                };
                passive.lines.push(newline);
            } else {
                let other = if l1 != l { l1 } else { l2 };
                let newline = Line {
                    parts : vec![
                        Part{ group : Group::from(vec![label_m,label_u]), gtype : GroupType::Many(1) },
                        Part{ group : Group::from(vec![other]), gtype : GroupType::Many(1) }
                    ]
                };
                passive.lines.push(newline);
            }
        } 


        let old_mapping : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();
        let mut mapping_label_text : Vec<_> = p.mapping_label_text.iter().cloned().filter(|(label,_)|*label != l).collect();
        
        let old_name = old_mapping[&l].replace("(", "").replace(")", "");
        mapping_label_text.push((label_m,format!("({}_m)",old_name)));
        mapping_label_text.push((label_u,format!("({}_u)",old_name)));
        mapping_label_text.push((label_p,format!("({}_p)",old_name)));


        let mut p = Problem {             
            active : active,
            passive : passive,
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
        p.discard_useless_stuff(false, &mut EventHandler::null());
        p
    }

    pub fn autologstar(&mut self, max_labels : usize, mut max_depth : usize, initial_active : String, initial_passive : String, max_active : usize, max_passive : usize, onlybool : bool, eh: &mut EventHandler) -> Option<(usize, Vec<(AutoOperation,Problem)>)> {

        #[cfg(target_arch = "wasm32")]
        {
            self.autologstar_aux(max_labels,max_depth,initial_active,initial_passive,max_active,max_passive,onlybool,eh, None)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::{atomic::AtomicBool, Arc};

            use rayon::iter::{IntoParallelIterator, ParallelIterator};
            let done = Arc::new(AtomicBool::new(false));
            let threads = rayon::current_num_threads();
            let result : Vec<_> = (0..threads).into_par_iter().map(|_|{
                let done = done.clone();
                let r = self.clone().autologstar_aux(max_labels,max_depth,initial_active.clone(),initial_passive.clone(),max_active,max_passive,onlybool,&mut EventHandler::null(), Some(done));
                r
            })
                .filter_map(std::convert::identity)
                .collect::<_>();
            result.into_iter().next()
        }

    }


    pub fn autologstar_aux(&mut self, max_labels : usize, mut max_depth : usize, initial_active : String, initial_passive : String, max_active : usize, max_passive : usize, onlybool : bool, eh: &mut EventHandler, done : Option<Arc<AtomicBool>>) -> Option<(usize, Vec<(AutoOperation,Problem)>)> {
        let d = self.active.finite_degree();
        let initial = if initial_active == "" {
            Problem::from_string(format!("M^{}\nP U^{}\n\nM UP\nU U",d,d-1)).unwrap()
        } else {
            Problem::from_string_active_passive(initial_active,initial_passive).unwrap().0
        };
        
        if onlybool {
            max_depth = 1_000_000;
        }
        let mut rng = rand::thread_rng();

        #[cfg(not(target_arch = "wasm32"))]
        let mut last_print = std::time::Instant::now();
        let mut num_steps = 0;

        'outer: loop{
            let mut v = vec![];
            v.push((AutoOperation::Initial,initial.clone()));
            let mut p = initial.clone();

            for _ in 0..max_depth {
                if let Some(done) = done.as_ref() {
                    if done.load(std::sync::atomic::Ordering::Relaxed) {
                        return None;
                    }
                }
                num_steps += 1;
                #[cfg(not(target_arch = "wasm32"))]
                if last_print.elapsed() > std::time::Duration::from_secs(1) {
                    println!("{} steps/sec",num_steps);
                    last_print = std::time::Instant::now();
                    num_steps = 0;
                }
                if p.active.lines.len() > max_active || p.passive.lines.len() > max_passive {
                    continue 'outer;
                }
                if onlybool {
                    p.mapping_label_text = p.labels().into_iter().enumerate().map(|(i,l)|(l,format!("({})",i))).collect();
                }
                p.discard_useless_stuff(false, &mut EventHandler::null());
                p.compute_triviality(&mut EventHandler::null());
                if !p.trivial_sets.as_ref().unwrap().is_empty() {
                    continue 'outer;
                }
                //if p.active.labels_appearing() != p.passive.labels_appearing() {
                //    panic!("wtf");
                //}
                //println!("{}",p);
                self.compute_triviality_with_input_with_sat(p.clone());
                if self.is_trivial_with_input.unwrap() {
                    if let Some(done) = done.as_ref() {
                        done.store(true,std::sync::atomic::Ordering::Relaxed);
                    }
                    return Some((v.len(),v))
                }

                let r = if p.labels().len() > max_labels {
                    3
                } else {
                    rng.gen_range(0..4)
                };

                match r {
                    0 => {
                        let labels = p.labels();
                        let random_label = *labels.choose(&mut rng).unwrap();
                        let set_label = vec![random_label];
                        let new_p = p.logstar_mis(&set_label);
                        if new_p.labels().len() > max_labels {
                            continue;
                        }
                        if !onlybool{
                            v.push((AutoOperation::LogstarMIS(set_label,p.clone()),new_p.clone()));
                        }
                        p = new_p;
                    }
                    1 => {
                        let labels = p.labels();
                        let random_label = *labels.choose(&mut rng).unwrap();
                        let set_label = vec![random_label];
                        let (new_p,map) = p.logstar_dup(&set_label);
                        let (_,dups) = map.iter().next().unwrap();
                        if dups.len() == 1 {
                            continue;
                        }
                        let pivot = rng.gen_range(1..dups.len());
                        let g1 = dups[0..pivot].into_iter().cloned().collect_vec();
                        let g2 = &dups[pivot..].into_iter().cloned().collect_vec();
                        let new_p = new_p.relax_merge_group(&g1,g1[0]);
                        let new_p = new_p.relax_merge_group(&g2,g2[0]);
                        if new_p.labels().len() > max_labels {
                            continue;
                        }
                        if !onlybool{
                            v.push((AutoOperation::LogstarDup(set_label,p.clone()),new_p.clone()));
                        }
                        p = new_p;
                    }
                    2 => {
                        let labels = p.labels();
                        let random_label = *labels.choose(&mut rng).unwrap();
                        let set_label = vec![random_label];
                        let new_p = p.logstar_see(&set_label);
                        if new_p.labels().len() > max_labels {
                            continue;
                        }
                        if !onlybool{
                            v.push((AutoOperation::LogstarSee(set_label,p.clone()),new_p.clone()));
                        }
                        p = new_p;
                    }
                    3 => {
                        let labels = p.labels();
                        let random_label_1 = *labels.choose(&mut rng).unwrap();
                        let random_label_2 = *labels.choose(&mut rng).unwrap();
                        let new_p = p.relax_merge(random_label_1,random_label_2);
                        if new_p.labels().len() > max_labels {
                            continue;
                        }
                        if !onlybool{
                            v.push((AutoOperation::Merge(vec![(random_label_1,random_label_2)],p.clone()),new_p.clone()));
                        }
                        p = new_p;
                    }
                    _ => { unreachable!() }
                }

            }
        }
    }


}