//! LLM: Counted pair combinations and dominance pruning of intersection alignments.

use super::{antichain::dominates, engine::Operations};
use crate::{
    algorithms::configurations::visit_alignments_avoiding_with,
    computation::{Cancellation, Cancelled},
    model::Configuration,
};
use std::collections::{BTreeMap, BTreeSet};

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
    known: &BTreeSet<Configuration<T>>,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    // LLM: Read the parents' distinct values and counts, then collect useful incomparable
    // LLM: union-operand choices.
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
    // LLM: Identify exchange patterns that prove an alignment dominated before its full
    // LLM: completion.
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
    let mut choices = Vec::new();
    // LLM: Consider possible union operands; comparable operands cannot produce anything
    // LLM: stronger than a parent.
    for (i, (x, _)) in a.iter().enumerate() {
        // LLM: Retain only admissible joins of incomparable values before preparing their
        // LLM: remaining intersections.
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
            // LLM: Retain the newly constructed choices in the result for this stage.
            choices.push((i, j, union));
        }
    }
    // LLM: Dense alignments benefit from pruning repeated joins (notably finite
    // LLM: lattices allowing bottom values). Sparse set problems reject alignments
    // LLM: cheaply, so grouping costs more than it saves on the measured fixtures.
    // LLM: Traced runs keep the original operand order and intermediate admissions.

    if !tracking && compatible.iter().all(|&allowed| allowed) {
        choices = prune_union_choices(choices, a, b, operations, cancellation)?;
    }
    // LLM: Consider possible union operands; comparable operands cannot produce anything
    // LLM: stronger than a parent.
    for (i, j, union) in choices {
        // LLM: Prepare counted rows or remaining row capacities for the next exact comparison.
        let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
        let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
        rows[i] -= 1;
        columns[j] -= 1;
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        visit_alignments_avoiding_with(
            &rows,
            &columns,
            &compatible,
            &conflicts,
            cancellation,
            |counts| {
                // LLM: Form a counted child from its selected join and every positive
                // LLM: intersection count.
                let mut parts = vec![(union.clone(), 1)];
                parts.extend(
                    counts
                        .iter()
                        .zip(&intersections)
                        .filter(|(n, _)| **n > 0)
                        .map(|(&n, value)| (value.clone(), n)),
                );
                // LLM: Canonicalize equal child values and their counts before duplicate and
                // LLM: dominance checks.
                let configuration =
                    Configuration::new(parts).expect("combination preserves degree");
                // LLM: Every history entry is already covered by the global antichain.
                // LLM: Check before local matching/flows, even if another parent pair
                // LLM: produced this configuration. History remains immutable until
                // LLM: this entire generation batch finishes.

                if known.contains(&configuration) {
                    return Ok(());
                }
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
            },
        )?;
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(result)
}

/// LLM: Discard a union-operand choice when a retained choice gives the same join and leaves
/// LLM: stronger operands for all remaining meets. Used before count-matrix enumeration in
/// LLM: pair combination.
fn prune_union_choices<T: Clone + Ord>(
    choices: Vec<(usize, usize, T)>,
    left: &[(T, u32)],
    right: &[(T, u32)],
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<(usize, usize, T)>, Cancelled> {
    // LLM: Index labels by equal semantic profiles for shared comparison work.
    let mut groups: BTreeMap<T, Vec<(usize, usize)>> = BTreeMap::new();
    for (i, j, union) in choices {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let same = groups.entry(union).or_default();
        let mut redundant = false;
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for &(a, b) in same.iter() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if operations.is_superset(&left[i].0, &left[a].0)
                && operations.is_superset(&right[j].0, &right[b].0)
            {
                redundant = true;
                break;
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if redundant {
            continue;
        }
        // LLM: Keep cancellation polling independent of the size of a join group.

        let mut kept = Vec::with_capacity(same.len() + 1);
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for &(a, b) in same.iter() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if !(operations.is_superset(&left[a].0, &left[i].0)
                && operations.is_superset(&right[b].0, &right[j].0))
            {
                kept.push((a, b));
            }
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        kept.push((i, j));
        *same = kept;
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(groups
        .into_iter()
        .flat_map(|(union, positions)| {
            positions
                .into_iter()
                .map(move |(i, j)| (i, j, union.clone()))
        })
        .collect())
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
    // LLM: Precompute two-cell conflicts whose exchanged meets strictly dominate the occupied
    // LLM: ones.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// LLM: A small bit-set algebra used as an independent test/benchmark value domain. Join
    /// LLM: is bitwise OR, meet is AND, and containment tests inclusion of set bits.
    struct Bits {
        dual: bool,
    }
    impl Operations<u8> for Bits {
        /// LLM: Compute the join in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn union(&self, a: &u8, b: &u8) -> u8 {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if self.dual { a & b } else { a | b }
        }
        /// LLM: Compute the meet in this representation's chosen standard or dual order.
        /// LLM: Called by the generic pairwise closure and its pruning proofs.
        fn intersection(&self, a: &u8, b: &u8) -> u8 {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if self.dual { a | b } else { a & b }
        }
        /// LLM: Test domination in this representation's chosen standard or dual order. Called
        /// LLM: by the generic pairwise closure and its pruning proofs.
        fn is_superset(&self, a: &u8, b: &u8) -> bool {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if self.dual { a & b == *a } else { a & b == *b }
        }
    }

    /// LLM: Expand a small counted fixture for an independent exhaustive oracle. Used by
    /// LLM: pair-combination tests rather than the production counted search.
    fn occurrences(row: &Configuration<u8>) -> Vec<u8> {
        // LLM: Update the current working state before continuing this operation.
        row.multiplicities()
            .iter()
            .flat_map(|&(value, count)| std::iter::repeat_n(value, count as usize))
            .collect()
    }

    /// LLM: Regression test: verify that pruned combinations cover all degree two choices in
    /// LLM: both lattice orders. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn pruned_combinations_cover_all_degree_two_choices_in_both_lattice_orders() {
        // LLM: Prepare counted rows or remaining row capacities for the next exact comparison.
        let rows: Vec<_> = (0..8)
            .flat_map(|a| (a..8).map(move |b| Configuration::new(vec![(a, 1), (b, 1)]).unwrap()))
            .collect();
        let cancellation = Cancellation::default();
        // LLM: Generate consequences of the selected parent pair under one join and the
        // LLM: remaining meets.
        for dual in [false, true] {
            // LLM: Prepare `operations` for the following computation; it stores the
            // LLM: operations for this stage.
            let operations = Bits { dual };
            for (index, left) in rows.iter().enumerate() {
                // LLM: Generate consequences of the selected parent pair under one join and
                // LLM: the remaining meets.
                for right in &rows[..=index] {
                    // LLM: Generate consequences of the selected parent pair under one join
                    // LLM: and the remaining meets.
                    let generated = combine(
                        left,
                        right,
                        &operations,
                        &cancellation,
                        false,
                        &BTreeSet::new(),
                    )
                    .unwrap();
                    // LLM: Prepare `a` for the following computation; it stores the a for this
                    // LLM: stage.
                    let a = occurrences(left);
                    let b = occurrences(right);
                    // LLM: Independent oracle: choose the union ports explicitly;
                    // LLM: the only other two ports must be intersected. Include
                    // LLM: parents because combinations may correctly be discarded
                    // LLM: as dominated by a parent, even for comparable operands.

                    for i in 0..2 {
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for j in 0..2 {
                            // LLM: Prepare `offered` for the following computation; it stores
                            // LLM: the offered for this stage.
                            let offered = [
                                operations.union(&a[i], &b[j]),
                                operations.intersection(&a[1 - i], &b[1 - j]),
                            ];
                            // LLM: Check the expected result and invariants for this fixture,
                            // LLM: including the relevant edge case.
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
                    // LLM: Generate consequences of the selected parent pair under one join
                    // LLM: and the remaining meets.
                    let traced = combine(
                        left,
                        right,
                        &operations,
                        &cancellation,
                        true,
                        &BTreeSet::new(),
                    )
                    .unwrap();
                    // LLM: Explore the next counted alternative while preserving the required
                    // LLM: total capacities.
                    for candidate in traced {
                        // LLM: Prepare `recipe` for the following computation; it stores the
                        // LLM: recipe for this stage.
                        let recipe = candidate.combination.unwrap();
                        let l = recipe.left.multiplicities();
                        let r = recipe.right.multiplicities();
                        let (i, j) = recipe.union_parts;
                        let mut reconstructed = vec![(operations.union(&l[i].0, &r[j].0), 1)];
                        // LLM: Retain the newly constructed choices in the result for this
                        // LLM: stage.
                        reconstructed.extend(
                            recipe
                                .intersection_counts
                                .iter()
                                .enumerate()
                                .filter(|(_, count)| **count > 0)
                                .map(|(cell, &count)| {
                                    // LLM: Update the current working state before continuing
                                    // LLM: this operation.
                                    (
                                        operations.intersection(
                                            &l[cell / r.len()].0,
                                            &r[cell % r.len()].0,
                                        ),
                                        count,
                                    )
                                }),
                        );
                        // LLM: Check the expected result and invariants for this fixture,
                        // LLM: including the relevant edge case.
                        assert_eq!(
                            Configuration::new(reconstructed).unwrap(),
                            candidate.configuration
                        );
                    }
                }
            }
        }
    }

    /// LLM: Regression test: verify that previously generated configurations are filtered
    /// LLM: before local pruning. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn previously_generated_configurations_are_filtered_before_local_pruning() {
        // LLM: Prepare `left` for the following computation; it stores the left for this
        // LLM: stage.
        let left = Configuration::new(vec![(1, 1), (2, 1)]).unwrap();
        let right = left.clone();
        let operations = Bits { dual: false };
        let cancellation = Cancellation::default();
        // LLM: Generate consequences of the selected parent pair under one join and the
        // LLM: remaining meets.
        let generated = combine(
            &left,
            &right,
            &operations,
            &cancellation,
            true,
            &BTreeSet::new(),
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(!generated.is_empty());
        let known = generated
            .into_iter()
            .map(|candidate| candidate.configuration)
            .collect();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            combine(&left, &right, &operations, &cancellation, true, &known)
                .unwrap()
                .is_empty()
        );
    }
}
