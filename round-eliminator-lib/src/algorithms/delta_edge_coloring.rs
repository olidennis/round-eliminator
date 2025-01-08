use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use permutator::Permutation;

use crate::{
    constraint::Constraint, group::{Group, Label}, line::Line, problem::Problem
};

impl Problem {
    pub fn duplicate_labels_delta_edge_coloring(&self) -> Self {
        let mut active = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.active.degree,
        };
        let mut next_label = 0;
        let mut mapping_oldlabel_labels_by_color = (0..active.finite_degree()).map(|_|HashMap::<Label, Vec<Label>>::new()).collect_vec();
        let mut seen = HashSet::new();
        
        for line in self.active.all_choices(false) {
            let mut parts = line.parts.clone();
            for perm in parts.permutation() {
                if !seen.insert(perm.clone()) {
                    continue;
                }
                let lineperm = Line{ parts : perm };
                let mut i = 0;
                let mut newline = lineperm.edited(|g| {
                    let label = g.first();
                    let new_label = next_label;
                    next_label += 1;
                    mapping_oldlabel_labels_by_color[i]
                        .entry(label)
                        .or_default()
                        .push(new_label);
                    i += 1;
                    Group::from(vec![new_label])
                });
                newline.normalize();
                active.lines.push(newline);
            }
        }

        let empty = vec![];
        let mut passive = Constraint{ lines: vec![], is_maximized: false, degree: self.passive.degree };
        for i in 0..active.finite_degree() {
            let passive_i = self.passive.edited(|g| {
                Group::from(
                    g.iter()
                        .flat_map(|label| {
                            mapping_oldlabel_labels_by_color[i]
                                .get(label)
                                .unwrap_or(&empty)
                                .iter()
                                .cloned()
                        })
                        .unique()
                        .sorted()
                        .collect(),
                )
            });
            passive.lines.extend(passive_i.lines.into_iter());
        }

        let mut mapping_oldlabel_labels = HashMap::<Label, Vec<Label>>::new();
        for i in 0..active.finite_degree() {
            for (k,v) in mapping_oldlabel_labels_by_color[i].iter() {
                mapping_oldlabel_labels.entry(*k).or_default().extend(v.into_iter());
            }
        }
        
        let mapping_oldlabel_labels = mapping_oldlabel_labels.into_iter().sorted().collect();

        let mut p = Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text: vec![],
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: Some(mapping_oldlabel_labels),
            mapping_oldlabel_text: Some(self.mapping_label_text.clone()),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_indirect_old: self.diagram_indirect.clone(),
            diagram_direct: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: self.orientation_given,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None,
            is_trivial_with_input : None
        };
        p.assign_chars();
        p
    }
}
