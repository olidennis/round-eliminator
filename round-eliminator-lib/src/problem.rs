use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{algorithms::event::EventHandler, constraint::Constraint, group::{Exponent, Group, GroupType, Label}, line::Line, part::Part};
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
    pub demisifiable : Option<Vec<(Vec<Label>,Vec<Label>)>>,
    pub is_trivial_with_input : Option<bool>,
    pub triviality_with_input : Option<(Vec<(Label, String)>,Vec<(Label, Vec<Label>)>)>,
    pub expressions : Option<String>
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
            demisifiable : None,
            is_trivial_with_input : None,
            triviality_with_input : None,
            expressions : None
        }
    }

    pub fn from_string_active_passive<S: AsRef<str>>(
        active: S,
        passive: S,
    ) -> Result<(Self,bool), &'static str> {
        let mut mapping_label_text = HashMap::new();

        let active = Constraint::parse(active, &mut mapping_label_text)?;
        let passive = Constraint::parse(passive, &mut mapping_label_text)?;

        let missing_labels = active.labels_appearing() != passive.labels_appearing();

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
            demisifiable : None,
            is_trivial_with_input : None,
            triviality_with_input : None,
            expressions : None
        };
        Ok((p,missing_labels))
    }

    pub fn from_string<S: AsRef<str>>(s: S) -> Result<Self, &'static str> {
        let s = s.as_ref();
        let mut lines = s.lines();

        let active = lines.by_ref().take_while(|l| !l.is_empty()).join("\n");
        let passive = lines.take_while(|l| !l.is_empty()).join("\n");

        Self::from_string_active_passive(active, passive).map(|p|p.0)
    }

    pub fn labels(&self) -> Vec<Label> {
        self.active.labels_appearing().into_iter().chain(self.passive.labels_appearing().into_iter()).unique().sorted().collect()
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
    
    pub fn add_active_predecessors(&mut self) {
        let pred = self.diagram_indirect_to_inverse_reachability_adj();
        self.active = self.active.edited(|g|{
            let g = g.iter().fold(HashSet::new(),|mut a,l|{
                a.extend(pred[l].iter().cloned());
                a
            });
            Group::from_set(&g)
        });
    }
    
    pub fn remove_trivial_lines(&self) -> Self {
        let mut result = HashSet::new();
        let mut handled = HashSet::new();

        let pred = self.diagram_direct_to_pred_adj();
        for line in self.active.all_choices(true) {
            self.remove_trivial_lines_rec(line, &pred, &mut result, &mut handled);
        }
        let c = Constraint{
            lines : result.into_iter().sorted().collect(),
            degree : self.active.degree,
            is_maximized: false,
        };
        Self {
                active : c,
                passive : self.passive.clone(),
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

    pub fn remove_trivial_lines_rec(&self, line : Line, pred : &HashMap<Label, HashSet<Label>>, result : &mut HashSet<Line>, handled : &mut HashSet<Line>) {
        if handled.contains(&line) {
            return;
        }
        handled.insert(line.clone());
        if !self.is_line_trivial_with_assumed_input(&line) {
            result.insert(line);
        } else {
            for i in 0..line.parts.len() {
                let l = line.parts[i].group.first();
                for &p in &pred[&l] {
                    let mut newline = line.clone();
                    newline.parts[i].gtype = GroupType::Many(newline.parts[i].gtype.value() as Exponent - 1);
                    newline.parts.push(
                        Part { gtype: GroupType::ONE, group: Group::from(vec![p]) }
                    );
                    newline.normalize();
                    self.remove_trivial_lines_rec(newline, pred, result, handled);
                }
            }
        }
    }

    

    pub fn is_line_trivial(&self, line : &Line) -> bool {
        assert!(line.parts.iter().all(|p|p.group.len() == 1));
        let h = line.groups().fold(HashSet::new(),|mut h : HashSet<Label>,g|{
            h.extend(g.iter());
            h
        });
        let group = Group::from_set(&h);
        let passive_degree = crate::group::GroupType::Many(self.passive.finite_degree() as Exponent);
        let part = Part {
            gtype: passive_degree,
            group,
        };
        let passive_line = Line { parts: vec![part] };
        self.passive.includes(&passive_line)
    }

    pub fn is_line_trivial_with_given_orientation(&self, line : &Line, outdegree : usize) -> bool {
        assert!(line.parts.iter().all(|p|p.group.len() == 1));
        for (g1,g2) in line.minimal_splits(outdegree) {
            let p1 = Part {
                gtype: GroupType::ONE,
                group: g1,
            };
            let p2 = Part {
                gtype: GroupType::ONE,
                group: g2,
            };
            let line = Line {
                parts: vec![p1, p2],
            };
            if self.passive.includes(&line) {
                return true;
            }
        }
        false
    }

    pub fn is_line_trivial_with_assumed_input(&self, line : &Line) -> bool {
        if self.is_line_trivial(line) {
            return true;
        }
        if let Some(out) = self.orientation_given {
            if self.is_line_trivial_with_given_orientation(line, out) {
                return true;
            }
        }
        false
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

