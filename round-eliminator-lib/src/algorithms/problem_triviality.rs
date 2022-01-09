use itertools::Itertools;

use crate::{
    group::{Group, GroupType},
    line::{Degree, Line},
    part::Part,
    problem::Problem,
};

use super::event::EventHandler;

impl Problem {
    pub fn compute_triviality(&mut self, eh: &mut EventHandler) {
        if self.trivial_sets.is_some() {
            panic!("triviality has been computed already");
        }

        self.passive.maximize(eh);

        if self.passive.lines.is_empty() {
            self.trivial_sets = Some(vec![]);
            return;
        }

        let passive_degree = match self.passive.lines[0].degree() {
            Degree::Finite(x) => GroupType::Many(x),
            Degree::Star => GroupType::Star,
        };

        let active_sets = self.active.minimal_sets_of_all_choices();

        let mut trivial_sets = vec![];
        let num_active_sets = active_sets.len();

        for (i, set) in active_sets.into_iter().enumerate() {
            eh.notify("triviality", i, num_active_sets);

            let group = Group(set.into_iter().sorted().collect());
            let part = Part {
                gtype: passive_degree,
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

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn triviality() {
        let mut p = Problem::from_string("M U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A AB AB\n\nA A\nB B").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B\nAB AB").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());

        let mut p = Problem::from_string("A B AB\n\nA A\nB B\nA B").unwrap();
        p.compute_triviality(&mut EventHandler::null());
        assert!(!p.trivial_sets.unwrap().is_empty());
    }
}
