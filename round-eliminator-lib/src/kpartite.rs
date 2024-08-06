use std::{collections::{HashMap, HashSet}, fmt::Display, hash::Hash};

use crossbeam_channel::after;
use itertools::Itertools;
use permutator::CartesianProductIterator;

use crate::{algorithms::event::EventHandler, constraint::Constraint, group::{Group, GroupType, Label}, line::Line, part::Part};


struct KPartiteProblem{
    constraints : Vec<Constraint>,
    pub mapping_label_text: Vec<(Label, String)>,
    pub mapping_label_oldlabels: Option<Vec<(Label, Vec<Label>)>>,
}


/* 
#[test]
fn kpartite_sinkless_coloring(){
    let eh = &mut EventHandler::null();

    let mut mapping_label_text = HashMap::new();

    let c1 = Constraint::parse(
        "123 23",
        &mut mapping_label_text).unwrap();

    let c2 = Constraint::parse(
        "123 13",
        &mut mapping_label_text).unwrap();

    let c3 = Constraint::parse(
        "123 12",
        &mut mapping_label_text).unwrap();
    
    let mapping_label_text = mapping_label_text
        .into_iter()
        .map(|(a, b)| (b, a))
        .collect();
    
    let mut p = KPartiteProblem{ constraints: vec![c1,c2,c3], mapping_label_text, mapping_label_oldlabels: None  };
    println!("{}",p);

    for i in 0..3 {
        p = p.speedup_kpartite(|set|{
            let s_i = format!("{}",i%3 + 1);
            if set == [&s_i] {
                s_i.to_owned()
            } else {
                (*set.iter().filter(|&s| s != &&s_i).next().unwrap()).to_owned()
            }
        },eh);
        println!("{}",p);
    }
}*/



/* 
#[test]
fn kpartite_test(){
    let eh = &mut EventHandler::null();

    let mut mapping_label_text = HashMap::new();

    let c1 = Constraint::parse(
        "1 234\n2 34\n 3 4",
        &mut mapping_label_text).unwrap();

    let c2 = Constraint::parse(
        "1 234\n2 34\n 3 4",
        &mut mapping_label_text).unwrap();

    let c3 = Constraint::parse(
        "1 234\n2 34\n 3 4",
        &mut mapping_label_text).unwrap();

    let c4 = Constraint::parse(
        "1 234\n2 34\n 3 4",
        &mut mapping_label_text).unwrap();
    
    let mapping_label_text = mapping_label_text
        .into_iter()
        .map(|(a, b)| (b, a))
        .collect();
    
    let mut p = KPartiteProblem{ constraints: vec![c1,c2,c3,c4], mapping_label_text, mapping_label_oldlabels: None  };
    println!("{}",p);

    for i in 0..1 {
        p = p.speedup_kpartite(|set|{
            format!("({})",set.iter().join(""))
        },eh);
        for c in p.constraints.iter_mut() {
            c.maximize(eh);
        }
        println!("{}",p);
    }
    
}*/


impl KPartiteProblem{

    

    pub fn speedup_kpartite<F>(&self, zero_mapping : F, eh: &mut EventHandler) -> KPartiteProblem where F : Fn(&[&String]) -> String {
        let constraints = &self.constraints;

        let label_to_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let mut new_mapping_text_label = HashMap::new();
        let mut old_groups_to_new_label = HashMap::new();

        let mut new_constraints = vec![];
        for i in 1..constraints.len() {
            let mut new_constraint_i = constraints[i].clone();
            println!("maximizing");
            new_constraint_i.maximize(eh);
            println!("adding non maximal");
            new_constraint_i.add_non_maximal();
            println!("editing");

            let mut after_mapping_i = new_constraint_i.edited(|group|{
                let s_group : Vec<&String> = group.iter().map(|l|&label_to_text[l]).collect();
                let mapped = zero_mapping(&s_group);
                if !new_mapping_text_label.contains_key(&mapped) {
                    new_mapping_text_label.insert(mapped.clone(), new_mapping_text_label.len() as Label);
                }
                if !old_groups_to_new_label.contains_key(&group.as_vec()) {
                    old_groups_to_new_label.insert(group.as_vec(), new_mapping_text_label[&mapped]);
                }
                Group::from(vec![new_mapping_text_label[&mapped]])
            });
            println!("merged");
            after_mapping_i.lines = after_mapping_i.lines.iter().cloned().map(|mut l|{l.normalize(); l}).unique().sorted().collect();
            println!("maximizing again");
            //after_mapping_i.maximize(eh);
            println!("done");
            new_constraints.push(after_mapping_i);
        }


        let mut constraint_exists = constraints[0].edited(|g| {
            let h = g.as_set();
            let ng = old_groups_to_new_label
                .iter()
                .filter(|(o,_)| o.iter().any(|l| h.contains(l)))
                .map(|p| *p.1)
                .unique()
                .sorted()
                .collect();
            Group::from(ng)
        });
        constraint_exists.lines = constraint_exists.lines.iter().cloned().map(|mut l|{l.normalize(); l}).unique().sorted().collect();

        new_constraints.push(constraint_exists);

        KPartiteProblem {
            constraints : new_constraints,
            mapping_label_text: new_mapping_text_label.into_iter().map(|(a,b)|(b,a)).collect(),
            mapping_label_oldlabels: Some(old_groups_to_new_label.into_iter().map(|(a,b)|(b,a)).collect()),
        }
    }
}


impl Constraint {
    fn add_non_maximal(&mut self) {
        let mut lines : HashSet<_> = self.lines.iter().cloned().map(|mut l|{l.normalize(); l}).collect();
        for line in lines.clone() {
            let mut subsets = vec![];
            for part in &line.parts {
                for _ in 0..part.gtype.value() {
                    let group = part.group.as_vec();
                    let p : Vec<_> = group.iter().cloned().powerset().filter(|x|!x.is_empty()).collect();
                    subsets.push(p);
                }
            }
            let subsets : Vec<_> = subsets.iter().map(|v|&v[..]).collect();
            let all = CartesianProductIterator::new(&subsets);
            for choice in all {
                let mut sorted = choice.clone();
                sorted.sort_unstable();
                let mut new = Line {
                    parts: choice
                        .into_iter()
                        .map(|g| Part {
                            group: Group::from(g.clone()),
                            gtype: GroupType::ONE,
                        })
                        .collect(),
                };
                new.normalize();
                lines.insert(new);
            }
        }
        self.lines = lines.into_iter().sorted().collect();
    }
}

impl Display for KPartiteProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mapping = self.mapping_label_text.iter().cloned().collect();
        writeln!(f,"--------------")?;
        for (i,constraint) in self.constraints.iter().enumerate() {
            writeln!(f,"Constraint {}",i+1)?;
            for line in &constraint.lines {
                writeln!(f, "{}", line.to_string(&mapping))?;
            }
            writeln!(f)?;
        }
        writeln!(f,"--------------")?;
        Ok(())
    }
}