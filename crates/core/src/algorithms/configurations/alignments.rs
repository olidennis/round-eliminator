//! Visit count matrices with prescribed row and column sums and allowed cells.
//! Repeated occurrences remain counts; neither permutations nor labels expand.

use crate::computation::{Cancellation, Cancelled};

struct Frame {
    cell: usize,
    values: std::ops::RangeInclusive<u32>,
    chosen: Option<u32>,
}

/// The visitor borrows a row-major count matrix until its callback returns.
/// A false compatibility cell must have count zero. All margins are restored
/// when backtracking. Heap frames avoid recursion proportional to matrix size.
pub(crate) fn visit_alignments_with(
    rows: &[u32],
    columns: &[u32],
    compatible: &[bool],
    cancellation: &Cancellation,
    visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    visit_alignments_avoiding_with(rows, columns, compatible, &[], cancellation, visitor)
}

/// Additionally forbid simultaneously positive counts in each pair of cells.
/// Reject a partial assignment as soon as it violates a conflict, rather than
/// visiting every completion of that branch. Conflicts index the row-major matrix;
/// their meaning belongs to the caller (for example, a dominance proof).
pub(crate) fn visit_alignments_avoiding_with(
    rows: &[u32],
    columns: &[u32],
    compatible: &[bool],
    conflicts: &[(usize, usize)],
    cancellation: &Cancellation,
    visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    AlignmentSearch::new(compatible, conflicts, cancellation)?.visit_with(rows, columns, visitor)
}

/// Reuse count/margin/frame buffers and conflict indices across margin choices.
pub(crate) struct AlignmentSearch<'a> {
    search: Search<'a>,
    forbidden: Vec<Vec<usize>>,
    stack: Vec<Frame>,
}

impl<'a> AlignmentSearch<'a> {
    pub(crate) fn new(
        compatible: &'a [bool],
        conflicts: &[(usize, usize)],
        cancellation: &'a Cancellation,
    ) -> Result<Self, Cancelled> {
        cancellation.check()?;
        // Search fills cells in increasing order. Only the later cell needs to
        // check the earlier one. Ordinary visitation allocates no conflict table.
        let mut forbidden = if conflicts.is_empty() {
            Vec::new()
        } else {
            vec![Vec::new(); compatible.len()]
        };
        for &(a, b) in conflicts {
            cancellation.check()?;
            assert!(a < compatible.len() && b < compatible.len());
            forbidden[a.max(b)].push(a.min(b));
        }
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

    pub(crate) fn visit_with(
        &mut self,
        rows: &[u32],
        columns: &[u32],
        mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        self.search.cancellation.check()?;
        assert_eq!(self.search.compatible.len(), rows.len() * columns.len());
        if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
            != columns.iter().map(|&n| u64::from(n)).sum::<u64>()
        {
            return Ok(());
        }
        self.search.rows.clear();
        self.search.rows.extend_from_slice(rows);
        self.search.columns.clear();
        self.search.columns.extend_from_slice(columns);
        self.search.counts.fill(0);
        self.stack.clear();
        self.search.advance(0, &mut self.stack, &mut visitor)?;
        while let Some(frame) = self.stack.last_mut() {
            self.search.cancellation.check()?;
            let row = frame.cell / self.search.columns.len();
            let column = frame.cell % self.search.columns.len();
            if let Some(previous) = frame.chosen.take() {
                self.search.rows[row] += previous;
                self.search.columns[column] += previous;
                self.search.counts[frame.cell] = 0;
            }
            let Some(count) = frame.values.next() else {
                self.stack.pop();
                continue;
            };
            self.search.rows[row] -= count;
            self.search.columns[column] -= count;
            self.search.counts[frame.cell] = count;
            frame.chosen = Some(count);
            if count > 0
                && !self.forbidden.is_empty()
                && self.forbidden[frame.cell]
                    .iter()
                    .any(|&other| self.search.counts[other] > 0)
            {
                // The next iteration restores this assignment's margins as usual.
                continue;
            }
            let next = frame.cell + 1;
            self.search.advance(next, &mut self.stack, &mut visitor)?;
        }
        Ok(())
    }
}

/// Necessary capacity balance in connected components of the allowed-cell graph.
/// This proof only rejects alignments; passing still requires exact enumeration.
pub(crate) struct AlignmentComponents {
    rows: Vec<usize>,
    columns: Vec<usize>,
    nonzero: Vec<(usize, i128)>,
}

impl AlignmentComponents {
    pub(crate) fn new(
        rows: &[u32],
        columns: &[u32],
        compatible: &[bool],
        cancellation: &Cancellation,
    ) -> Result<Self, Cancelled> {
        cancellation.check()?;
        assert_eq!(compatible.len(), rows.len() * columns.len());
        let mut components =
            petgraph::unionfind::UnionFind::<usize>::new(rows.len() + columns.len());
        for i in 0..rows.len() {
            for j in 0..columns.len() {
                cancellation.check()?;
                if compatible[i * columns.len() + j] {
                    components.union(i, rows.len() + j);
                }
            }
        }
        let row_components: Vec<_> = (0..rows.len()).map(|i| components.find_mut(i)).collect();
        let column_components: Vec<_> = (0..columns.len())
            .map(|j| components.find_mut(rows.len() + j))
            .collect();
        let mut balance = vec![0i128; rows.len() + columns.len()];
        for (&i, &count) in row_components.iter().zip(rows) {
            cancellation.check()?;
            balance[i] += i128::from(count);
        }
        for (&j, &count) in column_components.iter().zip(columns) {
            cancellation.check()?;
            balance[j] -= i128::from(count);
        }
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

    /// Can the components balance after removing one occurrence from each of
    /// these margins? The caller must select initially positive margins.
    pub(crate) fn allows_removing(&self, row: usize, column: usize) -> bool {
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

struct Search<'a> {
    rows: Vec<u32>,
    columns: Vec<u32>,
    compatible: &'a [bool],
    counts: Vec<u32>,
    cancellation: &'a Cancellation,
}

impl Search<'_> {
    fn bounds(&self, cell: usize) -> Option<std::ops::RangeInclusive<u32>> {
        let width = self.columns.len();
        let row = cell / width;
        let column = cell % width;
        let maximum = if self.compatible[cell] {
            self.rows[row].min(self.columns[column])
        } else {
            0
        };
        // Demand that later compatible cells cannot fill is forced here.
        let later_columns: u64 = ((column + 1)..width)
            .filter(|&c| self.compatible[row * width + c])
            .map(|c| u64::from(self.columns[c]))
            .sum();
        let later_rows: u64 = ((row + 1)..self.rows.len())
            .filter(|&r| self.compatible[r * width + column])
            .map(|r| u64::from(self.rows[r]))
            .sum();
        let minimum = u64::from(self.rows[row])
            .saturating_sub(later_columns)
            .max(u64::from(self.columns[column]).saturating_sub(later_rows));
        (minimum <= u64::from(maximum)).then_some(minimum as u32..=maximum)
    }

    fn advance(
        &self,
        start: usize,
        stack: &mut Vec<Frame>,
        visitor: &mut impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        for cell in start..self.counts.len() {
            self.cancellation.check()?;
            let Some(values) = self.bounds(cell) else {
                return Ok(());
            };
            if *values.end() != 0 {
                stack.push(Frame {
                    cell,
                    values,
                    chosen: None,
                });
                return Ok(());
            }
        }
        if self.rows.iter().chain(&self.columns).all(|&n| n == 0) {
            visitor(&self.counts)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_search_restores_margins_counts_and_frames_after_early_exit() {
        let cancellation = Cancellation::default();
        let mut search = AlignmentSearch::new(&[true; 4], &[(3, 0)], &cancellation).unwrap();
        assert!(
            search
                .visit_with(&[1, 1], &[1, 1], |_| Err(Cancelled))
                .is_err()
        );
        for (rows, columns) in [([1, 1], [1, 1]), ([1, 2], [2, 1]), ([0, 0], [0, 0])] {
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

    #[test]
    fn component_rejection_never_discards_a_feasible_reduced_alignment() {
        let cancellation = Cancellation::default();
        // Every 2x3 allowed-cell graph, each possible consumed row/column,
        // including isolated vertices and unbalanced disconnected components.
        for mask in 0..64 {
            let allowed: Vec<_> = (0..6).map(|i| mask & (1 << i) != 0).collect();
            for original in [[1, 2], [2, 1]] {
                let components =
                    AlignmentComponents::new(&original, &[1; 3], &allowed, &cancellation).unwrap();
                for i in 0..2 {
                    for j in 0..3 {
                        let mut rows = original;
                        let mut columns = [1; 3];
                        rows[i] -= 1;
                        columns[j] -= 1;
                        let mut feasible = false;
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
        cancellation.cancel();
        assert!(AlignmentComponents::new(&[], &[], &[], &cancellation).is_err());
    }

    #[test]
    fn visits_exactly_the_feasible_matrices_and_empty_alignment() {
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
        matrices.sort();
        assert_eq!(matrices, [vec![0, 1, 2, 0], vec![1, 0, 1, 1]]);
        matrices.clear();
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
        assert_eq!(matrices, [vec![0, 1, 2, 0]]);
        let mut visits = 0;
        visit_alignments_with(&[], &[], &[], &Cancellation::default(), |m| {
            assert!(m.is_empty());
            visits += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(visits, 1);
    }

    #[test]
    fn early_conflicts_equal_filtering_complete_matrices() {
        let cancellation = Cancellation::default();
        let cell_pairs: Vec<_> = (0..4)
            .flat_map(|a| (a + 1..4).map(move |b| (a, b)))
            .collect();
        // Every 2x2 margin up to two, every allowed-cell mask and every
        // conflict mask: 19,456 cases, including zero margins and no solutions.
        for a in 0..=2 {
            for b in 0..=2 {
                for c in 0..=2 {
                    for d in 0..=2 {
                        if a + b != c + d {
                            continue;
                        }
                        for allowed_mask in 0u32..16 {
                            let allowed: Vec<_> =
                                (0..4).map(|i| allowed_mask & (1 << i) != 0).collect();
                            let mut complete = Vec::new();
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
                            for conflict_mask in 0u32..64 {
                                let conflicts: Vec<_> = cell_pairs
                                    .iter()
                                    .copied()
                                    .enumerate()
                                    .filter_map(|(i, pair)| {
                                        (conflict_mask & (1 << i) != 0).then_some(pair)
                                    })
                                    .collect();
                                let expected: Vec<_> = complete
                                    .iter()
                                    .filter(|matrix| {
                                        conflicts
                                            .iter()
                                            .all(|&(x, y)| matrix[x] == 0 || matrix[y] == 0)
                                    })
                                    .cloned()
                                    .collect();
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
                                assert_eq!(actual, expected);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn conflicts_support_reversed_indices_self_conflicts_and_cancellation() {
        let cancellation = Cancellation::default();
        let mut actual = Vec::new();
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
        assert_eq!(actual, [vec![0, 1, 1, 0]]);
        cancellation.cancel();
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
