//! LLM: Reachability, condensation and transitive reduction of directed relations.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{
    algo::{kosaraju_scc, toposort, tred},
    graph::DiGraph,
    visit::Bfs,
};

/// LLM: Compute reflexive directed reachability with cancellation. Used by simplification and
/// LLM: diagram-based order constructions to include each vertex itself.
pub(crate) fn reachability_with(
    successors: &[Vec<usize>],
    cancellation: &Cancellation,
) -> Result<Vec<Vec<usize>>, Cancelled> {
    // LLM: Prepare `graph` for the following computation; it stores the graph for this stage.
    let mut graph = DiGraph::<(), ()>::new();
    let vertices: Vec<_> = successors.iter().map(|_| graph.add_node(())).collect();
    for (source, targets) in successors.iter().enumerate() {
        cancellation.check()?;
        for &target in targets {
            graph.add_edge(vertices[source], vertices[target], ());
        }
    }
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let mut result = Vec::new();
    for start in vertices {
        // LLM: Prepare `traversal` for the following computation; it stores the traversal for
        // LLM: this stage.
        let mut traversal = Bfs::new(&graph, start);
        let mut reachable = Vec::new();
        while let Some(node) = traversal.next(&graph) {
            cancellation.check()?;
            reachable.push(node.index());
        }
        reachable.sort_unstable();
        result.push(reachable);
    }
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    Ok(result)
}

/// LLM: A directed relation condensed into deterministic SCC membership lists and cover edges.
/// LLM: Members index original vertices; edges index components, not the original label table.
pub(crate) struct Reduction {
    /// LLM: Original vertex indices in each component, ordered deterministically.
    pub components: Vec<Vec<u32>>,
    /// LLM: Cover edges between component indices, not original vertex indices.
    pub edges: Vec<(u32, u32)>,
}

/// LLM: Group equivalent vertices into SCCs and compute the transitive reduction of the
/// LLM: condensation DAG. Used to prepare compact display edges while retaining deterministic
/// LLM: component indices.
pub(crate) fn reduce_relation(
    successors: &[Vec<usize>],
    cancellation: &Cancellation,
) -> Result<Reduction, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let mut graph = DiGraph::<(), ()>::new();
    let vertices: Vec<_> = successors.iter().map(|_| graph.add_node(())).collect();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (source, targets) in successors.iter().enumerate() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        for &target in targets {
            if source != target {
                graph.add_edge(vertices[source], vertices[target], ());
            }
        }
    }
    // LLM: Group mutually reachable vertices into equivalence components.
    let mut components: Vec<Vec<u32>> = kosaraju_scc(&graph)
        .into_iter()
        .map(|component| {
            // LLM: Sort original vertex indices inside each SCC and then order the components
            // LLM: deterministically.
            let mut members: Vec<_> = component
                .into_iter()
                .map(|node| node.index() as u32)
                .collect();
            members.sort_unstable();
            members
        })
        .collect();
    // LLM: Restore deterministic canonical ordering before the value is stored or returned.
    components.sort_unstable();
    let mut component_of = vec![0; vertices.len()];
    for (index, members) in components.iter().enumerate() {
        for &member in members {
            component_of[member as usize] = index;
        }
    }
    let mut dag = DiGraph::<(), ()>::new();
    // LLM: Prepare `vertices` for the following computation; it stores the vertices for this
    // LLM: stage.
    let vertices: Vec<_> = components.iter().map(|_| dag.add_node(())).collect();
    let mut pairs = std::collections::BTreeSet::new();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (source, targets) in successors.iter().enumerate() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        for &target in targets {
            let pair = (component_of[source], component_of[target]);
            if pair.0 != pair.1 && pairs.insert(pair) {
                dag.add_edge(vertices[pair.0], vertices[pair.1], ());
            }
        }
    }
    // LLM: Topologically order the SCC DAG so the library can compute its transitive
    // LLM: reduction.
    let topo = toposort(&dag, None).expect("SCC condensation is acyclic");
    let (adjacency, _) = tred::dag_to_toposorted_adjacency_list::<_, usize>(&dag, &topo);
    let (reduced, _) = tred::dag_transitive_reduction_closure(&adjacency);
    // LLM: Prepare `edges` for the following computation; it stores the edges for this stage.
    let mut edges: Vec<_> = reduced
        .edge_indices()
        .map(|edge| {
            let (source, target) = reduced.edge_endpoints(edge).unwrap();
            (topo[source].index() as u32, topo[target].index() as u32)
        })
        .collect();
    edges.sort_unstable();
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    Ok(Reduction { components, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// LLM: Regression test: verify that groups cycles reduces transitive edges and retains
    /// LLM: isolated nodes. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn groups_cycles_reduces_transitive_edges_and_retains_isolated_nodes() {
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
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
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            result.components,
            vec![vec![0, 1], vec![2], vec![3], vec![4]]
        );
        assert_eq!(result.edges, vec![(0, 1), (1, 2)]);
        let empty = reduce_relation(&[], &Cancellation::default()).unwrap();
        assert!(empty.components.is_empty() && empty.edges.is_empty());
    }
}
