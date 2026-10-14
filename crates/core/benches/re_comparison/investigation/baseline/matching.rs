//! LLM: Capacity matching for arbitrary counted values with a compatibility relation.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{Graph, algo::dinics};

/// LLM: Decide whether all offered counts can be assigned to compatible target capacities
/// LLM: using cheap rejection followed by exact Dinic flow. Used by membership, dominance, and
/// LLM: existential choice checks.
pub(crate) fn matches_counts_with<A, B>(
    offered: &[(A, u32)],
    allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let degree: u64 = offered.iter().map(|(_, n)| u64::from(*n)).sum();
    if degree != allowed.iter().map(|(_, n)| u64::from(*n)).sum::<u64>() {
        return Ok(false);
    }
    if degree == 0 {
        return Ok(true);
    }
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if let [(target, _)] = allowed {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (value, count) in offered {
            cancellation.check()?;
            if *count > 0 && !compatible(value, target) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if let [(value, _)] = offered {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (target, count) in allowed {
            cancellation.check()?;
            if *count > 0 && !compatible(value, target) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // LLM: Build a source/target capacity network for exact counted assignment.
    let mut network = Graph::<(), u32>::new();
    let source = network.add_node(());
    let sink = network.add_node(());
    let mut destinations = Vec::with_capacity(allowed.len());
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (_, count) in allowed {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, *count);
        destinations.push(node);
    }
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (value, count) in offered {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        let mut capacity = 0u64;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for ((target, n), &destination) in allowed.iter().zip(&destinations) {
            cancellation.check()?;
            if compatible(value, target) {
                capacity += u64::from(*n);
                network.add_edge(node, destination, (*count).min(*n));
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if capacity < u64::from(*count) {
            return Ok(false);
        }
    }
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let matched = dinics(&network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}
