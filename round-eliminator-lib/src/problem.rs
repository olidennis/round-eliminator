use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{constraint::Constraint, group::{Group, Label}};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use crate::algorithms::fixpoint::FixpointDiagram;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    pub active: Constraint,
    pub passive: Constraint,
    pub passive_gen: Option<Constraint>,
    pub mapping_label_text: Vec<(Label, String)>,
    pub mapping_label_oldlabels: Option<Vec<(Label, Vec<Label>)>>,
    pub mapping_oldlabel_labels: Option<Vec<(Label, Vec<Label>)>>,
    pub mapping_oldlabel_text: Option<Vec<(Label, String)>>,
    pub trivial_sets: Option<Vec<Vec<Label>>>,
    pub coloring_sets: Option<Vec<Vec<Label>>>,
    pub diagram_indirect: Option<Vec<(Label, Label)>>,
    pub diagram_indirect_old: Option<Vec<(Label, Label)>>,
    pub diagram_direct: Option<DiagramDirect>,
    pub orientation_given: Option<usize>,
    pub orientation_trivial_sets: Option<Vec<(Vec<Label>, Vec<Label>)>>,
    pub orientation_coloring_sets: Option<Vec<(Vec<Label>, Vec<Label>)>>,
    pub fixpoint_diagram : Option<(Option<Vec<Label>>,FixpointDiagram)>,
    pub fixpoint_procedure_works : Option<bool>,
    pub marks_works : Option<bool>,
    pub demisifiable : Option<Vec<(Vec<Label>,Vec<Label>)>>
}

pub type DiagramDirect = (Vec<(Label, Vec<Label>)>, Vec<(Label, Label)>);

impl Problem {

    pub fn replace_passive(&self, passive : Constraint) -> Self {
        Self {
            active : self.active.clone(),
            passive,
            passive_gen : None,
            mapping_label_text : self.mapping_label_text.clone(),
            mapping_label_oldlabels: self.mapping_label_oldlabels.clone(),
            mapping_oldlabel_labels: self.mapping_oldlabel_labels.clone(),
            mapping_oldlabel_text: self.mapping_oldlabel_text.clone(),
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: None,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None
        }
    }

    pub fn from_string_active_passive<S: AsRef<str>>(
        active: S,
        passive: S,
    ) -> Result<Self, &'static str> {
        let mut mapping_label_text = HashMap::new();

        let active = Constraint::parse(active, &mut mapping_label_text)?;
        let passive = Constraint::parse(passive, &mut mapping_label_text)?;

        let mapping_label_text = mapping_label_text
            .into_iter()
            .map(|(a, b)| (b, a))
            .collect();

        let p = Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text,
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: None,
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: None,
            fixpoint_diagram : None,
            fixpoint_procedure_works : None,
            marks_works : None,
            demisifiable : None
        };
        Ok(p)
    }

    pub fn from_string<S: AsRef<str>>(s: S) -> Result<Self, &'static str> {
        let s = s.as_ref();
        let mut lines = s.lines();

        let active = lines.by_ref().take_while(|l| !l.is_empty()).join("\n");
        let passive = lines.take_while(|l| !l.is_empty()).join("\n");

        Self::from_string_active_passive(active, passive)
    }

    pub fn labels(&self) -> Vec<Label> {
        self.active.labels_appearing().into_iter().sorted().collect()
        //let mut labels: Vec<_> = self.mapping_label_text.iter().map(|(l, _)| *l).collect();
        //labels.sort_unstable();
        //labels
    }

    pub fn is_mergeable(&self) -> bool {
        self.diagram_direct.is_some() && self.diagram_direct.as_ref().unwrap().0.iter().any(|x|x.1.len() > 1)
    }
    
    pub fn compute_passive_gen(&mut self) {
        let predecessors = self.diagram_indirect_to_inverse_reachability_adj();
        self.passive_gen = None;
        if !self.is_mergeable() {
            let passive_gen = self.passive.edited(|g|{
                let h : HashSet<_> = g.as_set();
                Group::from(g.iter().cloned().filter(|x|{
                    predecessors[x].intersection(&h).count() == 1
                }).collect())
            });
            self.passive_gen = Some(passive_gen);
        }
    }

    
}

impl Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mapping = self.mapping_label_text.iter().cloned().collect();
        for line in &self.active.lines {
            writeln!(f, "{}", line.to_string(&mapping))?;
        }
        writeln!(f)?;
        for line in &self.passive.lines {
            writeln!(f, "{}", line.to_string(&mapping))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    #[should_panic]
    fn parsing_err() {
        let _ = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD**").unwrap();
    }

    #[test]
    fn parsing() {
        let p = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD").unwrap();
        assert_eq!(format!("{}", p), "ABCD^108\n\nAB CD\n");

        let p = Problem::from_string("A AB*\nC CD*\n\nAB CD").unwrap();
        assert_eq!(format!("{}", p), "A AB*\nC CD*\n\nAB CD\n");
    }

    #[test]
    fn serialize() {
        let mut p = Problem::from_string("M U*\nP*\n\nM UP*\nU*").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B B\nC D D\n\nAB AB\nCD CD").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B B\nC D D\n\nAB CD").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_coloring_solvability(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B AB C\n\nAB AB\nC C").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_coloring_solvability(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("M U*\nP*\n\nM UP*\nU*")
            .unwrap()
            .speedup(&mut eh);
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);
    }
}

