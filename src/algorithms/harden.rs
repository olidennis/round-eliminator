use std::collections::HashSet;

use itertools::Itertools;

use crate::{problem::Problem, constraint::Constraint, line::Line, part::Part, group::Group};



impl Problem {
    pub fn harden(&self, keep : &HashSet<usize>) -> Self {

        let mut keep = keep.clone();

        let mut newactive = self.active.clone();
        let mut newpassive = self.passive.clone();

        loop {
            newactive = newactive.harden(&keep);
            newpassive = newpassive.harden(&keep);

            let appearing_active = newactive.labels_appearing();
            let appearing_passive = newpassive.labels_appearing();

            let newkeep : HashSet<usize> = appearing_active.intersection(&appearing_passive).cloned().collect();
            
            if newkeep == keep {
                break;
            }
            keep = newkeep;
        }

        Problem {
            active : newactive,
            passive : newpassive,
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
    fn harden(&self, keep : &HashSet<usize>) -> Self {
        // it seems that hardening preserves maximization (in the sense of still having all lines satisfying the forall, non-maximal lines may be present though), TODO: check
        let mut c = Constraint{ lines : vec![], is_maximized : self.is_maximized, degree : self.degree };
        for line in &self.lines {
            let newline = line.harden(keep);
            if newline.parts.iter().all(|part|!part.group.0.is_empty()) {
                c.lines.push(newline);
            }
        }
        c
    }
}



impl Line {
    pub fn harden(&self, keep : &HashSet<usize>) -> Self {
        let mut line = Line{ parts : vec![] };
        for part in &self.parts {
            let newpart = part.harden(keep);
            line.parts.push(newpart);
        }
        line.normalize();
        line
    }
}

impl Part {
    pub fn harden(&self, keep : &HashSet<usize>) -> Self {
        Part{ gtype : self.gtype, group : self.group.harden(keep) }
    }
}

impl Group {
    pub fn harden(&self, keep : &HashSet<usize>) -> Self {
        let h = self.as_set();
        let newgroup : Vec<_> = h.intersection(keep).cloned().sorted().collect();
        Group(newgroup)
    }
}