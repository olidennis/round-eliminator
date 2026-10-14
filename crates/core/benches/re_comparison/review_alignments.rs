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
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for &(a, b) in conflicts {
        cancellation.check()?;
        assert!(a < compatible.len() && b < compatible.len());
        forbidden[a.max(b)].push(a.min(b));
    }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            search: Search {
                rows: Vec::new(), columns: Vec::new(), compatible,
                counts: vec![0; compatible.len()], cancellation,
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
        rows: &[u32], columns: &[u32],
        mut visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
    ) -> Result<(), Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.search.cancellation.check()?;
        assert_eq!(self.search.compatible.len(), rows.len() * columns.len());
        if rows.iter().map(|&n| u64::from(n)).sum::<u64>()
            != columns.iter().map(|&n| u64::from(n)).sum::<u64>() {
            return Ok(());
        }
        self.search.rows.clear(); self.search.rows.extend_from_slice(rows);
        self.search.columns.clear(); self.search.columns.extend_from_slice(columns);
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.search.counts.fill(0); self.stack.clear();
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
        // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
        // LLM: traversal to backtrack or continue.
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
        // LLM: Prepare `width` for the following computation; it stores the width for this
        // LLM: stage.
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
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
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
        // LLM: Reject an infeasible capacity state before exploring its completions.
        if self.rows.iter().chain(&self.columns).all(|&n| n == 0) {
            visitor(&self.counts)?;
        }
        Ok(())
    }
}

