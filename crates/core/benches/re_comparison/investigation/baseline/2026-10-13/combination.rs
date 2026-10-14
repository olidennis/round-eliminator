//! LLM: Counted pair combinations and dominance pruning of intersection alignments.

use super::{antichain::dominates, engine::Operations};
use crate::{
    algorithms::configurations::visit_alignments_with,
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::collections::BTreeSet;

/// LLM: A recorded pairwise derivation: parent configurations, the chosen union positions, and
/// LLM: row-major counts for remaining intersections. The indices refer to the parents' sorted
/// LLM: distinct values.
pub struct Combination<T> {
    pub left: Configuration<T>,
    pub right: Configuration<T>,
    pub union_parts: (usize, usize),
    pub intersection_counts: Vec<u32>,
}

/// LLM: The optional callback invoked on the requester when a derived configuration is
/// LLM: admitted. Its parents and alignment describe the derivation; later dominance pruning
/// LLM: can still remove the admitted row.
pub type DerivationObserver<'a, T> = dyn FnMut(&Configuration<T>, Combination<T>) + 'a;

/// LLM: A generated counted configuration and its optional derivation. The closure filters
/// LLM: duplicate and dominated candidates before admission; a generated candidate need not
/// LLM: survive in the final maximum.
pub(super) struct Candidate<T> {
    pub(super) configuration: Configuration<T>,
    pub(super) combination: Option<Box<Combination<T>>>,
}

/// LLM: Generate undominated consequences of two same-degree parents by selecting one join and
/// LLM: meeting the remaining counted occurrences. Used by each closure generation pass;
/// LLM: duplicate history and optional derivations are retained.
pub(super) fn combine<T: Clone + Ord>(
    left: &Configuration<T>,
    right: &Configuration<T>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
    tracking: bool,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    // LLM: Prepare `a` for the following computation; it stores the a for this stage.
    let a = left.multiplicities();
    let b = right.multiplicities();
    // LLM: Each intersection is computed once for all union choices and alignments.

    let mut intersections = Vec::with_capacity(a.len() * b.len());
    for (x, _) in a {
        for (y, _) in b {
            cancellation.check()?;
            intersections.push(operations.intersection(x, y));
        }
    }
    // LLM: Record the admissible pairings used to restrict exact matching or support search.
    let compatible: Vec<_> = intersections
        .iter()
        .map(|v| operations.is_admissible(v))
        .collect();
    // LLM: Prepare `conflicts` for the following computation; it stores the conflicts for this
    // LLM: stage.
    let conflicts = dominated_exchanges(
        &intersections,
        &compatible,
        a.len(),
        b.len(),
        operations,
        cancellation,
    )?;
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let mut result: Vec<Candidate<T>> = Vec::new();
    let mut seen = BTreeSet::new();
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    for (i, (x, _)) in a.iter().enumerate() {
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for (j, (y, _)) in b.iter().enumerate() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if operations.is_superset(x, y) || operations.is_superset(y, x) {
                continue;
            }
            let union = operations.union(x, y);
            if !operations.is_admissible(&union) {
                continue;
            }
            // LLM: Prepare counted rows or remaining row capacities for the next exact
            // LLM: comparison.
            let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
            let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
            rows[i] -= 1;
            columns[j] -= 1;
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            visit_alignments_with(&rows, &columns, &compatible, cancellation, |counts| {
                // LLM: Reject an infeasible capacity state before exploring its completions.
                if conflicts
                    .iter()
                    .any(|&(x, y)| counts[x] > 0 && counts[y] > 0)
                {
                    return Ok(());
                }
                let mut parts = vec![(union.clone(), 1)];
                // LLM: Retain the newly constructed choices in the result for this stage.
                parts.extend(
                    counts
                        .iter()
                        .zip(&intersections)
                        .filter(|(n, _)| **n > 0)
                        .map(|(&n, value)| (value.clone(), n)),
                );
                // LLM: Prepare `configuration` for the following computation; it stores the
                // LLM: configuration for this stage.
                let configuration =
                    Configuration::new(parts).expect("combination preserves degree");
                if !seen.insert(configuration.clone()) {
                    return Ok(());
                }
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                if dominates(left, &configuration, operations, cancellation)?
                    || dominates(right, &configuration, operations, cancellation)?
                {
                    return Ok(());
                }
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for old in &result {
                    if dominates(&old.configuration, &configuration, operations, cancellation)? {
                        return Ok(());
                    }
                }
                let mut kept = Vec::new();
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for old in result.drain(..) {
                    if !dominates(&configuration, &old.configuration, operations, cancellation)? {
                        kept.push(old)
                    }
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                kept.push(Candidate {
                    configuration,
                    combination: tracking.then(|| {
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
                        Box::new(Combination {
                            left: left.clone(),
                            right: right.clone(),
                            union_parts: (i, j),
                            intersection_counts: counts.to_vec(),
                        })
                    }),
                });
                // LLM: Update the current working state before continuing this operation.
                result = kept;
                Ok(())
            })?;
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(result)
}

/// LLM: Find pairs of positive matrix cells whose two-by-two exchange yields strictly stronger
/// LLM: meets. Used to reject dominated alignment branches while preserving row/column
/// LLM: margins.
fn dominated_exchanges<T: Eq>(
    values: &[T],
    compatible: &[bool],
    rows: usize,
    columns: usize,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<(usize, usize)>, Cancelled> {
    // LLM: Prepare `cells` for the following computation; it stores the cells for this stage.
    let mut cells = Vec::new();
    for row in 0..rows {
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for column in 0..columns {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let index = row * columns + column;
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if compatible[index] {
                // LLM: Retain the newly constructed choices in the result for this stage.
                cells.push((row, column, index));
                // LLM: This is an optional proof shortcut. Avoid quadratic
                // LLM: precomputation and extra storage for large matrices; the
                // LLM: ordinary enumeration remains exact.

                if cells.len() > 256 {
                    return Ok(Vec::new());
                }
            }
        }
    }
    // LLM: Prepare `conflicts` for the following computation; it stores the conflicts for this
    // LLM: stage.
    let mut conflicts = Vec::new();
    for (position, &(i, j, x)) in cells.iter().enumerate() {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for &(k, l, y) in &cells[position + 1..] {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if i == k || j == l {
                continue;
            }
            let (u, v) = (i * columns + l, k * columns + j);
            if !compatible[u] || !compatible[v] {
                continue;
            }
            // LLM: Update the current working state before continuing this operation.
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
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(conflicts)
}
