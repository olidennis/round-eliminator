use itertools::Itertools;

use crate::{
    algorithms::mapping_problem::mapping_problem::MappingProblem, group::{Exponent, Group, GroupType}, line::{Degree, Line}, part::Part, problem::Problem
};

use super::event::EventHandler;

impl Problem {
    pub fn compute_triviality(&mut self, eh: &mut EventHandler) {
        if self.trivial_sets.is_some() {
            panic!("triviality has been computed already");
        }

        if self.passive.degree != Degree::Finite(2) {
            self.passive.maximize(eh);
        }

        if self.passive.lines.is_empty() {
            self.trivial_sets = Some(vec![]);
            return;
        }

        let passive_degree = match self.passive.lines[0].degree() {
            Degree::Finite(x) => GroupType::Many(x as Exponent),
            Degree::Star => GroupType::Star,
        };

        let active_sets = self.active.minimal_sets_of_all_choices();

        let mut trivial_sets = vec![];
        let num_active_sets = active_sets.len();

        for (i, set) in active_sets.into_iter().enumerate() {
            eh.notify("triviality", i, num_active_sets);

            let group = Group::from(set.into_iter().sorted().collect());
            let part = Part {
                gtype: passive_degree,
                group,
            };
            let mut line = Line { parts: vec![part] };
            if self.passive.includes(&line) {
                trivial_sets.push(line.parts[0].group.as_vec());
            }
        }

        self.trivial_sets = Some(trivial_sets);
    }

    pub fn compute_triviality_with_input(&mut self, other:Problem) {
        
        let mut mapping = MappingProblem::new(
            other.clone(),
            self.clone()
        );

        mapping.maximize_out_problem();

        if let Some(mapping) = mapping.search_for_mapping_parallel() {
            let other_label_to_text = other.mapping_label_text;


            let mapping : Vec<_> = mapping.into_iter().map(|(l,h)|{
                let v : Vec<_> = h.into_iter().sorted().collect();
                (l,v)
            }).collect();
            self.is_trivial_with_input = Some(true);
            self.triviality_with_input = Some((other_label_to_text,mapping));
        } else {
            self.is_trivial_with_input = Some(false);
        }
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
