use itertools::Itertools;

use crate::{
    group::{Group, GroupType, Label},
    line::Line,
};

impl Line {
    pub fn includes(&self, other: &Line) -> bool {
        self.includes_with_custom_supersets(other, None::<fn(&'_ _, &'_ _) -> _>)
    }

    #[inline(never)]
    pub fn includes_with_custom_supersets<T>(&self, other: &Line, is_superset: Option<T>) -> bool
    where
        T: Fn(&Group, &Group) -> bool,
    {
        self.matches(other,is_superset).is_some()
    }

    pub fn pick_existing_choice(&self, other: &Line) -> Option<Vec<Label>> {
        let v = self.matches(other, 
            Some(|g1 : &Group,g2 : &Group| { !g1.intersection(&g2).is_empty()})
        )?;
        let mut labels = vec![];
        for ((i,j),r) in v {
            let label = self.parts[i].group.intersection(&other.parts[j].group).first();
            for _ in 0..r {
                labels.push(label);
            }
        }
        Some(labels)
    }





    pub fn matches<T>(&self, other: &Line, can_match: Option<T>) -> Option<Vec<((usize,usize),usize)>>
        where
            T: Fn(&Group, &Group) -> bool,
        {

        let d1 = self.parts.len();
        let d2 = other.parts.len();
        let t1 = self.degree_without_star();
        let t2 = other.degree_without_star();
        let maxflow = if self.has_star() { t1 + t2 + 1 } else { t1 };

        let n = 2 + d1 + d2;
        let mut g = contest_algorithms::graph::flow::FlowGraph::new(n, d1 * d2);

        let mut edges = vec![];

        //6% of runtime
        for i in 0..d1 {
            let value = if let GroupType::Star = self.parts[i].gtype {
                maxflow - t1
            } else {
                self.parts[i].gtype.value()
            };
            g.add_edge(0, 1 + i, value as i64, 0, 0);
            edges.push((0, 1 + i));
            edges.push((1 + i,0));
        }

        //5% of runtime
        for i in 0..d2 {
            let value = if let GroupType::Star = other.parts[i].gtype {
                maxflow - t2
            } else {
                other.parts[i].gtype.value()
            };
            g.add_edge(1 + d1 + i, n - 1, value as i64, 0, 0);
            edges.push((1 + d1 + i, n - 1));
            edges.push((n - 1, 1 + d1 + i));
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

        //50% of runtime
        for i in 0..d1 {
            let g1 = &self.parts[i].group;
            let mut at_least_one = false;
            for j in 0..d2 {
                let g2 = &other.parts[j].group;

                let is_superset = match can_match.as_ref() {
                    None => g1.is_superset(g2),
                    Some(f) => f(g1, g2),
                };
                if is_superset {
                    at_least_one = true;
                    g.add_edge(1 + i, 1 + d1 + j, maxflow as i64, 0, 0);
                    edges.push((1 + i, 1 + d1 + j));
                    edges.push((1 + d1 + j,1 + i));
                }
            }
            if !at_least_one && self.parts[i].gtype != GroupType::Many(0) {
                return None;
            }
        }

        //32% of runtime
        let (flowvalue, flow) = g.dinic(0, n - 1);

        if flowvalue == maxflow as i64 {
            let matching = flow.into_iter().enumerate()
                .filter(|&(_, f)| f > 0)
                .map(|(e, f)| (edges[e],f))
                .filter(|&((u, v),_f)| u < v && u != 0 && v != n-1)
                .sorted()
                .map(|((u,v),f)|((u-1,v-d1-1),f as usize))
                .collect::<Vec<_>>();
            Some(matching)
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::{line::Line, group::{Group, GroupType}, part::Part};



    #[test]
    fn exists() {
        let line1 = Line{ parts : 
            vec![
                Part{ gtype : GroupType::Many(5), group : Group::from(vec![0,1,2]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![1]) },
            ] };
        let line2 = Line{ parts : 
            vec![
                Part{ gtype : GroupType::Many(5), group : Group::from(vec![0,2]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![0,1]) },
                Part{ gtype : GroupType::Many(1), group : Group::from(vec![1,2]) },
                Part{ gtype : GroupType::Many(1), group : Group::from(vec![1,2]) },
            ] };
        assert!(line1.pick_existing_choice(&line2) == Some(vec![0,0,0,0,0,0,0,1,1]));
        assert!(line2.pick_existing_choice(&line1).is_some());


        let line1 = Line{ parts : 
            vec![
                Part{ gtype : GroupType::Many(5), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![0]) },
            ] };
        let line2 = Line{ parts : 
            vec![
                Part{ gtype : GroupType::Many(5), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(2), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(1), group : Group::from(vec![0]) },
                Part{ gtype : GroupType::Many(1), group : Group::from(vec![1]) },
            ] };
        assert!(line1.pick_existing_choice(&line2).is_none());
        assert!(line2.pick_existing_choice(&line1).is_none());
    }
}