use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    group::{Group, Label},
    problem::Problem,
};

use super::event::EventHandler;

impl Problem {
    pub fn speedup(&self, eh: &mut EventHandler) -> Self {
        let mut newactive_before_renaming = self.passive.clone();
        newactive_before_renaming.maximize(eh);

        let mapping_label_oldlabels: Vec<_> = newactive_before_renaming
            .groups()
            .unique()
            .map(|g| g.as_vec())
            .sorted_by_key(|v| v.iter().cloned().rev().collect::<Vec<Label>>())
            .enumerate()
            .map(|(a, b)| (a as Label, b))
            .collect();
        let h_oldlabels_label: HashMap<_, _> = mapping_label_oldlabels
            .iter()
            .map(|(a, b)| (b.clone(), *a))
            .collect();

        let active = newactive_before_renaming.edited(|g| Group::from(vec![h_oldlabels_label[&g.as_vec()]]));

        let passive = self.active.edited(|g| {
            let h = g.as_set();
            let ng = mapping_label_oldlabels
                .iter()
                .filter(|(_, o)| o.iter().any(|l| h.contains(l)))
                .map(|p| p.0)
                .sorted()
                .collect();
            Group::from(ng)
        });

        let mut p = Problem {
            active,
            passive,
            passive_gen : None,
            mapping_label_text: vec![],
            mapping_label_oldlabels: Some(mapping_label_oldlabels),
            mapping_oldlabel_labels: None,
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

    pub fn assign_chars(&mut self) {
        if self.mapping_label_oldlabels.is_some() {
            let labels: Vec<_> = self.mapping_label_oldlabels
                .as_ref()
                .unwrap()
                .iter()
                .map(|(l, _)| *l)
                .collect();
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
                .collect();
        } else {
            let old_to_text = self.mapping_oldlabel_text.as_ref().unwrap(). iter().cloned().collect::<HashMap<_,_>>();
            self.mapping_label_text = self.mapping_oldlabel_labels
                .as_ref()
                .unwrap()
                .iter()
                .flat_map(|(oldlabel, labels)|{
                    if labels.len() == 1 {
                        labels.iter().enumerate().map(|(i,l)|(*l,format!("{}",old_to_text[oldlabel]))).collect_vec().into_iter()
                    }else{
                        labels.iter().enumerate().map(|(i,l)|{
                            let old_to_new = old_to_text[oldlabel].replace("(","[").replace(")","]");
                            (*l,format!("({}_{})",old_to_new,i+1))
                        }).collect_vec().into_iter()
                    }
                })
                .unique()
                .sorted()
                .collect();
        };        
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn speedup() {
        let p = Problem::from_string("M U U U\nP P P P\n\nM M\nU PU").unwrap();
        let mut p = p.speedup(&mut EventHandler::null());
        p.compute_diagram(&mut EventHandler::null());
        p.sort_active_by_strength();
        assert_eq!(format!("{}", p), "A^2\nB C\n\nA BC^3\nAC C^3\n");
    }

    #[test]
    fn matching() {
        //let mut eh = EventHandler::with(|(x,a,b)|println!("{} {} {}",x,a,b));
        let mut eh = EventHandler::null();
        let eh = &mut eh;
        let p0 = Problem::from_string("M U U U\nP P P P\n\nM UP UP UP\nU U U U").unwrap();
        let mut v = vec![p0];
        for i in 0..7 {
            v.push(v[i].speedup(eh));
        }
        v[6].compute_triviality(eh);
        v[7].compute_triviality(eh);
        assert!(
            v[6].trivial_sets.as_ref().unwrap().is_empty()
                && !v[7].trivial_sets.as_ref().unwrap().is_empty()
        );
    }

    #[test]
    fn matching2() {
        //let mut eh = EventHandler::with(|(x,a,b)|println!("{} {} {}",x,a,b));
        let mut eh = EventHandler::null();
        let eh = &mut eh;
        let mut p0 = Problem::from_string("M U U U\nP P P P\n\nM UP UP UP\nU U U U").unwrap();
        p0.discard_useless_stuff(true, eh);
        p0.compute_triviality(eh);
        p0.sort_active_by_strength();

        let mut v = vec![p0];
        for i in 0..7 {
            let mut r = v[i].speedup(eh);
            r.discard_useless_stuff(true, eh);
            r.compute_triviality(eh);
            r.sort_active_by_strength();
            v.push(r);
        }
        assert!(
            v[6].trivial_sets.as_ref().unwrap().is_empty()
                && !v[7].trivial_sets.as_ref().unwrap().is_empty()
        );
    }

    #[test]
    fn with_star() {
        //let mut eh = EventHandler::with(|(x,a,b)|println!("{} {} {}",x,a,b));
        let mut eh = EventHandler::null();
        let eh = &mut eh;
        let mut p0 = Problem::from_string("A AB*\n\nB AB*").unwrap();
        p0.discard_useless_stuff(true, eh);
        p0.compute_triviality(eh);
        p0.sort_active_by_strength();

        let mut v = vec![p0];
        for i in 0..2 {
            let mut r = v[i].speedup(eh);
            r.discard_useless_stuff(true, eh);
            r.compute_triviality(eh);
            r.sort_active_by_strength();
            v.push(r);
        }

        assert_eq!(v[2].to_string(), "A B*\n\nB AB*\n");
    }

    #[test]
    fn dont_discard_emptysets_with_exponent_zero() {
        let mut eh = EventHandler::null();
        let eh = &mut eh;
        let p = Problem::from_string("1 2\n\n12 1 1\n12 2 2").unwrap();
        let p = p.speedup(eh);
        assert_eq!(p.to_string(), "A^3\n\nA^2\n")
    }
}
