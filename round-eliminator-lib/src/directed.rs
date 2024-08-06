use std::{collections::{HashMap, HashSet}, fmt::Display};

use serde::{de::Expected, Deserialize, Serialize};

use crate::{algorithms::event::EventHandler, constraint::Constraint, group::{Exponent, Group, GroupType, Label}, line::Degree, part::Part};
use itertools::Itertools;



#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectedProblem {
    pub constraints : Vec<(Group,Constraint)>,
    pub mapping_label_text: Vec<(Label, String)>,
    pub mapping_label_oldlabels: Option<Vec<(Label, Vec<Label>)>>,
    pub mapping_oldlabel_labels: Option<Vec<(Label, Vec<Label>)>>,
    pub mapping_oldlabel_text: Option<Vec<(Label, String)>>
}

impl DirectedProblem {
    pub fn from_string<S: AsRef<str>>(s: S) -> Result<Self, &'static str> {

        let s = s.as_ref();

        let mut string_constraints = HashMap::<&str,Vec<&str>>::new();

        for line in s.lines() {
            let mut parts = line.split(" : ");
            let label = parts.next().unwrap();
            let line = parts.next().unwrap();
            string_constraints.entry(label).or_default().push(line);
        }

        let mut mapping_label_text = HashMap::new();

        let mut constraints = vec![];
        for (label,constraint) in string_constraints.into_iter().sorted() {
            let part = Part::parse(label, &mut mapping_label_text)?;
            if part.gtype != GroupType::Many(1) {
                return Err("Only one-element groups are supported as predecessors");
            }
            let head = part.group;
            let constraint = constraint.join("\n");
            let constraint = Constraint::parse(constraint, &mut mapping_label_text)?;
            constraints.push((head, constraint));
        }

        let mapping_label_text = mapping_label_text
            .into_iter()
            .map(|(a, b)| (b, a))
            .collect();

        let p = DirectedProblem {
            constraints,
            mapping_label_text,
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: None,
        };
        Ok(p)
    }

    pub fn labels(&self) -> Vec<Label> {
        let mut labels: Vec<_> = self.mapping_label_text.iter().map(|(l, _)| *l).collect();
        labels.sort_unstable();
        labels
    }

    pub fn maximize(&mut self, eh: &mut EventHandler) {
        let mut new_constraints = vec![];

        let degree = self.constraints[0].1.degree;
        for label in self.labels() {
            let mut constraint = Constraint{ lines : vec![], is_maximized : false, degree };

            for (h,c) in &mut self.constraints {
                if h.contains(&label){
                    for line in &c.lines {
                        constraint.add_line_and_discard_non_maximal(line.clone());
                    }
                }
            }
            if !constraint.lines.is_empty() {
                constraint.maximize(eh);
                new_constraints.push((Group::from(vec![label]),constraint));
            }
        }

        self.constraints = new_constraints;
    }

    pub fn groups(&self) -> impl Iterator<Item = &'_ Group> {
        self.constraints.iter().map(|(_,c)|c).flat_map(|c| c.groups())
    }


    pub fn assign_chars(&mut self) {
        let labels: Vec<_> = if self.mapping_label_oldlabels.is_some() {
            self.mapping_label_oldlabels
                .as_ref()
                .unwrap()
                .iter()
                .map(|(l, _)| *l)
                .collect()
        } else {
            self.mapping_oldlabel_labels
                .as_ref()
                .unwrap()
                .iter()
                .flat_map(|(_, labels)| labels.iter().cloned())
                .unique()
                .sorted()
                .collect()
        };

        self.mapping_label_text = labels
            .iter()
            .map(|&i| {
                if labels.len() <= 62 {
                    let i8 = i as u8;
                    let c = match i {
                        0..=25 => (b'A' + i8) as char,
                        26..=51 => (b'a' + i8 - 26) as char,
                        52..=61 => (b'0' + i8 - 52) as char,
                        _ => (b'z' + 1 + i8 - 62) as char,
                    };
                    (i, format!("{}", c))
                } else {
                    (i, format!("({})", i))
                }
            })
            .collect()
    }

    pub fn speedup(&self, eh: &mut EventHandler) -> Self {
        let mut p = self.clone();
        p.maximize(eh);

        let mut new_labels = p.groups().cloned().collect::<HashSet<_>>();
        loop {
            let mut new_labels_temp = new_labels.clone();
            for g1 in &new_labels {
                for g2 in &new_labels {
                    let int = g1.intersection(g2);
                    if !int.is_empty() {
                        new_labels_temp.insert(int);
                    }
                }
            }
            if new_labels.len() == new_labels_temp.len() {
                break;
            }
            new_labels = new_labels_temp;
        }

        let mapping_label_oldlabels: Vec<_> = new_labels.into_iter()
            .map(|g|g.as_vec())
            .sorted_by_key(|v| v.iter().cloned().rev().collect::<Vec<Label>>())
            .enumerate()
            .map(|(a, b)| (a as Label, b))
            .collect();

        let mut new_constraints = vec![];
        for (h,c) in p.constraints {
            let h = h.first();
            let new_h : Vec<_> = mapping_label_oldlabels.iter().filter(|(_,oldlabels)|oldlabels.contains(&h)).map(|(newlabel,_)|*newlabel).sorted().collect();
            let new_c = c.edited(|g|{
                Group::from(mapping_label_oldlabels.iter().filter(|(_,oldlabels)|  g.is_superset(&Group::from(oldlabels.clone()))).map(|(newlabel,_)|*newlabel).sorted().collect())
            });
            new_constraints.push((Group::from(new_h),new_c));
        }

        let mut new_problem = DirectedProblem {
            constraints : new_constraints,
            mapping_label_text: vec![],
            mapping_label_oldlabels: Some(mapping_label_oldlabels),
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: Some(self.mapping_label_text.clone()),
        };
        new_problem.assign_chars();
        new_problem
    }

    pub fn is_trivial(&self) -> bool {
        let degree = self.constraints[0].1.finite_degree() as Exponent;
        for (g,c) in &self.constraints {
            if c.includes(&crate::line::Line{ parts: vec![Part{ gtype: GroupType::Many(degree), group: g.clone()}]  }) {
                return true;
            }
        }
        false
    }
}


impl Display for DirectedProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mapping : HashMap<Label, String> = self.mapping_label_text.iter().cloned().collect();
        for (head,constraint) in &self.constraints {
            for line in &constraint.lines {
                for label in head.iter() {
                    write!(f, "{}",&mapping[label])?;
                }
                writeln!(f, " : {}",line.to_string(&mapping))?;
            }
        }
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::algorithms::event::EventHandler;

    use super::DirectedProblem;
    use itertools::Itertools;


    #[test]
    fn directed() {
        // step 0
        //let mut p = DirectedProblem::from_string("W : Bb BbW\nBb : W BW\nb : W W\nb : W B\nb : B B").unwrap();
        //let mut p = DirectedProblem::from_string("BbXY : BWX W\nbY : BWX^2\nWXY : Bb BbWXY").unwrap();
        // relax
        //let mut p = DirectedProblem::from_string("BbXY : BWXY W\nbY : BWX BWXY\nWXY : Bb BbWXY").unwrap();

        //let mut p = DirectedProblem::from_string("W : Bb BbWX\nB : BWX W\nb : BWX BWX\nX : BWb BWXb").unwrap();
        let mut p = DirectedProblem::from_string(
"A : BC(BC)^2
B : AC(AC)^2
C : AB(AB)^2
(AB) : A(AC)^2
(AB) : C(AC)^2
(AB) : B(BC)^2
(AB) : C(BC)^2
(AC) : A(AB)^2
(AC) : B(AB)^2
(AC) : B(BC)^2
(AC) : C(BC)^2
(BC) : A(AB)^2
(BC) : B(AB)^2
(BC) : A(AC)^2
(BC) : C(AC)^2"
        ).unwrap();

        let mut p = DirectedProblem::from_string(
"A : BC(BC)^2
B : AC(AC)^2
C : AB(AB)^2
(AB) : AC(AC)^2
(AB) : BC(BC)^2
(AC) : AB(AB)^2
(AC) : BC(BC)^2
(BC) : AB(AB)^2
(BC) : AC(AC)^2"
        ).unwrap();

/*

use itertools::Itertools;

fn sets_avoiding(labels : &[char], avoid : char) -> Vec<String> {
    labels.iter().powerset().filter(|set|{
        set.iter().all(|&&c|c != avoid)
    }).map(|set|set.into_iter().collect()).collect()
}

fn sets_of_size(labels : &[char], size : usize) -> Vec<String> {
    labels.iter().combinations(size).map(|set|set.into_iter().collect()).collect()
}

fn main(){
    let labels : Vec<_> = "ABCD".chars().collect();
    
    for i in 1..labels.len() {
        for set1 in sets_of_size(&labels,i) {
            for avoid in set1.chars() {
                print!("({}) : ",set1);
                for set2 in sets_avoiding(&labels,avoid).iter().filter(|s|*s!="") {
                    print!("({})",set2);
                }
                println!("^2");
            }
        }
    }
    
}

*/

        //let mut p = DirectedProblem::from_string("A : BCDE BCDE\nB : ACDE ACDE\nC : ABDE ABDE\nD : ABCE ABCE\nE : ABCD ABCD").unwrap();
        //let mut p = DirectedProblem::from_string("W : Bb WbB\nB : wW WBw\nw : WBb WBb\nb : WBw WBw").unwrap();
        //let mut p = DirectedProblem::from_string("M : UD UD\nU : M MU\nD : UM UM").unwrap();
        //let mut p = DirectedProblem::from_string("M : UDX UDX\nU : MX MUX\nD : UMX UMX\nX : UMD UMDX").unwrap();
        //let mut p = DirectedProblem::from_string("A : B AB\nB : A AB").unwrap();
        //let mut p = DirectedProblem::from_string("B : A AB B\nA : A AB B").unwrap();

        //let mut p = DirectedProblem::from_string("A : B B\nB : A A").unwrap();


        let mut eh = EventHandler::null();
        p.maximize(&mut eh);
        println!("{}",p);

        for _ in 0..16 {
            p = p.speedup(&mut eh);
            println!("--- Renaming ---");
            for (label,oldlabels) in p.mapping_label_oldlabels.as_ref().unwrap().iter() {
                let mapping : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();
                let oldmapping : HashMap<_,_> = p.mapping_oldlabel_text.as_ref().unwrap().iter().cloned().collect();
                println!("{} <- {}",mapping[label],oldlabels.iter().map(|l|&oldmapping[l]).join(""));
            }
            println!("\n--- Speedup ---");
            println!("{}",p);

            println!("--- Maximized ---");
            p.maximize(&mut eh);
            println!("{}",p);

            println!("--- Triviality: {} --- Labels: {} ---",p.is_trivial(),p.labels().len());

        }

    }
}