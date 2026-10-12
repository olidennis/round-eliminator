//! Condensation and transitive reduction of directed relations.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{
    algo::{kosaraju_scc, toposort, tred},
    graph::DiGraph,
};

pub(crate) struct Reduction {
    /// Original vertex indices in each component, ordered deterministically.
    pub components: Vec<Vec<u32>>,
    /// Cover edges between component indices, not original vertex indices.
    pub edges: Vec<(u32, u32)>,
}

pub(crate) fn reduce_relation(
    successors: &[Vec<usize>],
    cancellation: &Cancellation,
) -> Result<Reduction, Cancelled> {
    cancellation.check()?;
    let mut graph = DiGraph::<(), ()>::new();
    let vertices: Vec<_> = successors.iter().map(|_| graph.add_node(())).collect();
    for (source, targets) in successors.iter().enumerate() {
        cancellation.check()?;
        for &target in targets {
            if source != target {
                graph.add_edge(vertices[source], vertices[target], ());
            }
        }
    }
    let mut components: Vec<Vec<u32>> = kosaraju_scc(&graph)
        .into_iter()
        .map(|component| {
            let mut members: Vec<_> = component
                .into_iter()
                .map(|node| node.index() as u32)
                .collect();
            members.sort_unstable();
            members
        })
        .collect();
    components.sort_unstable();
    let mut component_of = vec![0; vertices.len()];
    for (index, members) in components.iter().enumerate() {
        for &member in members {
            component_of[member as usize] = index;
        }
    }
    let mut dag = DiGraph::<(), ()>::new();
    let vertices: Vec<_> = components.iter().map(|_| dag.add_node(())).collect();
    let mut pairs = std::collections::BTreeSet::new();
    for (source, targets) in successors.iter().enumerate() {
        cancellation.check()?;
        for &target in targets {
            let pair = (component_of[source], component_of[target]);
            if pair.0 != pair.1 && pairs.insert(pair) {
                dag.add_edge(vertices[pair.0], vertices[pair.1], ());
            }
        }
    }
    let topo = toposort(&dag, None).expect("SCC condensation is acyclic");
    let (adjacency, _) = tred::dag_to_toposorted_adjacency_list::<_, usize>(&dag, &topo);
    let (reduced, _) = tred::dag_transitive_reduction_closure(&adjacency);
    let mut edges: Vec<_> = reduced
        .edge_indices()
        .map(|edge| {
            let (source, target) = reduced.edge_endpoints(edge).unwrap();
            (topo[source].index() as u32, topo[target].index() as u32)
        })
        .collect();
    edges.sort_unstable();
    cancellation.check()?;
    Ok(Reduction { components, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_cycles_reduces_transitive_edges_and_retains_isolated_nodes() {
        let result = reduce_relation(
            &[
                vec![0, 1, 2, 3],
                vec![0, 1, 2, 3],
                vec![2, 3],
                vec![3],
                vec![4],
            ],
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            result.components,
            vec![vec![0, 1], vec![2], vec![3], vec![4]]
        );
        assert_eq!(result.edges, vec![(0, 1), (1, 2)]);
        let empty = reduce_relation(&[], &Cancellation::default()).unwrap();
        assert!(empty.components.is_empty() && empty.edges.is_empty());
    }
}
