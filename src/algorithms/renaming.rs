use std::collections::{HashSet, HashMap};

use itertools::Itertools;

use crate::problem::Problem;

impl Problem {
    pub fn rename(&mut self, v: &[(usize, String)]) -> Result<(), &'static str> {
        let given_labels: HashSet<usize> = v.iter().map(|(l, _)| *l).unique().collect();
        let labels: HashSet<usize> = self
            .mapping_label_text
            .iter()
            .map(|(l, _)| *l)
            .unique()
            .collect();

        if labels != given_labels || given_labels.len() != v.len() {
            return Err("There is something wrong with the given renaming");
        }

        if v.iter().map(|(_, s)| s).unique().count() != labels.len() {
            return Err("Labels are not unique");
        }

        let mut renaming = vec![];

        for (l, s) in v {
            if s.chars().any(|c| "()*^ ".contains(c)) {
                return Err("Some label contains characters that are not allowed");
            }
            if s.len() == 1 {
                renaming.push((*l, s.to_string()));
            } else {
                renaming.push((*l, format!("({})", s)));
            }
        }

        self.mapping_label_text = renaming;

        Ok(())
    }

    pub fn mapping_label_generators(&self) -> Vec<(usize,Vec<usize>)> {
        if self.mapping_label_oldlabels.is_none() {
            panic!("mapping label generators requires that the current problem is the result of a speedup");
        }

        let map_label_oldset = self.mapping_label_oldlabels.clone().unwrap();
        let oldsets : Vec<_> = map_label_oldset.iter().map(|(_,o)|o.clone()).collect();

        let mut result = vec![];

        if self.diagram_indirect_old.is_some() {
            let oldreach = self.diagram_indirect_old_to_reachability_adj();
            for (l,o) in &map_label_oldset {
                let mut gen = o.clone();
                for label in o {
                    gen.retain(|x|{
                        x == label ||
                        !oldreach[label].contains(x) ||
                        oldreach[x].contains(label)
                    });
                }
                gen.sort();
                result.push((*l,gen));
            }
        } else {
            for (l,o) in &map_label_oldset {
                let mut gen = vec![];
                for contained in o {
                    let mut containing : Vec<_> = oldsets.iter().filter(|old|old.contains(contained)).cloned().collect();
                    containing.sort_by_key(|set|set.len());
                    let minsize = containing[0].len();
                    if minsize == o.len() {
                        gen.push(*contained);
                    }
                }
                gen.sort();
                if !gen.is_empty() {
                    result.push((*l,gen));
                } else {
                    result.push((*l,o.clone()));
                }
            }
        }

        result.sort();

        result
    }

    pub fn rename_by_generators(&mut self) {
        let map_label_oldlabels = self.mapping_label_generators();
        let map_oldlabels_text = self.mapping_oldlabel_text.as_ref().expect("rename by generators requires that the current problem is the result of a speedup");
        let map_oldlabels_text : HashMap<_,_> = map_oldlabels_text.iter().cloned().collect();
        let renaming : Vec<_> = map_label_oldlabels.into_iter().map(|(label,oldset)|{
            if oldset.len() == 1 {
                (label,map_oldlabels_text[&oldset[0]].clone())
            } else {
                (label,format!("<{}>",oldset.iter().map(|x|&map_oldlabels_text[x]).join(",")))
            }
        }).collect();
        self.rename(&renaming).unwrap();
    }

}

#[cfg(test)]
mod tests {

    use crate::problem::Problem;

    #[test]
    fn renaming() {
        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.rename(&[(0, "B".into()), (1, "A".into())]).unwrap();
        assert_eq!(format!("{}", p), "B BA^2\n\nBA A\n");

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.rename(&[(1, "ABC".into()), (0, "TEST".into())]).unwrap();
        assert_eq!(
            format!("{}", p),
            "(TEST) (TEST)(ABC)^2\n\n(TEST)(ABC) (ABC)\n"
        );

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST".into()), (0, "ABC".into())]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST".into())]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST".into()), (1, "TEST".into())]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A".into()), (1, "B*".into())]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A".into()), (1, "(B)".into())]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A".into()), (1, "B ".into())]).is_err());
    }

    #[test]
    fn renaming_by_generators(){
        let mut p = Problem::from_string("A D D D\nB B B C\n\nAC	BCD	BCD	BCD\nD	D	D	D").unwrap();
        p.mapping_label_oldlabels = Some(vec![(0,vec![0]),(2,vec![1]),(3,vec![0,1]),(1,vec![1,2])]);
        p.diagram_indirect_old = Some(vec![(0,0),(1,1),(2,2),(2,1)]);
        p.mapping_oldlabel_text = Some(vec![(0,"M".into()),(1,"U".into()),(2,"P".into())]);
        p.rename_by_generators();
        assert_eq!(format!("{}",p),"M P^3\n(<M,U>) U^3\n\nM(<M,U>) PU(<M,U>)^3\nP^4\n");

        let mut p = Problem::from_string("A D D D\nB B B C\n\nAC	BCD	BCD	BCD\nD	D	D	D").unwrap();
        p.mapping_label_oldlabels = Some(vec![(0,vec![0]),(2,vec![1]),(3,vec![0,1]),(1,vec![1,2])]);
        p.mapping_oldlabel_text = Some(vec![(0,"M".into()),(1,"U".into()),(2,"P".into())]);
        p.rename_by_generators();
        assert_eq!(format!("{}",p),"M P^3\n(<M,U>) U^3\n\nM(<M,U>) PU(<M,U>)^3\nP^4\n");
    }
}
