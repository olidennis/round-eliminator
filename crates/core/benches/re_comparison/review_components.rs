//! Necessary component balance after consuming one row/column occurrence.

use crate::computation::{Cancellation, Cancelled};

pub(super) struct Components {
    rows: Vec<usize>,
    columns: Vec<usize>,
    nonzero: Vec<(usize, i64)>,
}

impl Components {
    pub(super) fn new(
        rows: &[u32], columns: &[u32], compatible: &[bool],
        cancellation: &Cancellation,
    ) -> Result<Self, Cancelled> {
        let mut components = petgraph::unionfind::UnionFind::<usize>::new(rows.len() + columns.len());
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
            .map(|j| components.find_mut(rows.len() + j)).collect();
        let mut balance = vec![0i64; rows.len() + columns.len()];
        for (&i, &count) in row_components.iter().zip(rows) { balance[i] += i64::from(count); }
        for (&j, &count) in column_components.iter().zip(columns) { balance[j] -= i64::from(count); }
        Ok(Self { rows: row_components, columns: column_components,
            nonzero: balance.into_iter().enumerate().filter(|&(_, b)| b != 0).collect() })
    }

    pub(super) fn allows(&self, row: usize, column: usize) -> bool {
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
