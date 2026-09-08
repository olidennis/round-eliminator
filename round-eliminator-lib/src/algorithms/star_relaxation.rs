use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    constraint::Constraint,
    group::{Group, GroupType, Label},
    line::{Degree, Line},
    part::Part,
};

use super::event::EventHandler;

impl Constraint {
    /// Replace this constraint by its maximal one-coordinate-star relaxation.
    ///
    /// For every valid atomic configuration `c`, position `i` is replaced by
    /// the set of all labels that keep `c` valid when only position `i` is
    /// changed.  The resulting configuration of sets contains the complete
    /// Hamming-distance-one star around `c`.  Finally, dominated
    /// configurations are discarded.
    pub fn maximize_with_star_relaxation(
        &mut self,
        eh: &mut EventHandler,
    ) -> Result<(), &'static str> {
        if self.degree == Degree::Star {
            return Err("Speedup with star relaxation requires a finite passive degree.");
        }

        let degree = self.finite_degree();
        let alphabet = self.labels_appearing().into_iter().sorted().collect_vec();
        let valid_configurations = self
            .all_choices(false)
            .into_iter()
            .map(|line| atomic_labels(&line))
            .unique()
            .sorted()
            .collect_vec();
        let valid_configuration_set = valid_configurations.iter().cloned().collect::<HashSet<_>>();

        let mut relaxed = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.degree,
        };

        // This also contains the ordinary forall result: choose any atomic
        // center from a valid forall box.  Every set in that box is contained
        // in the corresponding one-coordinate replacement set of the center.
        // Hence, after discarding dominated lines, it is enough to generate
        // these stars rather than separately generating the forall boxes.
        for (configuration_index, center) in valid_configurations.iter().enumerate() {
            eh.notify(
                "computing one-coordinate stars",
                configuration_index,
                valid_configurations.len(),
            );

            debug_assert_eq!(center.len(), degree);
            let mut parts = Vec::with_capacity(degree);
            for position in 0..degree {
                let replacement_labels = alphabet
                    .iter()
                    .copied()
                    .filter(|replacement| {
                        let mut candidate = center.clone();
                        candidate[position] = *replacement;
                        candidate.sort_unstable();
                        valid_configuration_set.contains(&candidate)
                    })
                    .collect_vec();

                debug_assert!(!replacement_labels.is_empty());
                parts.push(Part {
                    group: Group::from(replacement_labels),
                    gtype: GroupType::ONE,
                });
            }

            let mut star = Line { parts };
            star.normalize();
            relaxed.add_line_and_discard_non_maximal(star);
        }

        relaxed.is_maximized = true;
        *self = relaxed;
        Ok(())
    }
}

fn atomic_labels(line: &Line) -> Vec<Label> {
    let mut labels = line
        .parts
        .iter()
        .flat_map(|part| std::iter::repeat(part.group.first()).take(part.gtype.value()))
        .collect_vec();
    labels.sort_unstable();
    labels
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn degree_two_matches_the_counterexample_construction() {
        let mut problem = Problem::from_string("A B C\n\nA B\nA C\nB B").unwrap();
        problem
            .passive
            .maximize_with_star_relaxation(&mut EventHandler::null())
            .unwrap();

        let mapping = problem.mapping_label_text.iter().cloned().collect();
        let lines = problem
            .passive
            .lines
            .iter()
            .map(|line| line.to_string(&mapping))
            .sorted()
            .collect_vec();

        assert_eq!(lines, vec!["AB BC", "AB^2"]);
    }

    #[test]
    fn star_relaxation_contains_the_radius_one_star_in_degree_three() {
        let mut problem = Problem::from_string("A A\n\nA A A\nA A B\nA B B").unwrap();
        problem
            .passive
            .maximize_with_star_relaxation(&mut EventHandler::null())
            .unwrap();

        let mapping = problem.mapping_label_text.iter().cloned().collect();
        let lines = problem
            .passive
            .lines
            .iter()
            .map(|line| line.to_string(&mapping))
            .sorted()
            .collect_vec();

        assert_eq!(lines, vec!["AB^3"]);
    }

    #[test]
    fn rejects_variable_degree_constraints() {
        let mut problem = Problem::from_string("A*\n\nA*").unwrap();
        assert!(problem
            .passive
            .maximize_with_star_relaxation(&mut EventHandler::null())
            .is_err());
    }
}
