//! Generic, bounded portfolios derived from node supports and the edge
//! relation, never from a fixture's names or a hard-coded successful recipe.
use super::*;
use itertools::Itertools;

fn compact(recipe: &[Step]) -> Vec<Step> {
    recipe
        .iter()
        .flat_map(|s| [s.clone(), Step::Prune])
        .collect()
}

pub(super) fn recipes(
    q: &Problem,
    added: &[[Label; 2]],
    budget: &Budget,
    eh: &EventHandler,
) -> Result<Vec<Vec<Step>>, String> {
    let input = Input::new(q, budget, eh)?;
    let rows: Vec<Vec<Label>> = input
        .nodes
        .iter()
        .map(|r| {
            let mut r: Vec<_> = r.iter().map(|&s| input.base[s]).collect();
            r.sort();
            r
        })
        .collect();
    let supports: BTreeSet<BTreeSet<_>> =
        rows.iter().map(|r| r.iter().copied().collect()).collect();
    let mut components: Vec<BTreeSet<Label>> =
        input.base.iter().map(|&s| BTreeSet::from([s])).collect();
    for support in &supports {
        budget.check(eh)?;
        let mut merged = support.clone();
        components.retain(|c| {
            if c.is_disjoint(support) {
                true
            } else {
                merged.extend(c);
                false
            }
        });
        components.push(merged);
    }
    let edges: BTreeSet<_> = input
        .edges
        .iter()
        .map(|&[a, b]| pair(input.base[a], input.base[b]))
        .collect();
    let mut graphs = vec![Subgraph::All, Subgraph::Pairs(added.to_vec())];
    for support in components.iter().chain(supports.iter().take(16)) {
        let pairs: Vec<_> = edges
            .iter()
            .filter(|[a, b]| support.contains(a) && support.contains(b))
            .copied()
            .collect();
        if !pairs.is_empty() {
            graphs.push(Subgraph::Pairs(pairs));
        }
    }
    graphs.extend(
        input
            .base
            .iter()
            .filter(|&&l| edges.contains(&[l, l]))
            .map(|&l| Subgraph::Pairs(vec![[l, l]])),
    );
    graphs.extend(edges.iter().take(32).map(|&e| Subgraph::Pairs(vec![e])));
    let mut seen = BTreeSet::new();
    graphs.retain(|g| seen.insert(g.clone()));
    let mut result = vec![
        vec![],
        vec![Step::RepairPairs(added.to_vec())],
        vec![Step::FindMis(1)],
        vec![Step::Coloring],
    ];
    // One-stage rules of each family get a chance before expensive products.
    for graph in &graphs {
        for step in [
            Step::Mis(graph.clone()),
            Step::Matching(graph.clone()),
            Step::GreedyColoring(graph.clone()),
            Step::RulingSet(graph.clone()),
        ] {
            result.push(compact(&[step]));
        }
    }
    result.push(vec![Step::NodeContext, Step::Exchange, Step::Prune]);
    result.push(vec![Step::FindMis(2)]);
    result.push(vec![Step::FindMis(3)]);
    let old = super::recipes(q, added);
    result.extend(old.iter().cloned());
    result.extend(old.iter().map(|r| compact(r)));
    // Priorities are input node types, not label names. Exhaust all orders for
    // at most five types; otherwise use both directions and bounded rotations.
    let mut orders = vec![];
    if !rows.is_empty() && rows.len() <= 5 {
        orders.extend(rows.iter().cloned().permutations(rows.len()));
    } else if !rows.is_empty() && rows.len() <= 16 {
        for reverse in [false, true] {
            for start in 0..rows.len().min(8) {
                let mut order = rows.clone();
                if reverse {
                    order.reverse();
                }
                order.rotate_left(start);
                orders.push(order);
            }
        }
    }
    let type_pairs: Vec<Vec<(usize, usize)>> = if orders.is_empty() {
        vec![]
    } else {
        graphs
            .iter()
            .take(8)
            .map(|graph| {
                (0..rows.len())
                    .flat_map(|i| (i + 1..rows.len()).map(move |j| (i, j)))
                    .filter(|&(i, j)| {
                        rows[i].iter().any(|&a| {
                            rows[j].iter().any(|&b| {
                                let e = pair(a, b);
                                edges.contains(&e)
                                    && match graph {
                                        Subgraph::All => true,
                                        Subgraph::Pairs(p) => p.contains(&e),
                                    }
                            })
                        })
                    })
                    .collect()
            })
            .collect()
    };
    let mut priorities: Vec<BTreeSet<Vec<bool>>> = vec![BTreeSet::new(); type_pairs.len()];
    let mut priority_exchange = vec![];
    for order in orders {
        let ranks: Vec<_> = rows
            .iter()
            .map(|r| order.iter().position(|s| r == s).unwrap())
            .collect();
        for (g, graph) in graphs.iter().take(8).enumerate() {
            // Only comparisons between types joined by a selected edge can
            // affect parent eligibility. Avoid repeating equivalent orders.
            let signature = type_pairs[g]
                .iter()
                .map(|&(i, j)| ranks[i] < ranks[j])
                .collect();
            if !priorities[g].insert(signature) {
                continue;
            }
            let recipe = vec![
                Step::PriorityMis {
                    graph: graph.clone(),
                    order: order.clone(),
                },
                Step::Prune,
            ];
            result.push(recipe.clone());
            let mut expanded = recipe;
            expanded.extend([Step::Exchange, Step::Prune]);
            priority_exchange.push(expanded);
        }
    }
    for graph in &graphs {
        for step in [
            Step::Mis(graph.clone()),
            Step::Matching(graph.clone()),
            Step::RulingSet(graph.clone()),
        ] {
            result.push(vec![step.clone(), Step::Prune, Step::Exchange, Step::Prune]);
            result.push(vec![
                step,
                Step::Prune,
                Step::NodeContext,
                Step::Exchange,
                Step::Prune,
            ]);
        }
    }
    // Joint label-preserving computations, including mixed primitive families.
    for a in graphs.iter().take(12) {
        for b in graphs.iter().take(12) {
            if a == b {
                continue;
            }
            for (first, second) in [
                (Step::Mis(a.clone()), Step::Mis(b.clone())),
                (Step::Mis(a.clone()), Step::GreedyColoring(b.clone())),
                (
                    Step::GreedyColoring(a.clone()),
                    Step::GreedyColoring(b.clone()),
                ),
                (Step::RulingSet(a.clone()), Step::Mis(b.clone())),
                (Step::RulingSet(a.clone()), Step::GreedyColoring(b.clone())),
            ] {
                result.push(vec![first, Step::Prune, second, Step::Prune]);
            }
        }
    }
    // Full-node and edge exchange for each legacy MIS recipe, including joint
    // independent per-label MIS computations.
    for r in old.iter().filter(|r| !r.contains(&Step::Exchange)) {
        let mut r = compact(r);
        r.extend([Step::NodeContext, Step::Exchange, Step::Prune]);
        result.push(r);
    }
    result.extend(priority_exchange);
    let mut seen = BTreeSet::new();
    result.retain(|r| r.len() <= 16 && seen.insert(r.clone()));
    Ok(result)
}
