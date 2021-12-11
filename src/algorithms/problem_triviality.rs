use itertools::Itertools;

use crate::{
    group::{Group, GroupType},
    line::Line,
    part::Part,
    problem::Problem,
};

impl Problem {
    pub fn compute_triviality(&mut self) {
        if self.trivial_sets.is_some() {
            panic!("triviality has been computed already");
        }
        self.passive.maximize();

        let active_sets = self.active.minimal_sets_of_all_choices();

        let mut trivial_sets = vec![];

        for set in active_sets {
            let group = Group(set.into_iter().sorted().collect());
            let part = Part {
                gtype: GroupType::Star,
                group,
            };
            let mut line = Line { parts: vec![part] };
            if self.passive.includes(&line) {
                trivial_sets.push(std::mem::take(&mut line.parts[0].group.0));
            }
        }

        self.trivial_sets = Some(trivial_sets);
    }
}

#[cfg(test)]
mod tests {

    use crate::problem::Problem;

    #[test]
    fn triviality() {
        let mut p = Problem::from_string("M U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_triviality();
        assert!(p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A AB AB\n\nA A\nB B").unwrap();
        p.compute_triviality();
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B\nAB AB").unwrap();
        p.compute_triviality();
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B").unwrap();
        p.compute_triviality();
        assert!(!p.trivial_sets.unwrap().is_empty());
    }
}
