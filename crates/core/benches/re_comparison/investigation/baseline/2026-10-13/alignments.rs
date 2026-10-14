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
    mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    cancellation.check()?;
    assert_eq!(compatible.len(), rows.len() * columns.len());
    if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
        != columns.iter().map(|&n| u64::from(n)).sum::<u64>()
    {
        return Ok(());
    }
    let mut search = Search {
        rows: rows.to_vec(),
        columns: columns.to_vec(),
        compatible,
        counts: vec![0; compatible.len()],
        cancellation,
    };
    let mut stack = Vec::new();
    search.advance(0, &mut stack, &mut visitor)?;
    while let Some(frame) = stack.last_mut() {
        cancellation.check()?;
        let row = frame.cell / columns.len();
        let column = frame.cell % columns.len();
        if let Some(previous) = frame.chosen.take() {
            search.rows[row] += previous;
            search.columns[column] += previous;
            search.counts[frame.cell] = 0;
        }
        let Some(count) = frame.values.next() else {
            stack.pop();
            continue;
        };
        search.rows[row] -= count;
        search.columns[column] -= count;
        search.counts[frame.cell] = count;
        frame.chosen = Some(count);
        let next = frame.cell + 1;
        search.advance(next, &mut stack, &mut visitor)?;
    }
    Ok(())
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
}
