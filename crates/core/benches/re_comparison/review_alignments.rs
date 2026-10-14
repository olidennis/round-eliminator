//! Visit count matrices with prescribed row and column sums and allowed cells.
//! Repeated occurrences remain counts; neither permutations nor labels expand.

use crate::computation::{Cancellation, Cancelled};

struct Frame {
    cell: usize,
    values: std::ops::RangeInclusive<u32>,
    chosen: Option<u32>,
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
                rows: Vec::new(), columns: Vec::new(), compatible,
                counts: vec![0; compatible.len()], cancellation,
            },
            forbidden,
            stack: Vec::new(),
        })
    }

    pub(crate) fn visit_with(
        &mut self,
        rows: &[u32], columns: &[u32],
        mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        self.search.cancellation.check()?;
        assert_eq!(self.search.compatible.len(), rows.len() * columns.len());
        if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
            != columns.iter().map(|&n| u64::from(n)).sum::<u64>() {
            return Ok(());
        }
        self.search.rows.clear(); self.search.rows.extend_from_slice(rows);
        self.search.columns.clear(); self.search.columns.extend_from_slice(columns);
        self.search.counts.fill(0); self.stack.clear();
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

