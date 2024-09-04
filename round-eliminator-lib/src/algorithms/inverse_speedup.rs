use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    constraint::Constraint,
    group::{Group, Label},
    problem::Problem,
};

impl Problem {
    pub fn inverse_speedup(&self) -> Self {
        let mut passive = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.active.degree,
        };
        let mut next_label = 0;
        let mut mapping_oldlabel_labels = HashMap::<Label, Vec<Label>>::new();

        for line in self.active.all_choices(false) {
            let mut newline = line.edited(|g| {
                let label = g.first();
                let new_label = next_label;
                next_label += 1;
                mapping_oldlabel_labels
                    .entry(label)
                    .or_default()
                    .push(new_label);
                Group::from(vec![new_label])
            });
            newline.normalize();
            passive.lines.push(newline);
        }

        let empty = vec![];
        let active = self.passive.edited(|g| {
            Group::from(
                g.iter()
                    .flat_map(|label| {
                        mapping_oldlabel_labels
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
            demisifiable : None
        };
        p.assign_chars();
        p
    }
}
