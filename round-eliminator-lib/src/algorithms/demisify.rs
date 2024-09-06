use std::collections::HashSet;

use itertools::{iproduct, Itertools};
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{algorithms::parallel::CollectWithProgress, group::{Exponent, Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem, serial::fix_problem};

use super::event::EventHandler;




impl Problem {


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
            let mut toremove = d1;
            toremove.extend(d2.into_iter());
            toremove.extend(d3.into_iter());
            let tokeep : HashSet<_> = HashSet::from_iter(labels.iter().cloned()).difference(&toremove).cloned().collect();

            let mut after_remove = self.harden_keep(&tokeep, true);
            after_remove.discard_useless_stuff(true, &mut EventHandler::null());
            let after_remove = after_remove.merge_subdiagram(&String::new(), &mut EventHandler::null()).unwrap();

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
    }
}