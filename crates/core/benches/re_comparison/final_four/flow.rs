//! LLM: Benchmark-only retained graph storage. Dinic remains petgraph's implementation.

use crate::computation::{Cancellation, Cancelled};
use petgraph::{Graph, graph::NodeIndex, algo::dinics};
use std::cell::RefCell;

/// LLM: Reusable flow-network scratch storage in the benchmark prototype. Retaining buffers
/// LLM: lets the experiment measure whether allocation savings outweigh network-reset costs.
#[derive(Default)]
struct Scratch {
    network: Graph<(), u32>,
    destinations: Vec<NodeIndex>,
}

thread_local! { static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default()); }

/// LLM: Run exact counted flow matching with reusable thread-local graph storage when
/// LLM: borrowing permits it. Used only by the retained allocation experiment; nested matching
/// LLM: falls back to fresh scratch storage.
pub(crate) fn solve_with<A, B>(
    offered: &[(A, u32)], allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool,
    bits: Option<u128>, edges: usize, degree: u64,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Update the current working state before continuing this operation.
    SCRATCH.with(|cell| {
        // LLM: A compatibility callback can itself perform matching. Nested calls
        // LLM: receive fresh storage instead of panicking on a RefCell borrow.

        if let Ok(mut scratch) = cell.try_borrow_mut() {
            run(&mut scratch, offered, allowed, compatible, bits, edges, degree, cancellation)
        } else {
            run(&mut Scratch::default(), offered, allowed, compatible, bits, edges, degree, cancellation)
        }
    })
}

/// LLM: Evaluate the prepared benchmark or operation fixture and return its result data. Used
/// LLM: by this module's harness rather than the application protocol.
fn run<A, B>(
    scratch: &mut Scratch, offered: &[(A, u32)], allowed: &[(B, u32)],
    compatible: impl Fn(&A, &B) -> bool, bits: Option<u128>, edges: usize,
    degree: u64, cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Prepare `Scratch` for the following computation; it stores the scratch for this
    // LLM: stage.
    let Scratch { network, destinations } = scratch;
    network.clear();
    network.reserve_nodes(2 + offered.len() + allowed.len());
    network.reserve_edges(offered.len() + allowed.len() + edges);
    destinations.clear();
    let source = network.add_node(());
    let sink = network.add_node(());
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
    for (i, (value, count)) in offered.iter().enumerate() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if *count == 0 { continue; }
        let node = network.add_node(());
        network.add_edge(source, node, *count);
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (j, ((target, n), &destination)) in allowed.iter().zip(destinations.iter()).enumerate() {
            cancellation.check()?;
            let edge = if let Some(bits) = bits { bits & (1 << (i * allowed.len() + j)) != 0 }
                else { *n > 0 && compatible(value, target) };
            if edge { network.add_edge(node, destination, (*count).min(*n)); }
        }
    }
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let matched = dinics(&*network, source, sink).0;
    cancellation.check()?;
    Ok(u64::from(matched) == degree)
}
