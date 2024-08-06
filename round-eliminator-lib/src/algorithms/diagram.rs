use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use petgraph::graph::IndexType;
use rayon::iter::ParallelBridge;

use crate::{group::Label, line::Degree, problem::Problem};

use super::event::EventHandler;

impl Problem {

    pub fn compute_diagram_without_storing_maximized_passive(&mut self, eh: &mut EventHandler) {
        let mut p = self.clone();
        p.diagram_indirect = None;
        p.compute_diagram(eh);
        self.diagram_indirect = p.diagram_indirect;
        self.diagram_direct = p.diagram_direct;
    }

    pub fn compute_diagram(&mut self, eh: &mut EventHandler) {
        if self.diagram_indirect.is_some() {
            panic!("diagram has been computed already");
        }

        if self.passive.degree != Degree::Finite(2) {
            self.passive.maximize(eh);
        }


        #[cfg(not(target_arch = "wasm32"))]
        let diagram = {
            let diagram = crossbeam::scope(|s| {

                let labels: Vec<_> = self.labels();
                let (progress_tx, progress_rx) =  crossbeam_channel::unbounded();
        
                let slf = &self;
                s.spawn(move |_|{
                    use rayon::prelude::*;
                    labels.iter().cartesian_product(labels.iter())
                        .par_bridge()
                        .for_each(|(l1,l2)|{
                            if l1 == l2 || slf.passive.is_diagram_predecessor(*l1, *l2) {
                                progress_tx.send(Some((*l1, *l2))).unwrap();
                            } else {
                                progress_tx.send(None).unwrap();
                            }
                        });
                    drop(progress_tx);
                });

                let labels: Vec<_> = self.labels();
                let total = labels.len()*labels.len();
                let mut diagram = vec![];
                let mut received = 0;

                while let Ok(r) = progress_rx.recv() {
                    received += 1;
                    if let Some((l1,l2)) = r {
                        diagram.push((l1,l2));
                    }
                    eh.notify("diagram", received, total);
                }

                diagram.sort();

                diagram
            }).unwrap();
            diagram
        };

        #[cfg(target_arch = "wasm32")]
        let diagram = {
            let mut diagram = vec![];
            let labels: Vec<_> = self.labels();
            for (i, l1) in labels.iter().enumerate() {
                for (j, l2) in labels.iter().enumerate() {
                    eh.notify("diagram", i * labels.len() + j, labels.len() * labels.len());
                    if l1 == l2 || self.passive.is_diagram_predecessor(*l1, *l2) {
                        diagram.push((*l1, *l2));
                    }
                }
            }
            diagram
        };

        self.diagram_indirect = Some(diagram);
        self.compute_direct_diagram();
    }

    pub fn compute_partial_diagram(&mut self, eh: &mut EventHandler) {
        if self.diagram_indirect.is_some() {
            panic!("diagram has been computed already");
        }

        let labels: Vec<_> = self.labels();

        let mut diagram = vec![];

        for (i, l1) in labels.iter().enumerate() {
            for (j, l2) in labels.iter().enumerate() {
                eh.notify("diagram", i * labels.len() + j, labels.len() * labels.len());
                if l1 == l2 || self.passive.is_diagram_predecessor_partial(*l1, *l2) {
                    diagram.push((*l1, *l2));
                }
            }
        }

        self.diagram_indirect = Some(diagram);
        self.compute_direct_diagram();
    }

    pub fn compute_set_inclusion_diagram(&mut self) {
        if self.diagram_indirect.is_some() {
            panic!("diagram has been computed already");
        }

        let labels: Vec<_> = self.labels();
        let mapping : HashMap<Label,HashSet<Label>> = self.mapping_label_oldlabels.as_ref().expect("set inclusion diagram can only be computed if the current problem has been obtained through round elimination")
            .iter().map(|(label,set)|(*label,set.iter().cloned().collect())).collect();

        let mut diagram = vec![];

        for &l1 in &labels {
            for &l2 in &labels {
                if l1 == l2 || mapping[&l1].is_subset(&mapping[&l2]) {
                    diagram.push((l1, l2));
                }
            }
        }

        self.diagram_indirect = Some(diagram);
        self.compute_direct_diagram();
    }

    pub fn compute_direct_diagram(&mut self) {
        let diagram = self.diagram_indirect.as_ref().unwrap();
        let labels : Vec<_> = self.labels().into_iter().collect();

        self.diagram_direct = Some(compute_direct_diagram(&labels,diagram));
    }

    pub fn diagram_indirect_to_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect
            .as_ref()
            .expect("diagram required, but still not computed")
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_indirect_old_to_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect_old
            .as_ref()
            .expect("old diagram required")
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self
            .mapping_oldlabel_text
            .as_ref()
            .unwrap()
            .iter()
            .map(|(x, _)| *x)
        {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_indirect_to_inverse_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect
            .as_ref()
            .expect("diagram required, but still not computed")
        {
            h.entry(b).or_default().insert(a);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_direct_to_succ_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in &self
            .diagram_direct
            .as_ref()
            .expect("diagram required, but still not computed")
            .1
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_direct_to_pred_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in &self
            .diagram_direct
            .as_ref()
            .expect("diagram required, but still not computed")
            .1
        {
            h.entry(b).or_default().insert(a);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }


}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn diagram() {
        let mut p = Problem::from_string("M U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        assert_eq!(
            p.diagram_indirect,
            Some(vec![(0, 0), (1, 1), (2, 1), (2, 2)])
        );
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1]), (2, vec![2])], vec![(2, 1)]))
        );

        let mut p = Problem::from_string("M U U\nP P P\n\nM UP").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        assert_eq!(
            p.diagram_indirect,
            Some(vec![(0, 0), (1, 1), (1, 2), (2, 1), (2, 2)])
        );
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1, 2])], vec![]))
        );

        let mut p = Problem::from_string("A AB AB\n\nA A\nB B").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        assert_eq!(p.diagram_indirect, Some(vec![(0, 0), (1, 1)]));
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1])], vec![]))
        );

        let mut p = Problem::from_string("A B AB\n\nB AB").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        assert_eq!(p.diagram_indirect, Some(vec![(0, 0), (0, 1), (1, 1)]));
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1])], vec![(0, 1)]))
        );

        let mut p = Problem::from_string("0	1	1	1\n2	1	1	3\n4	4	4	5\n\n053 4513 4513 4513\n13 13 13 204513\n53 4513 4513 04513\n513 513 0513 4513\n513 513 513 04513").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        assert_eq!(
            p.diagram_indirect,
            Some(vec![
                (0, 0),
                (0, 3),
                (0, 5),
                (1, 1),
                (1, 3),
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3),
                (2, 4),
                (2, 5),
                (3, 3),
                (4, 1),
                (4, 3),
                (4, 4),
                (4, 5),
                (5, 3),
                (5, 5)
            ])
        );
        assert_eq!(
            p.diagram_direct,
            Some((
                vec![
                    (0, vec![0]),
                    (1, vec![1]),
                    (2, vec![2]),
                    (3, vec![3]),
                    (4, vec![4]),
                    (5, vec![5]),
                ],
                vec![(0, 5), (1, 3), (2, 0), (2, 4), (4, 1), (4, 5), (5, 3)]
            ))
        );

        let mut p = Problem::from_string("0	1	1	1\n2	1	1	3\n4	4	4	5\n\n053 4513 4513 4513\n13 13 13 204513\n53 4513 4513 04513\n513 513 0513 4513\n513 513 513 04513").unwrap();
        p.mapping_label_oldlabels = Some(vec![
            (0, vec![0, 2]),
            (1, vec![1, 2, 3]),
            (2, vec![2]),
            (3, vec![0, 1, 2, 3]),
            (4, vec![2, 3]),
            (5, vec![0, 2, 3]),
        ]);
        p.compute_set_inclusion_diagram();
        assert_eq!(
            p.diagram_indirect,
            Some(vec![
                (0, 0),
                (0, 3),
                (0, 5),
                (1, 1),
                (1, 3),
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3),
                (2, 4),
                (2, 5),
                (3, 3),
                (4, 1),
                (4, 3),
                (4, 4),
                (4, 5),
                (5, 3),
                (5, 5)
            ])
        );
        assert_eq!(
            p.diagram_direct,
            Some((
                vec![
                    (0, vec![0]),
                    (1, vec![1]),
                    (2, vec![2]),
                    (3, vec![3]),
                    (4, vec![4]),
                    (5, vec![5]),
                ],
                vec![(0, 5), (1, 3), (2, 0), (2, 4), (4, 1), (4, 5), (5, 3)]
            ))
        );
    }
}

pub fn diagram_indirect_to_reachability_adj(labels : &[Label], diagram : &Vec<(Label,Label)>) -> HashMap<Label, HashSet<Label>> {
    let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
    for &(a, b) in diagram
    {
        h.entry(a).or_default().insert(b);
    }
    for &(a,b) in diagram {
        h.entry(a).or_default();
        h.entry(b).or_default();
    }
    for &l in labels {
        h.entry(l).or_default();
    }
    h
}

pub fn diagram_to_indirect(labels : &[Label], diagram : &Vec<(Label,Label)>) -> Vec<(Label,Label)> {
    let mut r = vec![];
    let mut adj : HashMap::<Label,Vec<_>> = HashMap::new();
    for &(a,b) in diagram {
        adj.entry(a).or_default().push(b);
    }
    for &l in labels {
        adj.entry(l).or_default();
    }

    for &label in labels {
        let mut visited = HashSet::new();
        let mut v = vec![];
        r.push((label,label));
        v.push(label);
        visited.insert(label);
        while let Some(cur) = v.pop() {
            for &x in &adj[&cur] {
                if !visited.contains(&x) {
                    r.push((label,x));
                    v.push(x);
                    visited.insert(x);
                }
            }
        }
    }

    r
}

pub fn compute_direct_diagram(labels : &[Label], diagram_indirect : &Vec<(Label,Label)>) -> (Vec<(Label, Vec<Label>)>, Vec<(Label, Label)>){
    let diagram = diagram_indirect;
    let diagram_usize: Vec<_> = diagram_indirect
        .iter()
        .map(|(a, b)| (*a as usize, *b as usize))
        .collect();
    // We need to compute the transitive reduction of the diagram.
    // The algorithm for transitive reduction only works in DAGs.
    // We need to first compute the strongly connected components, that are equivalent labels,
    // replace each SCC with a single label, obtaining a DAG,
    // and then compute the transitive reduction

    let labels: HashSet<_> = labels.into_iter().collect();

    // compute SCC
    let g = petgraph::graph::DiGraph::<usize, (), usize>::from_edges(diagram_usize);
    let scc = petgraph::algo::kosaraju_scc(&g);
    let mut merged: Vec<_> = scc
        .into_iter()
        .map(|group| {
            let mut group: Vec<_> = group.into_iter().map(|x| x.index() as Label).collect();
            group.sort_unstable();
            (group[0] as Label, group)
        })
        // petgraph is adding nodes also for labels that are not present
        .filter(|(x, _)| labels.contains(&(*x as Label)))
        .collect();

    // compute renaming
    let mut rename = HashMap::new();
    for (name, group) in &merged {
        for label in group {
            rename.insert(label, name);
        }
    }

    // compute edges that are not self loops after merging
    let mut new_edges = vec![];
    for (a, b) in diagram {
        if rename[a] != rename[b] {
            new_edges.push((*rename[a] as Label, *rename[b] as Label))
        }
    }

    // create DAG
    let g = petgraph::graph::DiGraph::<Label, (), Label>::from_edges(new_edges);

    //compute transitive reduction
    let topo = petgraph::algo::toposort(&g, None).unwrap();

    let (topoadj, _revmap) =
        petgraph::algo::tred::dag_to_toposorted_adjacency_list::<_, usize>(&g, &topo);

    let (reduction, _closure) =
        petgraph::algo::tred::dag_transitive_reduction_closure(&topoadj);

    let mut edges: Vec<_> = reduction
        .edge_indices()
        .map(|e| reduction.edge_endpoints(e).unwrap())
        .map(|(u, v)| {
            (
                topo[u.index()].index() as Label,
                topo[v.index()].index() as Label,
            )
        })
        .unique()
        .collect();

    merged.sort_unstable();
    edges.sort_unstable();

    (merged,edges)
}