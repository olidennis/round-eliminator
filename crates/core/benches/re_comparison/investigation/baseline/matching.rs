//! Capacity matching for arbitrary counted values with a compatibility relation.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{Graph, algo::dinics};

pub(crate) fn matches_counts_with<A, B>(
    offered: &[(A, u32)],
    allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    let degree: u64 = offered.iter().map(|(_, n)| u64::from(*n)).sum();
    if degree != allowed.iter().map(|(_, n)| u64::from(*n)).sum::<u64>() {
        return Ok(false);
    }
    if degree == 0 {
        return Ok(true);
    }
    if let [(target, _)] = allowed {
        for (value, count) in offered {
            cancellation.check()?;
            if *count > 0 && !compatible(value, target) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if let [(value, _)] = offered {
        for (target, count) in allowed {
            cancellation.check()?;
            if *count > 0 && !compatible(value, target) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    let mut network = Graph::<(), u32>::new();
    let source = network.add_node(());
    let sink = network.add_node(());
    let mut destinations = Vec::with_capacity(allowed.len());
    for (_, count) in allowed {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, *count);
        destinations.push(node);
    }
    for (value, count) in offered {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        let mut capacity = 0u64;
        for ((target, n), &destination) in allowed.iter().zip(&destinations) {
            cancellation.check()?;
            if compatible(value, target) {
                capacity += u64::from(*n);
                network.add_edge(node, destination, (*count).min(*n));
            }
        }
        if capacity < u64::from(*count) {
            return Ok(false);
        }
    }
    cancellation.check()?;
    let matched = dinics(&network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}
