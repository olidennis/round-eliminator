//! Counted pair combinations and dominance pruning of intersection alignments.

use super::{antichain::dominates, engine::Operations};
use crate::{
    algorithms::configurations::{AlignmentComponents, AlignmentSearch},
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

/// An optional derivation of an accepted configuration. Indices refer to the
/// parents' sorted distinct values, and intersection counts are row-major.
/// Parent copies and count matrices are allocated only when tracking is enabled.
pub struct Combination<T> {
    pub left: Configuration<T>,
    pub right: Configuration<T>,
    pub union_parts: (usize, usize),
    pub intersection_counts: Vec<u32>,
}

/// Called on the requesting thread when a derived configuration is admitted.
/// Later configurations may dominate it; this is a derivation trace, not a
/// notification that the configuration will belong to the final maximum.
pub type DerivationObserver<'a, T> = dyn FnMut(&Configuration<T>, Combination<T>) + 'a;

pub(super) struct Candidate<T> {
    pub(super) configuration: Configuration<T>,
    pub(super) combination: Option<Box<Combination<T>>>,
}

pub(super) fn combine<T: Clone + Ord>(
    left: &Configuration<T>,
    right: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    tracking: bool,
    known: &super::history::History<T>,
    generated: Option<&Mutex<BTreeSet<Configuration<T>>>>,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    let a = left.multiplicities();
    let b = right.multiplicities();
    let mut choices = Vec::new();
    for (i, (x, _)) in a.iter().enumerate() {
        for (j, (y, _)) in b.iter().enumerate() {
            cancellation.check()?;
            if operations.is_superset(x, y) || operations.is_superset(y, x) {
                continue;
            }
            let union = operations.union(x, y);
            if !operations.is_admissible(&union) {
                continue;
            }
            choices.push((i, j, union));
        }
    }
    // Comparable operands cannot produce a configuration stronger than a parent.
    // Avoid intersection/exchange preparation altogether when no join survives.
    if choices.is_empty() {
        return Ok(Vec::new());
    }
    // Each intersection is computed once for all union choices and alignments.
    let mut intersections = Vec::with_capacity(a.len() * b.len());
    for (x, _) in a {
        for (y, _) in b {
            cancellation.check()?;
            intersections.push(operations.intersection(x, y));
        }
    }
    let compatible: Vec<_> = intersections
        .iter()
        .map(|v| operations.is_admissible(v))
        .collect();
    let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
    let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
    // After selecting one join, every remaining occurrence must be assigned
    // through an admissible intersection. Component demand must balance.
    if !compatible.iter().all(|&allowed| allowed) {
        let components = AlignmentComponents::new(&rows, &columns, &compatible, cancellation)?;
        choices.retain(|&(i, j, _)| components.allows_removing(i, j));
        if choices.is_empty() {
            return Ok(Vec::new());
        }
    }
    let conflicts = dominated_exchanges(
        &intersections,
        &compatible,
        a.len(),
        b.len(),
        operations,
        cancellation,
    )?;
    let mut result: Vec<Candidate<T>> = Vec::new();
    let mut seen = BTreeSet::new();
    // Dense alignments benefit from pruning repeated joins (notably finite
    // lattices allowing bottom values). Sparse set problems reject alignments
    // cheaply, so grouping costs more than it saves on the measured fixtures.
    // Traced runs keep the original operand order and intermediate admissions.
    if !tracking && compatible.iter().all(|&allowed| allowed) {
        choices = prune_union_choices(choices, a, b, operations, cancellation)?;
    }
    // Intersections and exchange conflicts are constant for this parent pair.
    // Reuse their indices and the traversal buffers across every join choice.
    let mut alignments = AlignmentSearch::new(&compatible, &conflicts, cancellation)?;
    for (i, j, union) in choices {
        rows[i] -= 1;
        columns[j] -= 1;
        alignments.visit_with(&rows, &columns, |counts| {
            let mut parts = vec![(union.clone(), 1)];
            parts.extend(
                counts
                    .iter()
                    .zip(&intersections)
                    .filter(|(n, _)| **n > 0)
                    .map(|(&n, value)| (value.clone(), n)),
            );
            let configuration = Configuration::new(parts).expect("combination preserves degree");
            // Every history entry is already covered by the global antichain.
            // Check before local matching/flows, even if another parent pair
            // produced this configuration. Global history remains immutable
            // until the generation batch finishes; selected dense searches
            // also share exact generated keys within the batch.
            if known.contains(&configuration) {
                return Ok(());
            }
            if let Some(generated) = generated {
                if !generated.lock().unwrap().insert(configuration.clone()) {
                    return Ok(());
                }
            } else if !seen.insert(configuration.clone()) {
                return Ok(());
            }
            if dominates(left, &configuration, operations, cancellation)?
                || dominates(right, &configuration, operations, cancellation)?
            {
                return Ok(());
            }
            for old in &result {
                if dominates(&old.configuration, &configuration, operations, cancellation)? {
                    return Ok(());
                }
            }
            let mut kept = Vec::new();
            for old in result.drain(..) {
                if !dominates(&configuration, &old.configuration, operations, cancellation)? {
                    kept.push(old)
                }
            }
            kept.push(Candidate {
                configuration,
                combination: tracking.then(|| {
                    Box::new(Combination {
                        left: left.clone(),
                        right: right.clone(),
                        union_parts: (i, j),
                        intersection_counts: counts.to_vec(),
                    })
                }),
            });
            result = kept;
            Ok(())
        })?;
        rows[i] += 1;
        columns[j] += 1;
    }
    Ok(result)
}

/// For the same join, consuming smaller operands leaves larger values available
/// for the remaining intersections. Monotonicity therefore makes every result
/// of the discarded operand choice dominated by a result of the retained one.
fn prune_union_choices<T: Clone + Ord>(
    choices: Vec<(usize, usize, T)>,
    left: &[(T, u32)],
    right: &[(T, u32)],
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<(usize, usize, T)>, Cancelled> {
    let mut groups: BTreeMap<T, Vec<(usize, usize)>> = BTreeMap::new();
    for (i, j, union) in choices {
        cancellation.check()?;
        let same = groups.entry(union).or_default();
        let mut redundant = false;
        for &(a, b) in same.iter() {
            cancellation.check()?;
            if operations.is_superset(&left[i].0, &left[a].0)
                && operations.is_superset(&right[j].0, &right[b].0)
            {
                redundant = true;
                break;
            }
        }
        if redundant {
            continue;
        }
        // Keep cancellation polling independent of the size of a join group.
        let mut kept = Vec::with_capacity(same.len() + 1);
        for &(a, b) in same.iter() {
            cancellation.check()?;
            if !(operations.is_superset(&left[a].0, &left[i].0)
                && operations.is_superset(&right[b].0, &right[j].0))
            {
                kept.push((a, b));
            }
        }
        kept.push((i, j));
        *same = kept;
    }
    Ok(groups
        .into_iter()
        .flat_map(|(union, positions)| {
            positions
                .into_iter()
                .map(move |(i, j)| (i, j, union.clone()))
        })
        .collect())
}

/// A two-by-two exchange preserves row/column counts and the selected union.
/// If the crossing intersections contain the two occupied values, with at
/// least one strict increase, the crossed configuration dominates the original.
/// Upward-closed admissibility keeps it valid. A finite partial order ensures
/// that repeated exchanges reach an undominated alignment, so skipping these
/// alignments preserves the closure for custom algebras as well as label sets.
fn dominated_exchanges<T: Eq>(
    values: &[T],
    compatible: &[bool],
    rows: usize,
    columns: usize,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<(usize, usize)>, Cancelled> {
    let mut cells = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            cancellation.check()?;
            let index = row * columns + column;
            if compatible[index] {
                cells.push((row, column, index));
                // This is an optional proof shortcut. Avoid quadratic
                // precomputation and extra storage for large matrices; the
                // ordinary enumeration remains exact.
                if cells.len() > 256 {
                    return Ok(Vec::new());
                }
            }
        }
    }
    let mut conflicts = Vec::new();
    for (position, &(i, j, x)) in cells.iter().enumerate() {
        for &(k, l, y) in &cells[position + 1..] {
            cancellation.check()?;
            if i == k || j == l {
                continue;
            }
            let (u, v) = (i * columns + l, k * columns + j);
            if !compatible[u] || !compatible[v] {
                continue;
            }
            let (px, py, pu, pv) = (&values[x], &values[y], &values[u], &values[v]);
            if (operations.is_superset(pv, px)
                && operations.is_superset(pu, py)
                && (pv != px || pu != py))
                || (operations.is_superset(pu, px)
                    && operations.is_superset(pv, py)
                    && (pu != px || pv != py))
            {
                conflicts.push((x, y));
            }
        }
    }
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bits {
        dual: bool,
    }
    impl Operations<u8> for Bits {
        fn union(&self, a: &u8, b: &u8) -> u8 {
            if self.dual { a & b } else { a | b }
        }
        fn intersection(&self, a: &u8, b: &u8) -> u8 {
            if self.dual { a | b } else { a & b }
        }
        fn is_superset(&self, a: &u8, b: &u8) -> bool {
            if self.dual { a & b == *a } else { a & b == *b }
        }
    }

    fn occurrences(row: &Configuration<u8>) -> Vec<u8> {
        row.multiplicities()
            .iter()
            .flat_map(|&(value, count)| std::iter::repeat_n(value, count as usize))
            .collect()
    }

    #[test]
    fn pruned_combinations_cover_all_degree_two_choices_in_both_lattice_orders() {
        let rows: Vec<_> = (0..8)
            .flat_map(|a| (a..8).map(move |b| Configuration::new(vec![(a, 1), (b, 1)]).unwrap()))
            .collect();
        let cancellation = Cancellation::default();
        for dual in [false, true] {
            let operations = Bits { dual };
            for (index, left) in rows.iter().enumerate() {
                for right in &rows[..=index] {
                    let generated = combine(
                        left,
                        right,
                        &operations,
                        &cancellation,
                        false,
                        &super::super::history::History::new(false),
                        None,
                    )
                    .unwrap();
                    let a = occurrences(left);
                    let b = occurrences(right);
                    // Independent oracle: choose the union ports explicitly;
                    // the only other two ports must be intersected. Include
                    // parents because combinations may correctly be discarded
                    // as dominated by a parent, even for comparable operands.
                    for i in 0..2 {
                        for j in 0..2 {
                            let offered = [
                                operations.union(&a[i], &b[j]),
                                operations.intersection(&a[1 - i], &b[1 - j]),
                            ];
                            assert!(
                                std::iter::once(left)
                                    .chain(std::iter::once(right))
                                    .chain(
                                        generated.iter().map(|candidate| &candidate.configuration)
                                    )
                                    .any(|target| {
                                        let target = occurrences(target);
                                        (operations.is_superset(&target[0], &offered[0])
                                            && operations.is_superset(&target[1], &offered[1]))
                                            || (operations.is_superset(&target[0], &offered[1])
                                                && operations.is_superset(&target[1], &offered[0]))
                                    })
                            );
                        }
                    }
                    let traced = combine(
                        left,
                        right,
                        &operations,
                        &cancellation,
                        true,
                        &super::super::history::History::new(false),
                        None,
                    )
                    .unwrap();
                    for candidate in traced {
                        let recipe = candidate.combination.unwrap();
                        let l = recipe.left.multiplicities();
                        let r = recipe.right.multiplicities();
                        let (i, j) = recipe.union_parts;
                        let mut reconstructed = vec![(operations.union(&l[i].0, &r[j].0), 1)];
                        reconstructed.extend(
                            recipe
                                .intersection_counts
                                .iter()
                                .enumerate()
                                .filter(|(_, count)| **count > 0)
                                .map(|(cell, &count)| {
                                    (
                                        operations.intersection(
                                            &l[cell / r.len()].0,
                                            &r[cell % r.len()].0,
                                        ),
                                        count,
                                    )
                                }),
                        );
                        assert_eq!(
                            Configuration::new(reconstructed).unwrap(),
                            candidate.configuration
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn previously_generated_configurations_are_filtered_before_local_pruning() {
        let left = Configuration::new(vec![(1, 1), (2, 1)]).unwrap();
        let right = left.clone();
        let operations = Bits { dual: false };
        let cancellation = Cancellation::default();
        let generated = combine(
            &left,
            &right,
            &operations,
            &cancellation,
            true,
            &super::super::history::History::new(false),
            None,
        )
        .unwrap();
        assert!(!generated.is_empty());
        let mut known = super::super::history::History::new(false);
        for candidate in generated {
            known.insert(candidate.configuration);
        }
        assert!(
            combine(
                &left,
                &right,
                &operations,
                &cancellation,
                true,
                &known,
                None
            )
            .unwrap()
            .is_empty()
        );
    }
}
