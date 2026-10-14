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
    visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    AlignmentSearch::new(compatible, conflicts, cancellation)?.visit_with(rows, columns, visitor)
}

/// LLM: Reusable count-matrix enumeration buffers, conflict indices, and explicit backtracking
/// LLM: frames. The visitor borrows each complete matrix; conflicting positive cells are
/// LLM: rejected before exploring their completions.
pub(crate) struct AlignmentSearch<'a> {
    search: Search<'a>,
    forbidden: Vec<Vec<usize>>,
    stack: Vec<Frame>,
}

impl<'a> AlignmentSearch<'a> {
    /// LLM: Allocate reusable alignment buffers and orient each conflict toward its later
    /// LLM: cell. Used before visiting many choices of margins.
    pub(crate) fn new(
        compatible: &'a [bool],
        conflicts: &[(usize, usize)],
        cancellation: &'a Cancellation,
    ) -> Result<Self, Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        // LLM: Search fills cells in increasing order. Only the later cell needs to
        // LLM: check the earlier one. Ordinary visitation allocates no conflict table.

        let mut forbidden = if conflicts.is_empty() {
            Vec::new()
        } else {
            vec![Vec::new(); compatible.len()]
        };
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for &(a, b) in conflicts {
            cancellation.check()?;
            assert!(a < compatible.len() && b < compatible.len());
            forbidden[a.max(b)].push(a.min(b));
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            search: Search {
                rows: Vec::new(),
                columns: Vec::new(),
                compatible,
                counts: vec![0; compatible.len()],
                cancellation,
            },
            forbidden,
            stack: Vec::new(),
        })
    }

    /// LLM: Enumerate exact feasible count matrices using reusable heap frames and restore
    /// LLM: margins while backtracking. Used by combination and product construction; each
    /// LLM: callback borrows the current matrix.
    pub(crate) fn visit_with(
        &mut self,
        rows: &[u32],
        columns: &[u32],
        mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.search.cancellation.check()?;
        assert_eq!(self.search.compatible.len(), rows.len() * columns.len());
        if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
            != columns.iter().map(|&n| u64::from(n)).sum::<u64>()
        {
            return Ok(());
        }
        self.search.rows.clear();
        // LLM: Retain the newly constructed choices in the result for this stage.
        self.search.rows.extend_from_slice(rows);
        self.search.columns.clear();
        self.search.columns.extend_from_slice(columns);
        self.search.counts.fill(0);
        self.stack.clear();
        self.search.advance(0, &mut self.stack, &mut visitor)?;
        // LLM: Resume the next backtracking decision, restoring previous choices before trying
        // LLM: another feasible branch.
        while let Some(frame) = self.stack.last_mut() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            self.search.cancellation.check()?;
            let row = frame.cell / self.search.columns.len();
            let column = frame.cell % self.search.columns.len();
            if let Some(previous) = frame.chosen.take() {
                self.search.rows[row] += previous;
                self.search.columns[column] += previous;
                self.search.counts[frame.cell] = 0;
            }
            // LLM: Pop an exhausted decision; its margins have already been restored.
            let Some(count) = frame.values.next() else {
                self.stack.pop();
                continue;
            };
            self.search.rows[row] -= count;
            self.search.columns[column] -= count;
            self.search.counts[frame.cell] = count;
            frame.chosen = Some(count);
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if count > 0
                && !self.forbidden.is_empty()
                && self.forbidden[frame.cell]
                    .iter()
                    .any(|&other| self.search.counts[other] > 0)
            {
                // LLM: The next iteration restores this assignment's margins as usual.

                continue;
            }
            // LLM: Prepare `next` for the following computation; it stores the next for this
            // LLM: stage.
            let next = frame.cell + 1;
            self.search.advance(next, &mut self.stack, &mut visitor)?;
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(())
    }
}

/// LLM: Connected-component IDs and row-minus-column capacity balances of the allowed-cell
/// LLM: graph. They give a necessary feasibility test when consuming one occurrence on each
/// LLM: side; passing still requires exact search.
pub(crate) struct AlignmentComponents {
    rows: Vec<usize>,
    columns: Vec<usize>,
    nonzero: Vec<(usize, i128)>,
}

impl AlignmentComponents {
    /// LLM: Summarize allowed-cell connected components and their capacity imbalance. Used to
    /// LLM: cheaply reject impossible consumed union-operand pairs.
    pub(crate) fn new(
        rows: &[u32],
        columns: &[u32],
        compatible: &[bool],
        cancellation: &Cancellation,
    ) -> Result<Self, Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        assert_eq!(compatible.len(), rows.len() * columns.len());
        let mut components =
            petgraph::unionfind::UnionFind::<usize>::new(rows.len() + columns.len());
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for i in 0..rows.len() {
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for j in 0..columns.len() {
                cancellation.check()?;
                if compatible[i * columns.len() + j] {
                    components.union(i, rows.len() + j);
                }
            }
        }
        // LLM: Record the connected component of each margin before accumulating
        // LLM: row-minus-column balances.
        let row_components: Vec<_> = (0..rows.len()).map(|i| components.find_mut(i)).collect();
        let column_components: Vec<_> = (0..columns.len())
            .map(|j| components.find_mut(rows.len() + j))
            .collect();
        let mut balance = vec![0i128; rows.len() + columns.len()];
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for (&i, &count) in row_components.iter().zip(rows) {
            cancellation.check()?;
            balance[i] += i128::from(count);
        }
        for (&j, &count) in column_components.iter().zip(columns) {
            cancellation.check()?;
            balance[j] -= i128::from(count);
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            rows: row_components,
            columns: column_components,
            nonzero: balance
                .into_iter()
                .enumerate()
                .filter(|&(_, b)| b != 0)
                .collect(),
        })
    }

    /// LLM: Test whether consuming one count from the chosen row and column can balance every
    /// LLM: connected component. Used before exact intersection alignment enumeration.
    pub(crate) fn allows_removing(&self, row: usize, column: usize) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self.nonzero.as_slice() {
            [] => self.rows[row] == self.columns[column],
            [(a, x), (b, y)] => {
                (*x == 1 && *y == -1 && self.rows[row] == *a && self.columns[column] == *b)
                    || (*y == 1 && *x == -1 && self.rows[row] == *b && self.columns[column] == *a)
            }
            _ => false,
        }
    }
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

    /// LLM: Regression test: verify that reused search restores margins counts and frames
    /// LLM: after early exit. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn reused_search_restores_margins_counts_and_frames_after_early_exit() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let mut search = AlignmentSearch::new(&[true; 4], &[(3, 0)], &cancellation).unwrap();
        assert!(
            search
                .visit_with(&[1, 1], &[1, 1], |_| Err(Cancelled))
                .is_err()
        );
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for (rows, columns) in [([1, 1], [1, 1]), ([1, 2], [2, 1]), ([0, 0], [0, 0])] {
            // LLM: Construct the independent reference result for this fixture.
            let mut expected = Vec::new();
            visit_alignments_avoiding_with(
                &rows,
                &columns,
                &[true; 4],
                &[(3, 0)],
                &cancellation,
                |m| {
                    expected.push(m.to_vec());
                    Ok(())
                },
            )
            .unwrap();
            // LLM: Evaluate the implementation under test on the same fixture.
            let mut actual = Vec::new();
            search
                .visit_with(&rows, &columns, |m| {
                    actual.push(m.to_vec());
                    Ok(())
                })
                .unwrap();
            assert_eq!(actual, expected);
        }
    }

    /// LLM: Regression test: verify that component rejection never discards a feasible reduced
    /// LLM: alignment. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn component_rejection_never_discards_a_feasible_reduced_alignment() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        // LLM: Every 2x3 allowed-cell graph, each possible consumed row/column,
        // LLM: including isolated vertices and unbalanced disconnected components.

        for mask in 0..64 {
            // LLM: Prepare `allowed` for the following computation; it stores the allowed for
            // LLM: this stage.
            let allowed: Vec<_> = (0..6).map(|i| mask & (1 << i) != 0).collect();
            for original in [[1, 2], [2, 1]] {
                // LLM: Prepare `components` for the following computation; it stores the
                // LLM: components for this stage.
                let components =
                    AlignmentComponents::new(&original, &[1; 3], &allowed, &cancellation).unwrap();
                // LLM: Check each relevant row independently, preserving its exact degree and
                // LLM: condensed choices.
                for i in 0..2 {
                    // LLM: Check each relevant row independently, preserving its exact degree
                    // LLM: and condensed choices.
                    for j in 0..3 {
                        // LLM: Prepare counted rows or remaining row capacities for the next
                        // LLM: exact comparison.
                        let mut rows = original;
                        let mut columns = [1; 3];
                        rows[i] -= 1;
                        columns[j] -= 1;
                        let mut feasible = false;
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
                        visit_alignments_with(&rows, &columns, &allowed, &cancellation, |_| {
                            feasible = true;
                            Ok(())
                        })
                        .unwrap();
                        assert!(!feasible || components.allows_removing(i, j));
                    }
                }
            }
        }
        // LLM: Update the current working state before continuing this operation.
        cancellation.cancel();
        assert!(AlignmentComponents::new(&[], &[], &[], &cancellation).is_err());
    }

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
