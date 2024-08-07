use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    algorithms::max_clique::Graph,
    group::{Group, GroupType},
    line::{Degree, Line},
    part::Part,
    problem::Problem,
};

use super::{event::EventHandler, max_clique::HyperGraph};

impl Problem {
    /// Computes the number of independent actions. If that number is x, then given an x coloring it is possible to solve the problem in 0 rounds.
    pub fn compute_coloring_solvability(&mut self, eh: &mut EventHandler) {
        if self.passive.degree != Degree::Finite(2) {
            return self.compute_hypergraph_coloring_solvability(eh);
            //panic!("cannot compute coloring solvability if the passive side has degree different from 2");
        }
        if self.coloring_sets.is_some() {
            panic!("coloring solvability has been computed already");
        }
        //self.passive.maximize(eh);

        let active_sets = self.active.minimal_sets_of_all_choices();

        let mut edges = vec![];

        for (i, set1) in active_sets.iter().enumerate() {
            for (j, set2) in active_sets.iter().enumerate() {
                eh.notify(
                    "coloring graph",
                    active_sets.len() * i + j,
                    active_sets.len() * active_sets.len(),
                );
                if i < j {
                    let group1 = Group::from(set1.iter().cloned().sorted().collect());
                    let group2 = Group::from(set2.iter().cloned().sorted().collect());
                    let part1 = Part {
                        gtype: GroupType::ONE,
                        group: group1,
                    };
                    let part2 = Part {
                        gtype: GroupType::ONE,
                        group: group2,
                    };

                    let line = Line {
                        parts: vec![part1, part2],
                    };
                    if self.passive.includes(&line) {
                        edges.push((i, j));
                    }
                }
            }
        }

        if edges.is_empty() {
            self.coloring_sets = Some(vec![]);
            return;
        }

        let n = active_sets.len();
        let mut adj = vec![vec![]; n];
        for (a, b) in edges {
            adj[a].push(b);
            adj[b].push(a);
        }

        let g = Graph::from_adj(adj);
        eh.notify("clique", 1, 1);
        let mut coloring_sets: Vec<_> = g
            .max_clique()
            .into_iter()
            .map(|x| active_sets[x].iter().cloned().sorted().collect())
            .collect();
        coloring_sets.sort();
        self.coloring_sets = Some(coloring_sets);
    }

    pub fn compute_hypergraph_coloring_solvability(&mut self, eh: &mut EventHandler) {
        if self.coloring_sets.is_some() {
            panic!("coloring solvability has been computed already");
        }

        self.passive.maximize(eh);

        let active_sets : Vec<_> = self.active.minimal_sets_of_all_choices().into_iter().flat_map(|set|std::iter::repeat(set).take(self.passive.finite_degree()-1)).enumerate().collect();

        let mut hyperedges = vec![];
        let len = active_sets.iter().combinations(self.passive.finite_degree()).count();
        for (i,sets) in active_sets.iter().combinations(self.passive.finite_degree()).enumerate() {
            eh.notify(
                "coloring graph",
                i,
                len,
            );
            let mut line = Line { parts : vec![]};
            for set in &sets {
                let group = Group::from(set.1.iter().cloned().sorted().collect());
                let part = Part {
                    gtype: GroupType::ONE,
                    group: group,
                };
                line.parts.push(part);
            }

            if self.passive.includes(&line) {
                hyperedges.push(sets.iter().map(|x|x.0).collect());
            }
        }

        if hyperedges.is_empty() {
            self.coloring_sets = Some(vec![]);
            return;
        }

        let active_sets : HashMap<_,_> = active_sets.into_iter().collect();
        let g = HyperGraph::from_hyperedges(hyperedges);
        eh.notify("clique", 1, 1);
        let mut coloring_sets: Vec<_> = g
            .max_clique()
            .into_iter()
            .map(|x| active_sets[&x].iter().cloned().sorted().collect())
            .collect();
        coloring_sets.sort();
        self.coloring_sets = Some(coloring_sets);
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn coloring() {
        let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA BC\nB C").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets, Some(vec![vec![0], vec![1], vec![2]]));

        let mut p = Problem::from_string("A A A\nB B B\nC C C\nD D D\n\nA BC\nB C\nD A").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets, Some(vec![vec![0], vec![1], vec![2]]));

        let mut p = Problem::from_string("A A A\nB B B\nC C D\nE E E\n\nA BCD\nB CD\nE A").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert_eq!(p.coloring_sets, Some(vec![vec![0], vec![1], vec![2, 3]]));

        let mut p = Problem::from_string("A AB AB\n\nA B").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
        assert!(p.coloring_sets.unwrap().len() < 2);
    }

    /*#[test]
    #[should_panic]
    fn coloring_hypergraph() {
        let mut p = Problem::from_string("A A A\nB B B\nC C C\n\nA B C").unwrap();
        p.compute_coloring_solvability(&mut EventHandler::null());
    }*/
}
