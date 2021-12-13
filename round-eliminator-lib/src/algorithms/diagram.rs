use std::collections::{HashMap, HashSet};

use petgraph::graph::IndexType;

use crate::problem::Problem;

use super::event::EventHandler;

impl Problem {
    pub fn compute_diagram(&mut self, eh: &EventHandler) {
        if self.diagram_indirect.is_some() {
            panic!("diagram has been computed already");
        }
        self.passive.maximize(eh);

        let labels: Vec<_> = self.labels();

        let mut diagram = vec![];

        for (i,l1) in labels.iter().enumerate() {
            for (j,l2) in labels.iter().enumerate() {
                eh.notify("diagram",i*labels.len()+j,labels.len()*labels.len());
                if l1 == l2 || self.passive.is_diagram_predecessor(*l1, *l2) {
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
        let mapping : HashMap<usize,HashSet<usize>> = self.mapping_label_oldlabels.as_ref().expect("set inclusion diagram can only be computed if the current problem has been obtained through round elimination")
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
        // We need to compute the transitive reduction of the diagram.
        // The algorithm for transitive reduction only works in DAGs.
        // We need to first compute the strongly connected components, that are equivalent labels,
        // replace each SCC with a single label, obtaining a DAG,
        // and then compute the transitive reduction

        // compute SCC
        let g = petgraph::graph::DiGraph::<usize, (), usize>::from_edges(diagram);
        let scc = petgraph::algo::kosaraju_scc(&g);
        let mut merged: Vec<_> = scc
            .into_iter()
            .map(|group| {
                let mut group: Vec<_> = group.into_iter().map(|x| x.index()).collect();
                group.sort();
                (group[0], group)
            })
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
                new_edges.push((*rename[a], *rename[b]))
            }
        }

        // create DAG
        let g = petgraph::graph::DiGraph::<usize, (), usize>::from_edges(new_edges);

        //compute transitive reduction
        let topo = petgraph::algo::toposort(&g, None).unwrap();
        let (topoadj, _revmap) =
            petgraph::algo::tred::dag_to_toposorted_adjacency_list::<_, usize>(&g, &topo);
        let (reduction, _closure) =
            petgraph::algo::tred::dag_transitive_reduction_closure(&topoadj);

        let mut edges: Vec<_> = reduction
            .edge_indices()
            .map(|e| reduction.edge_endpoints(e).unwrap())
            .map(|(u, v)| (topo[u.index()].index(), topo[v.index()].index()))
            .collect();

        merged.sort();
        edges.sort();

        self.diagram_direct = Some((merged, edges));
    }
}

#[cfg(test)]
mod tests {

    use crate::{problem::Problem, algorithms::event::EventHandler};

    #[test]
    fn diagram() {
        let mut p = Problem::from_string("M U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_diagram(&EventHandler::null());
        assert_eq!(
            p.diagram_indirect,
            Some(vec![(0, 0), (1, 1), (2, 1), (2, 2)])
        );
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1]), (2, vec![2])], vec![(2, 1)]))
        );

        let mut p = Problem::from_string("M U U\nP P P\n\nM UP").unwrap();
        p.compute_diagram(&EventHandler::null());
        assert_eq!(
            p.diagram_indirect,
            Some(vec![(0, 0), (1, 1), (1, 2), (2, 1), (2, 2)])
        );
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1, 2])], vec![]))
        );

        let mut p = Problem::from_string("A AB AB\n\nA A\nB B").unwrap();
        p.compute_diagram(&EventHandler::null());
        assert_eq!(p.diagram_indirect, Some(vec![(0, 0), (1, 1)]));
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1])], vec![]))
        );

        let mut p = Problem::from_string("A B AB\n\nB AB").unwrap();
        p.compute_diagram(&EventHandler::null());
        assert_eq!(p.diagram_indirect, Some(vec![(0, 0), (0, 1), (1, 1)]));
        assert_eq!(
            p.diagram_direct,
            Some((vec![(0, vec![0]), (1, vec![1])], vec![(0, 1)]))
        );

        let mut p = Problem::from_string("0	1	1	1\n2	1	1	3\n4	4	4	5\n\n053 4513 4513 4513\n13 13 13 204513\n53 4513 4513 04513\n513 513 0513 4513\n513 513 513 04513").unwrap();
        p.compute_diagram(&EventHandler::null());
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
