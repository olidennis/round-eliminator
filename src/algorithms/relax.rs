use crate::{problem::Problem, constraint::Constraint, line::Line, part::Part, group::Group};



impl Problem {

    pub fn relax_merge(&self, from: usize, to: usize) -> Self {
        let active = self.active.relax(from,to, true);
        let passive = self.passive.relax(from,to, true);

        Problem {
            active,
            passive,
            mapping_label_text : self.mapping_label_text.clone(),
            mapping_label_oldlabels : self.mapping_label_oldlabels.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
        }
    }
    
    pub fn relax_addarrow(&self, from: usize, to: usize) -> Self {
        let passive = self.passive.relax(from,to, false);

        Problem {
            active : self.active.clone(),
            passive,
            mapping_label_text : self.mapping_label_text.clone(),
            mapping_label_oldlabels : self.mapping_label_oldlabels.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
        }
    }
}

impl Constraint {
    pub fn relax(&self, from: usize, to: usize, remove_from : bool) -> Self {
        let mut c = Constraint{ lines : vec![], is_maximized : false, degree : self.degree };
        for line in &self.lines {
            let newline = line.relax(from,to, remove_from);
            c.lines.push(newline);
        }
        c
    }
}

impl Line {
    pub fn relax(&self, from: usize, to: usize, remove_from : bool) -> Self {
        let mut line = Line{ parts : vec![] };
        for part in &self.parts {
            let newpart = part.relax(from,to, remove_from);
            line.parts.push(newpart);
        }
        line.normalize();
        line
    }
}

impl Part {
    pub fn relax(&self, from: usize, to: usize, remove_from : bool) -> Self {
        Part{ gtype : self.gtype, group : self.group.relax(from,to, remove_from) }
    }
}

impl Group {
    pub fn relax(&self, from: usize, to: usize, remove_from : bool) -> Self {
        let v = &self.0;
        if !v.contains(&from) {
            self.clone()
        } else {
            let mut h = self.as_set();
            if remove_from {
                h.remove(&from);
            }
            h.insert(to);
            Group::from_set(&h)
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::problem::Problem;

    #[test]
    fn relax_merge() {
        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_merge(0, 1);
        p.discard_useless_stuff(true);
        assert_eq!(format!("{}", p), "B^3\n\nB^2\n");

        let p = Problem::from_string("M U U\nP P P\n\nM UP\nU U\n").unwrap();
        let mut p = p.relax_merge(2, 1);
        p.discard_useless_stuff(true);
        p.compute_triviality();
        assert_eq!(format!("{}", p), "U^3\n\nU^2\n");
        assert!(!p.trivial_sets.as_ref().unwrap().is_empty());

        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_addarrow(1, 0);
        p.discard_useless_stuff(true);
        assert_eq!(format!("{}", p), "A AB^2\n\nAB^2\n");
        let p = p.merge_equivalent_labels();
        assert_eq!(format!("{}", p), "A^3\n\nA^2\n");
    }
}