use std::{collections::HashSet, hash::Hash};

use itertools::Itertools;

use crate::{constraint::Constraint, group::Group, problem::Problem};

use super::event::EventHandler;

impl Problem {

    pub fn harden_remove(&self, label : usize, add_predecessors: bool) -> Self {
        let mut h : HashSet<_> = self.labels().into_iter().collect();
        h.remove(&label);
        self.harden_keep(&h, add_predecessors)
    }

    pub fn harden_keep(&self, keep: &HashSet<usize>, add_predecessors: bool) -> Self {
        let mut keep = keep.clone();

        let mut newpassive = self.passive.clone();
        let mut newactive = if add_predecessors {
            let predecessors = self.diagram_indirect_to_inverse_reachability_adj();

            self.active.edited(|g| {
                let mut h = HashSet::new();
                for label in &g.0 {
                    h.extend(predecessors[label].iter().cloned());
                }
                Group::from_set(&h)
            })
        } else {
            self.active.clone()
        };

        loop {
            newactive = newactive.harden(&keep);
            newpassive = newpassive.harden(&keep);

            let appearing_active = newactive.labels_appearing();
            let appearing_passive = newpassive.labels_appearing();

            let newkeep: HashSet<usize> = appearing_active
                .intersection(&appearing_passive)
                .cloned()
                .collect();

            if newkeep == keep {
                break;
            }
            keep = newkeep;
        }

        Problem {
            active: newactive,
            passive: newpassive,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
            mapping_oldlabel_text: self.mapping_oldlabel_text.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: self.diagram_indirect_old.clone(),
        }
    }
}

impl Constraint {
    fn harden(&self, keep: &HashSet<usize>) -> Self {
        self.edited(|g| Group(g.as_set().intersection(keep).cloned().sorted().collect()))
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashSet;

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn harden_with_predecessors() {
        let mut p = Problem::from_string("0	1	1	1\n2	1	1	3\n4	4	4	5\n\n053 4513 4513 4513\n13 13 13 204513\n53 4513 4513 04513\n513 513 0513 4513\n513 513 513 04513").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        let mut p = p.harden_keep(&HashSet::from([0, 1, 2, 3]), true);
        p.discard_useless_stuff(true, &mut EventHandler::null());
        let mut p = p.merge_equivalent_labels();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        assert_eq!(format!("{}", p), "0 1^3\n\n01 1^3\n");
    }
}
