use std::collections::{HashMap, HashSet};

use crate::{constraint::Constraint, group::Group, line::Line, problem::Problem};

use super::event::EventHandler;





impl Problem {

    fn is_trivial_given_input(&mut self, colors : Option<usize>, eh : &mut EventHandler) -> bool {
        if self.trivial_sets.is_none() {
            self.compute_triviality(eh);
        }
        if self.coloring_sets.is_none() && colors.is_some() {
            self.compute_coloring_solvability(eh);
        }
        if self.trivial_sets.as_ref().unwrap().len() > 0 {
            return true;
        }
        if let Some(colors) = colors {
            if self.coloring_sets.as_ref().unwrap_or(&vec![]).len() >= colors {
                return true;
            }
        }
        false
    }

    fn critical_sets(&mut self, zero : usize, colors : Option<usize>, colors_passive : Option<usize>, eh : &mut EventHandler) -> (HashSet<Group>,HashSet<Group>,Constraint) {
        let successors = self.diagram_indirect_to_reachability_adj();
        self.passive.maximize(eh);
        let mut new = self.passive.clone();

        let all_labels = Group::from(self.labels());

        let gmap: HashMap<Group, Group> = new.groups()
            .map(|g| {
                let mut candidates: Vec<_> = successors
                    .iter()
                    .filter(|(_, s)| s.is_superset(&g.as_set()))
                    .collect();
                candidates.sort_by_key(|(_, s)| s.len());
                if candidates.is_empty() {
                    (g.clone(), all_labels.clone())
                } else {
                    let sz = candidates[0].1.len();
                    let small: Vec<_> = candidates
                        .into_iter()
                        .filter(|(_, x)| x.len() == sz)
                        .collect();
                    if small.len() > 1 {
                        // this could be a problem
                    }
                    (g.clone(), Group::from_set(&small[0].1))
                }
            })
            .collect();


        let original_labels: HashSet<_> = gmap
            .iter()
            .filter(|(a, b)| a == b)
            .map(|(a, _)| a.clone())
            .collect();
        let mut critical_sets = HashSet::new();

        let mut vgmap: Vec<_> = gmap.into_iter().collect();
        vgmap.sort_by_key(|(a, _)| a.len());
        vgmap.reverse();

        let mut i = 0;
        while i != vgmap.len() {
            let mut size = vgmap.len() - i;
            while size >= 1 {
                let mut temp = new.clone();
                for j in i..i + size {
                    let (old, new) = &vgmap[j];
                    temp = temp.edited(|g| if g != old { g.clone() } else { new.clone() });
                }
                let newp = self.replace_passive(temp);
                let mut zerocheck = newp.clone();
                
                let mut colors = colors.clone();
                let mut colors_passive = colors_passive.clone();

                if !zerocheck.is_trivial_given_input(colors, eh) {
                    for _ in 0..zero {
                        zerocheck = zerocheck.speedup(eh);
                        std::mem::swap(&mut colors, &mut colors_passive);
                        if zerocheck.is_trivial_given_input(colors, eh) {
                            break;
                        }
                    }
                }
                                
                if !zerocheck.is_trivial_given_input(colors, eh) {
                    new = newp.passive;
                    i += size;
                    break;
                } else {
                    size /= 2;
                }
            }
            if size == 0 {
                critical_sets.insert(vgmap[i].0.clone());
                i += 1;
            }
        }

        (original_labels,critical_sets,new)
    }


    pub fn critical_relax(&self, zero : usize, colors : Option<usize>, colors_passive : Option<usize>, eh : &mut EventHandler) -> Self {
        let mut p = self.clone();
        let (_,_,new_passive) = p.critical_sets(zero, colors, colors_passive, eh);
        p.replace_passive(new_passive)
    }

    pub fn critical_harden(&self, zero : usize, colors : Option<usize>, colors_passive : Option<usize>, add_predecessors : bool, eh : &mut EventHandler) -> Self {
        let mut p = self.clone();
        let (orig,crit,_) = p.critical_sets(zero, colors, colors_passive, eh);
        let mut newp = p.speedup(eh);
        let mut tokeep = HashSet::new();

        for (label,oldset) in newp.mapping_label_oldlabels.as_ref().unwrap() {
            let oldset = Group::from(oldset.clone());
            if orig.contains(&oldset) || crit.contains(&oldset) {
                tokeep.insert(*label);
            }
        }
        newp.compute_partial_diagram(eh);
        newp.harden_keep(&tokeep, add_predecessors)
    }
}