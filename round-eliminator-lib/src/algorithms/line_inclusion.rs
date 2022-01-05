use std::collections::HashSet;

use crate::{group::{GroupType, Label, Group}, line::Line};

impl Line {
    pub fn includes(&self, other: &Line) -> bool {
        self.includes_with_custom_supersets(other, None::<fn(&'_ _, &'_ _) -> _>)
    }

    pub fn includes_with_custom_supersets<T>(&self, other: &Line, is_superset: Option<T>) -> bool
    where
        T: Fn(&Group, &Group) -> bool,
    {
        let d1 = self.parts.len();
        let d2 = other.parts.len();
        let t1 = self.degree_without_star();
        let t2 = other.degree_without_star();
        let maxflow = if self.has_star() { t1 + t2 + 1 } else { t1 };

        let n = 2 + d1 + d2;
        let mut g = contest_algorithms::graph::flow::FlowGraph::new(n, d1*d2);

        for i in 0..d1 {
            let value = if let GroupType::Star = self.parts[i].gtype {
                maxflow - t1
            } else {
                self.parts[i].gtype.value()
            };
            g.add_edge(0, 1 + i, value as i64, 0, 0);
        }

        for i in 0..d2 {
            let value = if let GroupType::Star = other.parts[i].gtype {
                maxflow - t2
            } else {
                other.parts[i].gtype.value()
            };
            g.add_edge(1 + d1 + i, n - 1, value as i64, 0, 0);
        }

        /*
        let h1: Vec<HashSet<_>> = self
            .parts
            .iter()
            .map(|x| x.group.iter().cloned().collect())
            .collect();
        let h2: Vec<HashSet<_>> = other
            .parts
            .iter()
            .map(|x| x.group.iter().cloned().collect())
            .collect();

        for i in 0..d1 {
            let g1 = &h1[i];
            for j in 0..d2 {
                let is_superset = match is_superset.as_ref() {
                    None => g1.is_superset(&h2[j]),
                    Some(f) => f(&g1, &h2[j]),
                };
                if is_superset {
                    g.add_edge(1 + i, 1 + d1 + j, maxflow as i64, 0, 0);
                }
            }
        }*/

        for i in 0..d1 {
            let g1 = &self.parts[i].group;
            for j in 0..d2 {
                let g2 = &other.parts[j].group;

                let is_superset = match is_superset.as_ref() {
                    None => g1.is_superset(&g2),
                    Some(f) => f(&g1, &g2),
                };
                if is_superset {
                    g.add_edge(1 + i, 1 + d1 + j, maxflow as i64, 0, 0);
                }
            }
        }

        g.dinic(0, n - 1).0 == maxflow as i64
    }
}
