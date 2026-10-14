//! Benchmark-only retained graph storage. Dinic remains petgraph's implementation.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{Graph, graph::NodeIndex, algo::dinics};
use std::cell::RefCell;

#[derive(Default)]
struct Scratch {
    network: Graph<(), u32>,
    destinations: Vec<NodeIndex>,
}

thread_local! { static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default()); }

pub(crate) fn solve_with<A, B>(
    offered: &[(A, u32)], allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool,
    bits: Option<u128>, edges: usize, degree: u64,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    SCRATCH.with(|cell| {
        // A compatibility callback can itself perform matching. Nested calls
        // receive fresh storage instead of panicking on a RefCell borrow.
        if let Ok(mut scratch) = cell.try_borrow_mut() {
            run(&mut scratch, offered, allowed, compatible, bits, edges, degree, cancellation)
        } else {
            run(&mut Scratch::default(), offered, allowed, compatible, bits, edges, degree, cancellation)
        }
    })
}

fn run<A, B>(
    scratch: &mut Scratch, offered: &[(A, u32)], allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool, bits: Option<u128>, edges: usize,
    degree: u64, cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let Scratch { network, destinations } = scratch;
    network.clear();
    network.reserve_nodes(2 + offered.len() + allowed.len());
    network.reserve_edges(offered.len() + allowed.len() + edges);
    destinations.clear();
    let source = network.add_node(());
    let sink = network.add_node(());
    for (_, count) in allowed {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, *count);
        destinations.push(node);
    }
    for (i, (value, count)) in offered.iter().enumerate() {
        cancellation.check()?;
        if *count == 0 { continue; }
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        for (j, ((target, n), &destination)) in allowed.iter().zip(destinations.iter()).enumerate() {
            cancellation.check()?;
            let edge = if let Some(bits) = bits { bits & (1 << (i * allowed.len() + j)) != 0 }
                else { *n > 0 && compatible(value, target) };
            if edge { network.add_edge(node, destination, (*count).min(*n)); }
        }
    }
    cancellation.check()?;
    let matched = dinics(&*network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}
