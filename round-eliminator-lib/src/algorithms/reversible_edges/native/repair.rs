//! Universally quantified local mending of a two-vertex cluster. An induced
//! matching of edges can be repaired simultaneously; a strong edge coloring
//! gives constantly many phases at fixed degree. Previously good edges remain
//! good, so processing every color removes every forbidden pair.
use super::*;
use itertools::Itertools;

pub(super) const NOT_REPAIRABLE: &str = "No universal two-endpoint repair for this recipe";

pub(super) fn transform(
    input: &Input,
    pairs: &[[Label; 2]],
    budget: &Budget,
    eh: &EventHandler,
) -> Result<Input, String> {
    let forbidden: BTreeSet<_> = input
        .edges
        .iter()
        .filter(|&&[a, b]| pairs.contains(&pair(input.base[a], input.base[b])))
        .copied()
        .collect();
    let good: BTreeSet<_> = input.edges.difference(&forbidden).copied().collect();
    if forbidden.is_empty() {
        return Err(NOT_REPAIRABLE.into());
    }
    let mut neighbors = vec![vec![]; input.names.len()];
    for &[a, b] in &input.edges {
        neighbors[a].push(b);
        if a != b {
            neighbors[b].push(a);
        }
    }
    let mut possibilities = vec![BTreeSet::<Vec<usize>>::new(); input.names.len()];
    for row in &input.nodes {
        for (i, &center) in row.iter().enumerate() {
            budget.check(eh)?;
            if i > 0 && row[i - 1] == center {
                continue;
            }
            let rest: Vec<_> = row
                .iter()
                .enumerate()
                .filter_map(|(j, &s)| (i != j).then_some(s))
                .collect();
            possibilities[center].insert(rest);
        }
    }
    // All legal output rows with the prospective repaired-edge label first.
    let mut output_rows = BTreeSet::new();
    for row in &input.nodes {
        for perm in row.iter().copied().permutations(row.len()) {
            budget.check(eh)?;
            output_rows.insert(perm);
            if output_rows.len() > budget.options.max_configurations {
                return Err(LIMIT.into());
            }
        }
    }
    let affected: BTreeSet<_> = forbidden.iter().flatten().copied().collect();
    let mut families = BTreeMap::<usize, Vec<BTreeSet<usize>>>::new();
    let mut views = 0;
    for &center in &affected {
        let mut minimal: Vec<BTreeSet<usize>> = vec![];
        for rest in &possibilities[center] {
            let choices: Vec<_> = rest.iter().map(|&s| neighbors[s].clone()).collect();
            for boundary in choices
                .iter()
                .map(|v| v.iter().copied())
                .multi_cartesian_product()
            {
                budget.check(eh)?;
                views += 1;
                if views > 200_000 {
                    return Err(LIMIT.into());
                }
                let options: BTreeSet<_> = output_rows
                    .iter()
                    .filter(|out| {
                        out.len() == rest.len() + 1
                            && rest.iter().zip(&boundary).zip(&out[1..]).all(
                                |((&old, &opposite), &new)| {
                                    let edge = annotations::edge(new, opposite);
                                    if good.contains(&annotations::edge(old, opposite)) {
                                        good.contains(&edge)
                                    } else {
                                        input.edges.contains(&edge)
                                    }
                                },
                            )
                    })
                    .map(|row| row[0])
                    .collect();
                if minimal.iter().any(|s| s.is_subset(&options)) {
                    continue;
                }
                minimal.retain(|s| !options.is_subset(s));
                minimal.push(options);
                if minimal.len() > budget.options.max_configurations {
                    return Err(LIMIT.into());
                }
            }
        }
        families.insert(center, minimal);
    }
    for &[a, b] in &forbidden {
        for left in &families[&a] {
            for right in &families[&b] {
                budget.check(eh)?;
                if !left.iter().any(|&x| {
                    right
                        .iter()
                        .any(|&y| good.contains(&annotations::edge(x, y)))
                }) {
                    return Err(NOT_REPAIRABLE.into());
                }
            }
        }
    }
    let mut repaired = input.clone();
    repaired.edges = good;
    Ok(repaired)
}
