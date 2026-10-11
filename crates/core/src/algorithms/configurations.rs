//! Multiset enumeration, concrete membership, and condensed containment.

use std::collections::BTreeSet;

use petgraph::{Graph, algo::dinics};

use crate::computation::{Cancellation, Cancelled};
use crate::model::{CondensedConfiguration, Configuration, Constraint};

/// Checks whether a constraint allows a configuration
pub fn accepts<L: Copy + Ord>(constraint: &Constraint<L>, concrete: &Configuration<L>) -> bool {
    accepts_with(constraint, concrete, &Cancellation::default()).expect("not cancelled")
}

/// This variant of `accepts` takes a Cancellation explicitly
pub(crate) fn accepts_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    // we go through all condensed configuration and check for each of them
    for configuration in constraint.configurations_at_degree(concrete.degree()) {
        cancellation.check()?;
        if matches(configuration, concrete, cancellation)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// We use a flow algorithm to check if a condensed configuration contains a given concrete configuration
fn matches<L: Copy + Ord>(
    configuration: &CondensedConfiguration<L>,
    concrete: &Configuration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let parts = configuration.parts();
    // Degree 0, easy
    if concrete.degree() == 0 {
        return Ok(true);
    }
    // the concrete configuration contains a single label many times, easy check
    if let [(label, _)] = concrete.multiplicities() {
        for part in parts {
            cancellation.check()?;
            if !part.labels().contains(*label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // the target condensed configuration has a single set repeated many tines, easy check
    if let [part] = parts {
        for &(label, _) in concrete.multiplicities() {
            cancellation.check()?;
            if !part.labels().contains(label) {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    // we construct the flow network
    // source -> concrete labels -> condensed parts -> sink.
    // Counts are capacities, so a large exponent never creates one vertex per
    // occurrence. Saturating the source assigns every occurrence to a part;
    // equal degrees then also fill every part to its required multiplicity.
    let mut network = Graph::<(), u32>::new();
    let source = network.add_node(());
    let sink = network.add_node(());
    let mut destinations = Vec::with_capacity(parts.len());
    // edges from target condensed configuration to sink
    for part in parts {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, part.multiplicity());
        destinations.push(node);
    }
    for &(label, count) in concrete.multiplicities() {
        cancellation.check()?;
        // edges from source to concrete configuration
        let node = network.add_node(());
        network.add_edge(source, node, count);
        // edges from concrete to target
        let mut available = 0u32;
        for (part, &destination) in parts.iter().zip(&destinations) {
            cancellation.check()?;
            if part.labels().contains(label) {
                available += part.multiplicity();
                network.add_edge(node, destination, count.min(part.multiplicity()));
            }
        }
        // Even assigning every compatible slot cannot place this label.
        // This often rules out a candidate without any flow search.
        if available < count {
            return Ok(false);
        }
    }
    cancellation.check()?;
    let (flow, _) = dinics(&network, source, sink);
    cancellation.check()?;
    Ok(flow == concrete.degree())
}

/// Prove containment by assigning each family occurrence to a target part whose
/// label set includes the whole family set. Capacities preserve multiplicities.
///
/// This is a sufficient test for semantic containment. Failure does not prove
/// that a concrete output is forbidden: several targets can cover a family
/// jointly. Scanning all targets is complete when the constraint is maximized.
pub(crate) fn contains_condensed_with<L: Copy + Ord>(
    target: &CondensedConfiguration<L>,
    family: &CondensedConfiguration<L>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    cancellation.check()?;
    if target.degree() != family.degree() {
        return Ok(false);
    }
    if let [part] = target.parts() {
        for query in family.parts() {
            cancellation.check()?;
            if !query.labels().is_subset_of(part.labels()) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if let [part] = family.parts() {
        for query in target.parts() {
            cancellation.check()?;
            if !part.labels().is_subset_of(query.labels()) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    let mut network = Graph::<(), u32>::new();
    let source = network.add_node(());
    let sink = network.add_node(());
    let mut destinations = Vec::new();
    for part in target.parts() {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(node, sink, part.multiplicity());
        destinations.push(node);
    }
    for part in family.parts() {
        cancellation.check()?;
        let node = network.add_node(());
        network.add_edge(source, node, part.multiplicity());
        let mut available = 0;
        for (target, &destination) in target.parts().iter().zip(&destinations) {
            cancellation.check()?;
            if part.labels().is_subset_of(target.labels()) {
                available += target.multiplicity();
                network.add_edge(node, destination, part.multiplicity().min(target.multiplicity()));
            }
        }
        if available < part.multiplicity() {
            return Ok(false);
        }
    }
    cancellation.check()?;
    let flow = dinics(&network, source, sink).0;
    cancellation.check()?;
    Ok(flow == family.degree())
}

/// Extracts all uncondensed configurations of given degrees from a constraint
#[cfg(test)]
pub(crate) fn concrete_multisets<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
) -> Vec<Configuration<L>> {
    concrete_multisets_with(constraint, degrees, &Cancellation::default()).expect("not cancelled")
}

pub(crate) fn concrete_multisets_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    degrees: &[u32],
    cancellation: &Cancellation,
) -> Result<Vec<Configuration<L>>, Cancelled> {
    cancellation.check()?;
    let mut result = BTreeSet::new();
    for &degree in degrees {
        for configuration in constraint.configurations_at_degree(degree) {
            let mut prefixes = BTreeSet::from([Configuration::default()]);
            for part in configuration.parts() {
                let choices: Vec<_> = part.labels().iter().collect();
                let mut next = BTreeSet::new();
                for prefix in &prefixes {
                    // Enumerate count distributions, using one counter per
                    // distinct alternative rather than per repeated occurrence.
                    let mut counts = vec![0u32; choices.len()];
                    counts[0] = part.multiplicity();
                    loop {
                        cancellation.check()?;
                        let mut entries = prefix.multiplicities().to_vec();
                        entries.extend(choices.iter().copied().zip(counts.iter().copied()));
                        next.insert(
                            Configuration::new(entries).expect("expanded degree was checked"),
                        );
                        let Some(position) = counts[..counts.len() - 1]
                            .iter()
                            .rposition(|&count| count != 0)
                        else {
                            break;
                        };
                        counts[position] -= 1;
                        let remainder = counts[position + 1..].iter().sum::<u32>() + 1;
                        counts[position + 1..].fill(0);
                        counts[position + 1] = remainder;
                    }
                }
                prefixes = next;
            }
            result.extend(prefixes);
        }
    }
    Ok(result.into_iter().collect())
}

#[cfg(test)]
mod tests;
