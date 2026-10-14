//! LLM: Visit count matrices with prescribed row and column sums and allowed cells.
//! LLM: Repeated occurrences remain counts; neither permutations nor labels expand.

use crate::computation::{Cancellation, Cancelled};

/// LLM: One count-matrix backtracking decision: row-major cell index, feasible count
/// LLM: alternatives, and the last chosen count. The chosen value lets the next iteration
/// LLM: restore both margins before trying another alternative.
struct Frame {
    cell: usize,
    values: std::ops::RangeInclusive<u32>,
    chosen: Option<u32>,
}

/// LLM: Visit row-major count matrices with the given margins and allowed cells. Used by joint
/// LLM: products and pairwise maximization without expanding repeated occurrences.
pub(crate) fn visit_alignments_with(
    rows: &[u32],
    columns: &[u32],
    compatible: &[bool],
    cancellation: &Cancellation,
    visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    visit_alignments_avoiding_with(rows, columns, compatible, &[], cancellation, visitor)
}

/// LLM: Visit the same exact alignments while forbidding caller-specified pairs of positive
/// LLM: cells. Used by maximization to skip branches with a dominance proof.
pub(crate) fn visit_alignments_avoiding_with(
    rows: &[u32],
    columns: &[u32],
    compatible: &[bool],
    conflicts: &[(usize, usize)],
    cancellation: &Cancellation,
    mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    assert_eq!(compatible.len(), rows.len() * columns.len());
    if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
        != columns.iter().map(|&n| u64::from(n)).sum::<u64>()
    {
        return Ok(());
    }
    // LLM: Prepare `search` for the following computation; it stores the search for this
    // LLM: stage.
    let mut search = Search {
        rows: rows.to_vec(),
        columns: columns.to_vec(),
        compatible,
        counts: vec![0; compatible.len()],
        cancellation,
    };
    // LLM: Search fills cells in increasing order. Only the later cell needs to
    // LLM: check the earlier one. Ordinary visitation allocates no conflict table.

    let mut forbidden = if conflicts.is_empty() {
        Vec::new()
    } else {
        vec![Vec::new(); compatible.len()]
    };
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for &(a, b) in conflicts {
        cancellation.check()?;
        assert!(a < compatible.len() && b < compatible.len());
        forbidden[a.max(b)].push(a.min(b));
    }
    let mut stack = Vec::new();
    search.advance(0, &mut stack, &mut visitor)?;
    // LLM: Resume the next backtracking decision, restoring previous choices before trying
    // LLM: another feasible branch.
    while let Some(frame) = stack.last_mut() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let row = frame.cell / columns.len();
        let column = frame.cell % columns.len();
        if let Some(previous) = frame.chosen.take() {
            search.rows[row] += previous;
            search.columns[column] += previous;
            search.counts[frame.cell] = 0;
        }
        // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
        // LLM: traversal to backtrack or continue.
        let Some(count) = frame.values.next() else {
            stack.pop();
            continue;
        };
        search.rows[row] -= count;
        search.columns[column] -= count;
        search.counts[frame.cell] = count;
        frame.chosen = Some(count);
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if count > 0
            && !forbidden.is_empty()
            && forbidden[frame.cell]
                .iter()
                .any(|&other| search.counts[other] > 0)
        {
            // LLM: The next iteration restores this assignment's margins as usual.

            continue;
        }
        // LLM: Prepare `next` for the following computation; it stores the next for this
        // LLM: stage.
        let next = frame.cell + 1;
        search.advance(next, &mut stack, &mut visitor)?;
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(())
}

/// LLM: The mutable count-matrix state: remaining row/column margins, allowed cells, current
/// LLM: row-major counts, and cancellation. It is reused while enumerating alignments.
struct Search<'a> {
    rows: Vec<u32>,
    columns: Vec<u32>,
    compatible: &'a [bool],
    counts: Vec<u32>,
    cancellation: &'a Cancellation,
}

impl Search<'_> {
    /// LLM: Compute the feasible count interval for one cell from remaining capacities and
    /// LLM: what later cells can still supply. Used by iterative alignment backtracking.
    fn bounds(&self, cell: usize) -> Option<std::ops::RangeInclusive<u32>> {
        // LLM: Map the row-major cell to its margins and bound its count by their remaining
        // LLM: capacity.
        let width = self.columns.len();
        let row = cell / width;
        let column = cell % width;
        let maximum = if self.compatible[cell] {
            self.rows[row].min(self.columns[column])
        } else {
            0
        };
        // LLM: Demand that later compatible cells cannot fill is forced here.

        let later_columns: u64 = ((column + 1)..width)
            .filter(|&c| self.compatible[row * width + c])
            .map(|c| u64::from(self.columns[c]))
            .sum();
        let later_rows: u64 = ((row + 1)..self.rows.len())
            .filter(|&r| self.compatible[r * width + column])
            .map(|r| u64::from(self.rows[r]))
            .sum();
        // LLM: Establish the feasible bound for this local search before exploring or
        // LLM: allocating alternatives.
        let minimum = u64::from(self.rows[row])
            .saturating_sub(later_columns)
            .max(u64::from(self.columns[column]).saturating_sub(later_rows));
        (minimum <= u64::from(maximum)).then_some(minimum as u32..=maximum)
    }

    /// LLM: Skip forced-zero cells, schedule the next branching cell, or visit a completed
    /// LLM: balanced matrix. Used by the reusable alignment search.
    fn advance(
        &self,
        start: usize,
        stack: &mut Vec<Frame>,
        visitor: &mut impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        // LLM: Skip forced-zero cells; suspend at the first cell with a nonzero alternative.
        for cell in start..self.counts.len() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            self.cancellation.check()?;
            let Some(values) = self.bounds(cell) else {
                return Ok(());
            };
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if *values.end() != 0 {
                // LLM: Retain the newly constructed choices in the result for this stage.
                stack.push(Frame {
                    cell,
                    values,
                    chosen: None,
                });
                return Ok(());
            }
        }
        // LLM: Visit a completed matrix only when every row and column margin has been
        // LLM: exhausted.
        if self.rows.iter().chain(&self.columns).all(|&n| n == 0) {
            visitor(&self.counts)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LLM: Regression test: verify that visits exactly the feasible matrices and empty
    /// LLM: alignment. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn visits_exactly_the_feasible_matrices_and_empty_alignment() {
        // LLM: Prepare `matrices` for the following computation; it stores the matrices for
        // LLM: this stage.
        let mut matrices = Vec::new();
        visit_alignments_with(
            &[1, 2],
            &[2, 1],
            &[true; 4],
            &Cancellation::default(),
            |m| {
                matrices.push(m.to_vec());
                Ok(())
            },
        )
        .unwrap();
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        matrices.sort();
        assert_eq!(matrices, [vec![0, 1, 2, 0], vec![1, 0, 1, 1]]);
        matrices.clear();
        // LLM: Retain the newly constructed choices in the result for this stage.
        visit_alignments_with(
            &[1, 2],
            &[2, 1],
            &[false, true, true, false],
            &Cancellation::default(),
            |m| {
                matrices.push(m.to_vec());
                Ok(())
            },
        )
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(matrices, [vec![0, 1, 2, 0]]);
        let mut visits = 0;
        visit_alignments_with(&[], &[], &[], &Cancellation::default(), |m| {
            assert!(m.is_empty());
            visits += 1;
            Ok(())
        })
        .unwrap();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(visits, 1);
    }

    /// LLM: Regression test: verify that early conflicts equal filtering complete matrices.
    /// LLM: Used by the test suite to protect this behavior while implementations change.
    #[test]
    fn early_conflicts_equal_filtering_complete_matrices() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let cell_pairs: Vec<_> = (0..4)
            .flat_map(|a| (a + 1..4).map(move |b| (a, b)))
            .collect();
        // LLM: Every 2x2 margin up to two, every allowed-cell mask and every
        // LLM: conflict mask: 19,456 cases, including zero margins and no solutions.

        for a in 0..=2 {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for b in 0..=2 {
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for c in 0..=2 {
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for d in 0..=2 {
                        // LLM: Handle this condition before continuing with the remaining
                        // LLM: search alternatives.
                        if a + b != c + d {
                            continue;
                        }
                        // LLM: Visit the remaining alternatives for this stage, stopping when
                        // LLM: the required search condition is settled.
                        for allowed_mask in 0u32..16 {
                            // LLM: Prepare `allowed` for the following computation; it stores
                            // LLM: the allowed for this stage.
                            let allowed: Vec<_> =
                                (0..4).map(|i| allowed_mask & (1 << i) != 0).collect();
                            let mut complete = Vec::new();
                            // LLM: Retain the newly constructed choices in the result for this
                            // LLM: stage.
                            visit_alignments_with(
                                &[a, b],
                                &[c, d],
                                &allowed,
                                &cancellation,
                                |matrix| {
                                    complete.push(matrix.to_vec());
                                    Ok(())
                                },
                            )
                            .unwrap();
                            // LLM: Visit the remaining alternatives for this stage, stopping
                            // LLM: when the required search condition is settled.
                            for conflict_mask in 0u32..64 {
                                // LLM: Prepare `conflicts` for the following computation; it
                                // LLM: stores the conflicts for this stage.
                                let conflicts: Vec<_> = cell_pairs
                                    .iter()
                                    .copied()
                                    .enumerate()
                                    .filter_map(|(i, pair)| {
                                        (conflict_mask & (1 << i) != 0).then_some(pair)
                                    })
                                    .collect();
                                // LLM: Construct the independent reference result for this
                                // LLM: fixture.
                                let expected: Vec<_> = complete
                                    .iter()
                                    .filter(|matrix| {
                                        conflicts
                                            .iter()
                                            .all(|&(x, y)| matrix[x] == 0 || matrix[y] == 0)
                                    })
                                    .cloned()
                                    .collect();
                                // LLM: Evaluate the implementation under test on the same
                                // LLM: fixture.
                                let mut actual = Vec::new();
                                visit_alignments_avoiding_with(
                                    &[a, b],
                                    &[c, d],
                                    &allowed,
                                    &conflicts,
                                    &cancellation,
                                    |matrix| {
                                        actual.push(matrix.to_vec());
                                        Ok(())
                                    },
                                )
                                .unwrap();
                                // LLM: Compare the implementation result with the
                                // LLM: independently prepared reference for this fixture.
                                assert_eq!(actual, expected);
                            }
                        }
                    }
                }
            }
        }
    }

    /// LLM: Regression test: verify that conflicts support reversed indices self conflicts and
    /// LLM: cancellation. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn conflicts_support_reversed_indices_self_conflicts_and_cancellation() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let mut actual = Vec::new();
        // LLM: Retain the newly constructed choices in the result for this stage.
        visit_alignments_avoiding_with(
            &[1, 1],
            &[1, 1],
            &[true; 4],
            &[(3, 0)],
            &cancellation,
            |matrix| {
                actual.push(matrix.to_vec());
                Ok(())
            },
        )
        .unwrap();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert_eq!(actual, [vec![0, 1, 1, 0]]);
        cancellation.cancel();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            visit_alignments_avoiding_with(
                &[1, 1],
                &[1, 1],
                &[true; 4],
                &[(0, 0)],
                &cancellation,
                |_| Ok(()),
            )
            .is_err()
        );
    }
}
