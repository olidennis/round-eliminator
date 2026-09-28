use itertools::Itertools;

use crate::{line::Degree, problem::Problem};

use super::{
    coloring_sat::{cardinality_network, SatEngine},
    edge_coloring_solvability::passive_allows,
    event::EventHandler,
};

impl Problem {
    /// Largest strong input coloring that makes the problem zero-round
    /// solvable. Each SAT variable selects one active color behavior; every
    /// incompatible passive palette becomes a clause excluding that palette.
    pub fn compute_coloring_solvability(&mut self, eh: &mut EventHandler) {
        if self.coloring_sets.is_some() {
            panic!("coloring solvability has been computed already");
        }
        let passive_degree = match self.passive.degree {
            Degree::Finite(degree) if degree > 0 => degree,
            _ => panic!("coloring solvability requires finite positive passive degree"),
        };
        let active_sets: Vec<_> = self
            .active
            .minimal_sets_of_all_choices()
            .into_iter()
            .map(|set| set.into_iter().sorted().collect::<Vec<_>>())
            .collect();
        // At a passive node all colors are different. A behavior can appear
        // at most (passive_degree - 1) times unless a compatible full tuple
        // of that behavior exists. These copies reproduce the old search's
        // finite multiset of candidate colors, without enumerating cliques.
        let candidates: Vec<_> = active_sets
            .iter()
            .flat_map(|set| std::iter::repeat(set.clone()).take(passive_degree - 1))
            .collect();
        if candidates.len() < passive_degree {
            self.coloring_sets = Some(Vec::new());
            return;
        }

        let mut sat = SatEngine::new();
        let chosen: Vec<_> = (0..candidates.len()).map(|_| sat.variable()).collect();
        // Equal copies are interchangeable. Selecting a later one must select
        // every earlier copy of the same behavior.
        for group in 0..active_sets.len() {
            for copy in 1..passive_degree - 1 {
                let current = group * (passive_degree - 1) + copy;
                sat.clause([-chosen[current], chosen[current - 1]]).unwrap();
            }
        }

        if passive_degree == 2 {
            for i in 0..candidates.len() {
                eh.notify("coloring SAT constraints", i, candidates.len());
                for j in i + 1..candidates.len() {
                    if !passive_allows(self, &[&candidates[i], &candidates[j]]) {
                        sat.clause([-chosen[i], -chosen[j]]).unwrap();
                    }
                }
            }
        }

        let at_least = cardinality_network(&mut sat, &chosen).unwrap();
        let mut low = passive_degree - 1;
        let mut high = candidates.len();
        let mut best = Vec::new();
        while low < high {
            let target = low + (high - low + 1) / 2;
            eh.notify("coloring SAT search", target, candidates.len());
            let compatible = loop {
                let Some(model) = sat.solve_assuming(&[at_least[target - 1]]).unwrap() else {
                    break None;
                };
                let selected: Vec<_> = chosen
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &lit)| model[(lit - 1) as usize].then_some(i))
                    .collect();
                if passive_degree == 2 {
                    break Some(selected);
                }
                // Hypergraph constraints are generated lazily. This avoids
                // materializing all degree-sized subsets of the candidate
                // behaviors, most of which never matter to the optimum.
                let invalid =
                    selected
                        .iter()
                        .copied()
                        .combinations(passive_degree)
                        .find(|palette| {
                            let sets: Vec<_> = palette.iter().map(|&i| &candidates[i]).collect();
                            !passive_allows(self, &sets)
                        });
                match invalid {
                    Some(palette) => sat.clause(palette.into_iter().map(|i| -chosen[i])).unwrap(),
                    None => break Some(selected),
                }
            };
            if let Some(solution) = compatible {
                low = target;
                best = solution;
            } else {
                high = target - 1;
            }
        }
        let mut coloring_sets: Vec<_> = best.into_iter().map(|i| candidates[i].clone()).collect();
        coloring_sets.sort();
        self.coloring_sets = Some(coloring_sets);
    }

    pub fn compute_hypergraph_coloring_solvability(&mut self, eh: &mut EventHandler) {
        self.compute_coloring_solvability(eh);
    }
}

#[cfg(test)]
mod tests {
    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn coloring() {
        let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets.as_ref().unwrap().len(), 3);

        let mut p = Problem::from_string("A A A\nB B B\nC C C\nD D D\n\nA BC\nB C\nD A").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets.as_ref().unwrap().len(), 3);

        let mut p = Problem::from_string("A A A\nB B B\nC C D\nE E E\n\nA BCD\nB CD\nE A").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets.as_ref().unwrap().len(), 3);

        let mut p = Problem::from_string("A AB AB\n\nA B").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert!(p.coloring_sets.unwrap().len() < 2);
    }

    #[test]
    fn hypergraph_strong_coloring_uses_all_passive_ports() {
        let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA B C").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets.unwrap().len(), 3);
    }
}
