//! Counted pair combinations and dominance pruning of intersection alignments.

use super::{antichain::dominates, engine::Operations};
use crate::{
    algorithms::configurations::visit_alignments_with,
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::collections::BTreeSet;

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
) -> Result<Vec<Candidate<T>>, Cancelled> {
    let a = left.multiplicities();
    let b = right.multiplicities();
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
            let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
            let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
            rows[i] -= 1;
            columns[j] -= 1;
            visit_alignments_with(&rows, &columns, &compatible, cancellation, |counts| {
                if conflicts
                    .iter()
                    .any(|&(x, y)| counts[x] > 0 && counts[y] > 0)
                {
                    return Ok(());
                }
                let mut parts = vec![(union.clone(), 1)];
                parts.extend(
                    counts
                        .iter()
                        .zip(&intersections)
                        .filter(|(n, _)| **n > 0)
                        .map(|(&n, value)| (value.clone(), n)),
                );
                let configuration =
                    Configuration::new(parts).expect("combination preserves degree");
                if !seen.insert(configuration.clone()) {
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
        }
    }
    Ok(result)
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
