use crate::{constraint::Constraint, group::Group, problem::Problem};

impl Problem {
    pub fn relax_merge(&self, from: usize, to: usize) -> Self {
        let active = self.active.relax(from, to, true);
        let passive = self.passive.relax(from, to, true);

        Problem {
            active,
            passive,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
            mapping_oldlabel_text: self.mapping_oldlabel_text.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: self.diagram_indirect_old.clone()
        }
    }

    pub fn relax_addarrow(&self, from: usize, to: usize) -> Self {
        let passive = self.passive.relax(from, to, false);

        Problem {
            active: self.active.clone(),
            passive,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
            mapping_oldlabel_text: self.mapping_oldlabel_text.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: self.diagram_indirect_old.clone()
        }
    }
}

impl Constraint {
    pub fn relax(&self, from: usize, to: usize, remove_from: bool) -> Self {
        self.edited(|g| {
            let v = &g.0;
            if !v.contains(&from) {
                g.clone()
            } else {
                let mut h = g.as_set();
                if remove_from {
                    h.remove(&from);
                }
                h.insert(to);
                Group::from_set(&h)
            }
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::{problem::Problem, algorithms::event::EventHandler};

    #[test]
    fn relax_merge() {
        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_merge(0, 1);
        p.discard_useless_stuff(true, &EventHandler::null());
        assert_eq!(format!("{}", p), "B^3\n\nB^2\n");

        let p = Problem::from_string("M U U\nP P P\n\nM UP\nU U\n").unwrap();
        let mut p = p.relax_merge(2, 1);
        p.discard_useless_stuff(true, &EventHandler::null());
        p.compute_triviality(&EventHandler::null());
        assert_eq!(format!("{}", p), "U^3\n\nU^2\n");
        assert!(!p.trivial_sets.as_ref().unwrap().is_empty());

        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_addarrow(1, 0);
        p.discard_useless_stuff(true, &EventHandler::null());
        assert_eq!(format!("{}", p), "A AB^2\n\nAB^2\n");
        let p = p.merge_equivalent_labels();
        assert_eq!(format!("{}", p), "A^3\n\nA^2\n");
    }
}
