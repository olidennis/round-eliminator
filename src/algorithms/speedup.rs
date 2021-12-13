use std::collections::HashMap;

use itertools::Itertools;

use crate::{problem::Problem, group::Group};

use super::event::EventHandler;



impl Problem {
    pub fn speedup(&self, eh : &EventHandler) -> Self {
        let mut newactive_before_renaming = self.passive.clone();
        newactive_before_renaming.maximize(eh);

        let mapping_label_oldlabels : Vec<_> =  newactive_before_renaming.groups().unique().map(|g|g.0.clone()).enumerate().collect();
        let h_oldlabels_label : HashMap<_,_> = mapping_label_oldlabels.iter().map(|(a,b)|(b.clone(),*a)).collect();

        let active = newactive_before_renaming.edited(|g|{
            Group(vec![h_oldlabels_label[&g.0]])
        });

        let passive = self.active.edited(|g|{
            let h = g.as_set();
            let ng = mapping_label_oldlabels.iter().filter(|(_,o)|{
                o.iter().any(|l|h.contains(l))
            }).map(|p|p.0).sorted().collect();
            Group(ng)
        });

        let mut p = Problem { 
            active, 
            passive, 
            mapping_label_text: vec![], 
            mapping_label_oldlabels : Some(mapping_label_oldlabels), 
            mapping_oldlabel_text: Some(self.mapping_label_text.clone()), 
            trivial_sets: None, 
            coloring_sets: None, 
            diagram_indirect: None, 
            diagram_indirect_old: self.diagram_indirect.clone(),
            diagram_direct: None
        };
        p.assign_chars();
        p
    }

    pub fn assign_chars(&mut self) {
        let labels : Vec<_> = self.mapping_label_oldlabels.as_ref().unwrap().iter().map(|(l,_)|*l).collect();
        
        self.mapping_label_text = labels.iter()
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
}

#[cfg(test)]
mod tests {

    use crate::{problem::Problem, algorithms::event::EventHandler};

    #[test]
    fn speedup() {
        let p = Problem::from_string("M U U U\nP P P P\n\nM M\nU PU").unwrap();
        let mut p = p.speedup(&EventHandler::null());
        p.compute_diagram(&EventHandler::null());
        p.sort_active_by_strength();
        assert_eq!(format!("{}", p), "A^2\nB C\n\nA BC^3\nC^4\n");


    }


}