use crate::{group::Group, problem::Problem};

use super::event::EventHandler;

impl Problem {
    pub fn discard_labels_used_on_at_most_one_side_from_configurations(&mut self) {
        let labels_active = self.active.labels_appearing();
        let labels_passive = self.passive.labels_appearing();
        let to_keep = labels_active
            .intersection(&labels_passive)
            .cloned()
            .collect();
        let newp = self.harden_keep(&to_keep, false);
        self.active = newp.active;
        self.passive = newp.passive;
    }

    pub fn discard_unused_labels_from_internal_stuff(&mut self) {
        let to_keep = self.active.labels_appearing();

        self.mapping_label_text.retain(|(l, _)| to_keep.contains(l));
        if let Some(x) = self.mapping_label_oldlabels.as_mut() {
            x.retain(|(l, _)| to_keep.contains(l))
        }
        if let Some(v) = self.mapping_oldlabel_labels.as_mut() {
            for (_, x) in v {
                x.retain(|l| to_keep.contains(l))
            }
        }

        // the assumption here is that discard_unused_labels_from_internal_stuff should be called only after changing the labels
        // so after simplifications/hardenings
        // so these variables should anyways be None
        // but to emphasize that they now may contain garbage, they are set to None
        self.trivial_sets = None;
        self.coloring_sets = None;
    }

    pub fn discard_useless_stuff(&mut self, recompute_full_diagram: bool, eh: &mut EventHandler) {
        // if passive side is maximized and some label gets discarded, it is still maximized, but some non-maximal lines may be present
        // zero-round solvability is preserved
        // coloring solvability is preserved
        // diagram may change

        self.diagram_indirect = None;
        self.diagram_direct = None;

        loop {
            let p = self.clone();
            // since the diagram may become wrong, the best thing to do here is to erase it
            self.diagram_indirect = None;
            self.diagram_direct = None;
            eh.notify("recompute diagram", 1, 1);
            if recompute_full_diagram {
                self.compute_diagram(eh);
            } else {
                self.compute_partial_diagram(eh);
            }
            eh.notify("remove weak", 1, 1);
            self.remove_weak_active_lines();
            eh.notify("discard non maximal", 1, 1);
            self.passive.discard_non_maximal_lines();
            self.active.discard_non_maximal_lines();
            eh.notify("discard labels at most one side", 1, 1);
            self.discard_labels_used_on_at_most_one_side_from_configurations();
            eh.notify("discard unused internal", 1, 1);
            self.discard_unused_labels_from_internal_stuff();
            if self == &p {
                break;
            }
        }
    }

    pub fn remove_weak_active_lines(&mut self) {
        let reachable = self.diagram_indirect_to_reachability_adj();

        // part 1: make groups smaller if possible
        for line in self.active.lines.iter_mut() {
            for part in line.parts.iter_mut() {
                let group = part.group.as_set();

                let mut newgroup = vec![];
                'outer: for &label in &group {
                    for &other in &group {
                        // ignoring labels that are equivalent
                        if label != other
                            && !reachable[&other].contains(&label)
                            && reachable[&label].contains(&other)
                        {
                            continue 'outer;
                        }
                    }
                    newgroup.push(label);
                }
                newgroup.sort_unstable();
                part.group = Group::from(newgroup);
            }
        }

        // part 2: remove lines by inclusion
        self.active
            .discard_non_maximal_lines_with_custom_supersets(Some(|h1: &Group, h2: &Group| {
                // h1 is superset of h2 if all elements of h2 have a successor in h1
                h2.iter().all(|x| {
                    h1.iter()
                        .any(|y| x == y || (reachable[x].contains(y) && !reachable[y].contains(x)))
                })
            }));
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn useless1() {
        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A B^2\n\nAB B\n");

        let mut p = Problem::from_string("M M M\nP UP UP\n\nM UP\nU U").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "M^3\nP U^2\n\nM PU\nMU U\n");

        let mut p = Problem::from_string("A AB AB\n\nAB AB").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A AB^2\n\nAB^2\n");
    }

    #[test]
    fn useless2() {
        let mut p = Problem::from_string("A A A\nA A B\n A B B\n\nB AB").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A B^2\n\nAB B\n");

        let mut p = Problem::from_string("M M M\nP U P\nP U U\nP P P\n\nM UP\nU U").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "M^3\nP U^2\n\nM PU\nMU U\n");

        let mut p = Problem::from_string("A A A\nA A B\n A B B\n\nAB AB").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A^3\nB A^2\nA B^2\n\nAB^2\n");

        let mut p = Problem::from_string("A A A\nA A B\n A B B\nC C C\n\nAB AB\nB C").unwrap();
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A B^2\n\nAB^2\n");

        let mut p = Problem::from_string("A A A\nA A B\n A B B\nC C C\n\nAB AB\nB ABC").unwrap();
        p.discard_useless_stuff(false, &mut EventHandler::null());
        assert_eq!(format!("{}", p), "A B^2\n\nAB^2\n");
    }
}
