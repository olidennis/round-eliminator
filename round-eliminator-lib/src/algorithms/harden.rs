use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    constraint::Constraint,
    group::{Group, Label},
    problem::Problem,
};

impl Problem {
    pub fn harden_remove(&self, label: Label, add_predecessors: bool) -> Self {
        let mut h: HashSet<_> = self.labels().into_iter().collect();
        h.remove(&label);
        self.harden_keep(&h, add_predecessors)
    }

    pub fn harden_keep(&self, keep: &HashSet<Label>, add_predecessors: bool) -> Self {
        let mut keep = keep.clone();

        let mut newpassive = self.passive.clone();
        let mut newactive = if add_predecessors {
            let predecessors = self.diagram_indirect_to_inverse_reachability_adj();

            self.active.edited(|g| {
                let mut h = HashSet::new();
                for label in g.iter() {
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

            let newkeep: HashSet<Label> = appearing_active
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
            passive_gen : None,
            mapping_label_text: self.mapping_label_text.iter().filter(|(l,_)|keep.contains(l)).cloned().collect(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.as_ref().map(|x|x.iter().filter(|(l,_)|keep.contains(l)).cloned().collect()),
            mapping_oldlabel_labels: self.mapping_oldlabel_labels.clone(),
            mapping_oldlabel_text: self.mapping_oldlabel_text.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: self.diagram_indirect_old.clone(),
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: self.orientation_given,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None,
            is_trivial_with_input : None,
            triviality_with_input : None
        }
    }
}

impl Constraint {
    fn harden(&self, keep: &HashSet<Label>) -> Self {
        self.edited(|g| Group::from(g.as_set().intersection(keep).cloned().sorted().collect()))
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
