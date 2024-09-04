use std::collections::HashSet;

use crate::{group::{Exponent, Group, GroupType, Label}, line::{Degree, Line}, part::Part, problem::Problem};

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

        let mut sets = vec![];
        let labels = self.labels();

        //println!("{:?}",self.mapping_label_text);
        let predecessors = self.diagram_indirect_to_inverse_reachability_adj();
        let mut active_with_predecessors = self.active.edited(|g|{
            let h = g.iter().map(|label|&predecessors[label]).fold(HashSet::new(), |mut h1,h2|{h1.extend(h2.into_iter()); h1});
            Group::from_set(&h)
        });
        active_with_predecessors.is_maximized = true;
        let degree = self.active.finite_degree();

        let total = labels.len()*labels.len()*labels.len();
        let mut progress = 0;

        for &m in &labels {
            for &p in &labels {
                'outer: for &u in &labels {
                    progress += 1;
                    eh.notify("demisifiable", progress, total);

                    if m == p || m == u || p == u {
                        continue;
                    }
                    //println!("M={}, P={}, U={}",m,p,u);

                    let mut compatible_with_u : HashSet<Label> = self.labels_compatible_with_label(u);
                    let mut compatible_with_m : HashSet<Label> = self.labels_compatible_with_label(m);

                    //println!("compatible with u: {:?}",compatible_with_u);
                    //println!("compatible with m: {:?}",compatible_with_m);

                    if !compatible_with_m.contains(&u) || !compatible_with_m.contains(&p) ||  !compatible_with_u.contains(&u){
                        continue;
                    }


                    let l1 = Line{ parts : vec![Part{ gtype : GroupType::Many(degree as Exponent), group : Group::from(vec![m]) }] };
                    let l2 = Line{ parts : vec![
                        Part{gtype : GroupType::Many(1), group : Group::from(vec![p]) },
                        Part{gtype : GroupType::Many((degree -1) as Exponent), group : Group::from(vec![u]) }
                    ]};

                    if active_with_predecessors.includes(&l1) && active_with_predecessors.includes(&l2) {
                        sets.push(vec![m,p,u]);
                        continue;
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

                    if compatible_with_m != compatible_with_u || !compatible_with_p.is_subset(&compatible_with_m) {
                        continue;
                    }

                    for configuration in self.active.all_choices(false) {
                        if configuration.groups().any(|g|g.contains(&m) || g.contains(&u) || g.contains(&p)) {
                            let c1 = configuration.edited(|g|{
                                if g.contains(&m) || g.contains(&u) || g.contains(&p) {
                                    Group::from(vec![m])
                                } else {
                                    g.clone()
                                }
                            });
                            if !active_with_predecessors.includes(&c1) {
                                continue 'outer;
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
                                continue 'outer;
                            }
                        }
                    }

                    sets.push(vec![m,p,u]);

                }
            }
        }
        
        //println!("sets: {:?}",sets);
        self.demisifiable = Some(sets);
    }
}