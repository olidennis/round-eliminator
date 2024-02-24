use std::collections::HashSet;

use itertools::Itertools;
use streaming_iterator::StreamingIterator;

use crate::{
    constraint::Constraint,
    group::{Group, GroupType, Label, Exponent},
    line::{Degree, Line},
    part::Part,
    problem::Problem,
};

use super::{event::EventHandler, max_clique::Graph, multisets_pairing::Comb};

impl Problem {}

impl Constraint {
    pub fn minimal_splits(&self, outdegree: usize) -> Vec<(Group, Group)> {
        let mut result = HashSet::new();

        for line in &self.lines {
            let v = line
                .parts
                .iter()
                .map(|p| match p.gtype {
                    GroupType::Many(x) => std::cmp::min(x as usize, outdegree),
                    GroupType::Star => outdegree,
                })
                .collect();

            let mut combs = Comb::new(outdegree, v);
            while let Some(comb) = combs.next() {
                let mut outgoing = Line {
                    parts: line
                        .parts
                        .iter()
                        .zip(comb.iter())
                        .map(|(part, count)| Part {
                            group: part.group.clone(),
                            gtype: GroupType::Many(*count as Exponent),
                        })
                        .collect(),
                };
                let mut incoming = Line {
                    parts: line
                        .parts
                        .iter()
                        .zip(comb.iter())
                        .map(|(part, count)| Part {
                            group: part.group.clone(),
                            gtype: match part.gtype {
                                GroupType::Many(x) => GroupType::Many(x - *count as crate::group::Exponent),
                                GroupType::Star => GroupType::Star,
                            },
                        })
                        .collect(),
                };

                incoming.normalize();
                outgoing.normalize();

                let v_incoming = incoming.minimal_sets_of_all_choices();
                let v_outgoing = outgoing.minimal_sets_of_all_choices();

                for outgoing in &v_outgoing {
                    let g_out = Group(outgoing.iter().cloned().sorted().collect());
                    for incoming in &v_incoming {
                        let g_in = Group(incoming.iter().cloned().sorted().collect());
                        result.insert((g_out.clone(), g_in));
                    }
                }
            }
        }

        result.into_iter().collect()
    }
}

impl Problem {
    pub fn compute_triviality_given_orientation(
        &mut self,
        outdegree: usize,
        eh: &mut EventHandler,
    ) {
        if self.orientation_trivial_sets.is_some() {
            panic!("triviality has been computed already");
        }
        if self.passive.degree != Degree::Finite(2) {
            panic!("cannot compute solvability given orientation if the passive side has degree different from 2");
        }

        //self.passive.maximize(eh);

        let splits = self.active.minimal_splits(outdegree);

        let mut trivial_sets = vec![];
        let num_splits = splits.len();

        for (i, (outgoing, incoming)) in splits.into_iter().enumerate() {
            eh.notify("orientationtriviality", i, num_splits);
            let p1 = Part {
                gtype: GroupType::ONE,
                group: outgoing,
            };
            let p2 = Part {
                gtype: GroupType::ONE,
                group: incoming,
            };
            let mut line = Line {
                parts: vec![p1, p2],
            };
            if self.passive.includes(&line) {
                trivial_sets.push((
                    std::mem::take(&mut line.parts[0].group.0),
                    std::mem::take(&mut line.parts[1].group.0),
                ));
            }
        }

        self.orientation_trivial_sets = Some(trivial_sets);
    }

    pub fn compute_coloring_solvability_given_orientation(
        &mut self,
        outdegree: usize,
        eh: &mut EventHandler,
    ) {
        if self.passive.degree != Degree::Finite(2) {
            panic!("cannot compute coloring solvability given orientation if the passive side has degree different from 2");
        }
        if self.orientation_coloring_sets.is_some() {
            panic!("coloring solvability has been computed already");
        }

        //self.passive.maximize(eh);

        let splits = self.active.minimal_splits(outdegree);

        let mut edges = vec![];

        for (i, (outgoing_1, incoming_1)) in splits.iter().enumerate() {
            for (j, (outgoing_2, incoming_2)) in splits.iter().enumerate() {
                eh.notify(
                    "coloring graph",
                    splits.len() * i + j,
                    splits.len() * splits.len(),
                );
                if i < j {
                    let part_o_1 = Part {
                        gtype: GroupType::ONE,
                        group: outgoing_1.clone(),
                    };
                    let part_o_2 = Part {
                        gtype: GroupType::ONE,
                        group: outgoing_2.clone(),
                    };
                    let part_i_1 = Part {
                        gtype: GroupType::ONE,
                        group: incoming_1.clone(),
                    };
                    let part_i_2 = Part {
                        gtype: GroupType::ONE,
                        group: incoming_2.clone(),
                    };
                    let line1 = Line {
                        parts: vec![part_o_1, part_i_2],
                    };
                    let line2 = Line {
                        parts: vec![part_o_2, part_i_1],
                    };

                    if self.passive.includes(&line1) && self.passive.includes(&line2) {
                        edges.push((i, j));
                    }
                }
            }
        }

        if edges.is_empty() {
            self.orientation_coloring_sets = Some(vec![]);
            return;
        }

        let n = splits.len();
        let mut adj = vec![vec![]; n];
        for (a, b) in edges {
            adj[a].push(b);
            adj[b].push(a);
        }

        let g = Graph::from_adj(adj);
        eh.notify("clique", 1, 1);
        let mut coloring_sets: Vec<(Vec<Label>, Vec<Label>)> = g
            .max_clique()
            .into_iter()
            .map(|x| (splits[x].0 .0.clone(), splits[x].1 .0.clone()))
            .collect();
        coloring_sets.sort();

        self.orientation_coloring_sets = Some(coloring_sets);
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn orientation_triviality() {
        let mut p = Problem::from_string("A A B B B\n\nA B").unwrap();
        p.compute_triviality_given_orientation(2, &mut EventHandler::null());
        assert!(p.orientation_trivial_sets.unwrap().len() > 0);

        let mut p = Problem::from_string("A A AB AB AB\n\nA B").unwrap();
        p.compute_triviality_given_orientation(2, &mut EventHandler::null());
        assert!(p.orientation_trivial_sets.unwrap().len() > 0);

        let mut p = Problem::from_string("A A B B\n\nA B").unwrap();
        p.compute_triviality_given_orientation(3, &mut EventHandler::null());
        assert!(p.orientation_trivial_sets.unwrap().len() == 0);
    }

    #[test]
    fn orientation_coloring() {
        let mut p = Problem::from_string("A A B B B\nC C D D D\n\nA D\nB C").unwrap();
        p.compute_coloring_solvability_given_orientation(2, &mut EventHandler::null());
        assert!(p.orientation_coloring_sets.unwrap().len() == 2);

        let mut p = Problem::from_string("A A A B B B\nC C C D D D\n\nA D\nB C").unwrap();
        p.compute_coloring_solvability_given_orientation(2, &mut EventHandler::null());
        assert!(p.orientation_coloring_sets.unwrap().len() < 2);
    }
}
