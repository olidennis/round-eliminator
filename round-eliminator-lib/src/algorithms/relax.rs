use std::collections::{HashMap, HashSet};

use crate::{
    constraint::Constraint,
    group::{Group, Label},
    problem::Problem,
};

impl Problem {

    pub fn relax_merge_group(&self, from : &Vec<Label>, to: Label) -> Self {
        let active = self.active.relax_group(from, to, true);
        let passive = self.passive.relax_group(from, to, true);

        Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
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
            triviality_with_input : None,
            expressions : None
        }
    }

    pub fn relax_merge(&self, from: Label, to: Label) -> Self {
        let active = self.active.relax(from, to, true);
        let passive = self.passive.relax(from, to, true);

        Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
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
            triviality_with_input : None,
            expressions : None
        }
    }

    pub fn relax_many_merges(&self, merges : &Vec<(Label,Label)>) -> Self {
        let mut active = self.active.clone();
        let mut passive = self.passive.clone();
        let mut merges = merges.clone();
        for i in 0..merges.len() {
            let (from,to) = merges[i];
            if from != to {
                active = active.relax(from, to, true);
                passive = passive.relax(from, to, true);
                for j in i+1..merges.len() {
                    if merges[j].0 == from {
                        merges[j].0 = to;
                    }
                    if merges[j].1 == from {
                        merges[j].1 = to;
                    }
                }
            }
        }

        Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
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
            triviality_with_input : None,
            expressions : None
        }
    }

    pub fn relax_addarrow(&self, from: Label, to: Label) -> Self {
        let diagram = self.diagram_indirect_to_reachability_adj();
        let succ : Vec<_> = diagram[&to].iter().cloned().collect();
        let mut passive = self.passive.clone();
        for l in succ {
            passive = self.passive.relax(from, l, false);
        }

        Problem {
            active: self.active.clone(),
            passive,
            passive_gen : None,
            mapping_label_text: self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
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
            triviality_with_input : None,
            expressions : None
        }
    }
}

impl Constraint {
    pub fn relax(&self, from: Label, to: Label, remove_from: bool) -> Self {
        self.edited(|g| {
            let v = g.as_vec();
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
    pub fn relax_group(&self, from: &Vec<Label>, to: Label, remove_from: bool) -> Self {
        let from = HashSet::from_iter(from.into_iter().cloned());

        self.edited(|g| {
            let mut h = g.as_set();
            if from.intersection(&h).next().is_none() {
                g.clone()
            } else {
                if remove_from {
                    h = h.difference(&from).cloned().collect();
                }
                h.insert(to);
                Group::from_set(&h)
            }
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn relax_merge() {
        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_merge(0, 1);
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "B^3\n\nB^2\n");

        let p = Problem::from_string("M U U\nP P P\n\nM UP\nU U\n").unwrap();
        let mut p = p.relax_merge(2, 1);
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.compute_triviality(&mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "U^3\n\nU^2\n");
        assert!(!p.trivial_sets.as_ref().unwrap().is_empty());

        let p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        let mut p = p.relax_addarrow(1, 0);
        p.discard_useless_stuff(true, &mut EventHandler::null());
        p.passive.maximize(&mut EventHandler::null());
        assert_eq!(format!("{}", p), "A AB^2\n\nAB^2\n");
        let p = p.merge_equivalent_labels();
        assert_eq!(format!("{}", p), "A^3\n\nA^2\n");
    }
}
