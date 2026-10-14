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
    // Most containment queries fail this necessary capacity condition. Check
    // it before allocating a flow network. Successful queries repeat the
    // compatibility tests while adding edges for larger matrices. Small ones
    // retain the successful comparisons in a stack bitmask, so neither rejected
    // queries nor compatibility caching allocate a separate buffer.
    let cache = offered.len().saturating_mul(allowed.len()) <= 128;
    let mut bits = 0u128;
    let mut compatible_edges = 0usize;
    for (i, (value, count)) in offered.iter().enumerate() {
        cancellation.check()?;
        if *count == 0 {
            continue;
        }
        let mut capacity = 0u64;
        for (j, (target, n)) in allowed.iter().enumerate() {
            cancellation.check()?;
            if *n > 0 && compatible(value, target) {
                capacity += u64::from(*n);
                compatible_edges += 1;
                if cache {
                    bits |= 1u128 << (i * allowed.len() + j);
                }
            }
        }
        if capacity < u64::from(*count) {
            return Ok(false);
        }
    }
    let mut network = Graph::<(), u32>::with_capacity(
        2 + offered.len() + allowed.len(),
        offered.len() + allowed.len() + compatible_edges,
    );
    let source = network.add_node(());
    let sink = network.add_node(());
    let mut destinations = Vec::with_capacity(allowed.len());
    for (_, count) in allowed {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, *count);
        destinations.push(node);
    }
    for (i, (value, count)) in offered.iter().enumerate() {
        cancellation.check()?;
        if *count == 0 {
            continue;
        }
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        for (j, ((target, n), &destination)) in allowed.iter().zip(&destinations).enumerate() {
            cancellation.check()?;
            if *n > 0
                && (if cache {
                    bits & (1u128 << (i * allowed.len() + j)) != 0
                } else {
                    compatible(value, target)
                })
            {
                network.add_edge(node, destination, (*count).min(*n));
            }
        }
    }
    cancellation.check()?;
    let matched = dinics(&network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_cache_last_bit_and_larger_fallback_keep_exact_flow_checks() {
        let cancellation = Cancellation::default();
        for width in [64, 65] {
            let allowed: Vec<_> = (0..width).map(|i| (i, 1)).collect();
            assert!(
                matches_counts_with(
                    &[(0, width - 1), (1, 1)],
                    &allowed,
                    |a, b| (*a == 0 && *b < width - 1) || (*a == 1 && *b == width - 1),
                    &cancellation
                )
                .unwrap()
            );
        }
        // Every offered row passes its capacity check, but they compete for
        // the same destinations. Cached compatibility still needs the flow.
        let allowed: Vec<_> = (0..64).map(|i| (i, 1)).collect();
        assert!(
            !matches_counts_with(&[(0, 32), (1, 32)], &allowed, |_, b| *b < 32, &cancellation)
                .unwrap()
        );
    }

    #[test]
    fn zero_count_values_do_not_restrict_matching() {
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

    #[test]
    fn cancellation_during_compatibility_checks_aborts_matching() {
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
        assert!(result.is_err());
    }
}
