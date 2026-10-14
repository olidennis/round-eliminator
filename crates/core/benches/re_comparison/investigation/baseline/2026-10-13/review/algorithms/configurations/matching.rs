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
    // LLM: Most containment queries fail this necessary capacity condition. Check
    // LLM: it before allocating a flow network. Successful queries repeat the
    // LLM: compatibility tests while adding edges; rejected queries allocate none.

    let mut compatible_edges = 0usize;
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (value, count) in offered {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if *count == 0 {
            continue;
        }
        let mut capacity = 0u64;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (target, n) in allowed {
            cancellation.check()?;
            if *n > 0 && compatible(value, target) {
                capacity += u64::from(*n);
                compatible_edges += 1;
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if capacity < u64::from(*count) {
            return Ok(false);
        }
    }
    // LLM: Build a bipartite capacity network: offered counts enter from the source and target
    // LLM: counts drain to the sink.
    let mut network = Graph::<(), u32>::with_capacity(
        2 + offered.len() + allowed.len(),
        offered.len() + allowed.len() + compatible_edges,
    );
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
        if *count == 0 {
            continue;
        }
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for ((target, n), &destination) in allowed.iter().zip(&destinations) {
            cancellation.check()?;
            if *n > 0 && compatible(value, target) {
                network.add_edge(node, destination, (*count).min(*n));
            }
        }
    }
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let matched = dinics(&network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LLM: Regression test: verify that zero count values do not restrict matching. Used by
    /// LLM: the test suite to protect this behavior while implementations change.
    #[test]
    fn zero_count_values_do_not_restrict_matching() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        assert!(
            matches_counts_with(
                &[(0, 0), (1, 2), (2, 1)],
                &[(9, 0), (1, 2), (2, 1)],
                |a, b| a == b,
                &cancellation
            )
            .unwrap()
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            !matches_counts_with(
                &[(0, 0), (1, 2), (2, 1)],
                &[(9, 0), (1, 1), (2, 2)],
                |a, b| a == b,
                &cancellation
            )
            .unwrap()
        );
    }

    /// LLM: Regression test: verify that cancellation during compatibility checks aborts
    /// LLM: matching. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn cancellation_during_compatibility_checks_aborts_matching() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let result = matches_counts_with(
            &[(0, 2), (1, 2)],
            &[(0, 2), (1, 2)],
            |a, b| {
                cancellation.cancel();
                a == b
            },
            &cancellation,
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(result.is_err());
    }
}
