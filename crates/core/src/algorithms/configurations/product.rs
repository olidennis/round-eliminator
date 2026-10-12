//! Join two multiset constraints through an incidence compatibility relation.
//!
//! Count matrices align parts, rather than expanding repetitions or permutations.
//! Each nonzero cell produces one condensed set of compatible label pairs.

use crate::{
    computation::{Cancellation, Cancelled},
    model::{CondensedConfiguration, Constraint, LabelSet, Part},
};

/// In order to compute the input-aware diagram, we convert a problem into the paired form.
/// To do that, we need to take the product between the input and the output constraints,
/// where we exclude pairings that are not allowed.
pub(crate) fn product_with<A: Copy + Ord, B: Copy + Ord, L: Copy + Ord>(
    left: &Constraint<A>,
    right: &Constraint<B>,
    pair: impl Fn(A, B) -> Option<L>,
    cancellation: &Cancellation,
) -> Result<Constraint<L>, Cancelled> {
    cancellation.check()?;
    let mut result = Vec::new();
    for first in left.configurations() {
        for second in right.configurations_at_degree(first.degree()) {
            cancellation.check()?;
            let columns = second.parts().len();
            let mut choices = Vec::new();
            for first_part in first.parts() {
                for second_part in second.parts() {
                    let mut labels = Vec::new();
                    for a in first_part.labels().iter() {
                        for b in second_part.labels().iter() {
                            cancellation.check()?;
                            if let Some(label) = pair(a, b) {
                                labels.push(label)
                            }
                        }
                    }
                    choices.push(LabelSet::new(labels));
                }
            }
            let rows: Vec<_> = first.parts().iter().map(Part::multiplicity).collect();
            let remaining: Vec<_> = second.parts().iter().map(Part::multiplicity).collect();
            // A singleton compatible partner needs no occurrence expansion,
            // even when the input and output exponents are enormous.
            Alignments {
                columns,
                choices: &choices,
                rows,
                remaining,
                parts: Vec::new(),
                result: &mut result,
                cancellation,
            }
            .visit(0)?;
        }
    }
    Ok(Constraint::new(result))
}

/// Each search frame remembers one count choice and restores its capacities
/// before trying a sibling. Heap storage avoids call-stack growth with the
/// product of the numbers of input and output parts.
struct Frame {
    cell: usize,
    values: std::ops::RangeInclusive<u32>,
    chosen: Option<u32>,
}

struct Alignments<'a, L> {
    columns: usize,
    choices: &'a [LabelSet<L>],
    rows: Vec<u32>,
    remaining: Vec<u32>,
    parts: Vec<Part<L>>,
    result: &'a mut Vec<CondensedConfiguration<L>>,
    cancellation: &'a Cancellation,
}

impl<L: Copy + Ord> Alignments<'_, L> {
    fn bounds(&self, cell: usize) -> Option<std::ops::RangeInclusive<u32>> {
        let row = cell / self.columns;
        let column = cell % self.columns;
        let maximum = if self.choices[cell].is_empty() {
            0
        } else {
            self.rows[row].min(self.remaining[column])
        };
        // Demand which later compatible cells cannot fill must be allocated here.
        // At the last column/row this forces the exact remaining row/column count.
        let later_columns: u32 = ((column + 1)..self.columns)
            .filter(|&c| !self.choices[row * self.columns + c].is_empty())
            .map(|c| self.remaining[c])
            .sum();
        let later_rows: u32 = ((row + 1)..self.rows.len())
            .filter(|&r| !self.choices[r * self.columns + column].is_empty())
            .map(|r| self.rows[r])
            .sum();
        let minimum = self.rows[row]
            .saturating_sub(later_columns)
            .max(self.remaining[column].saturating_sub(later_rows));
        (minimum <= maximum).then_some(minimum..=maximum)
    }

    /// Skip forced zero cells without allocating frames. Empty degree-zero
    /// products reach the terminal case directly, without dividing by zero.
    fn advance(&mut self, start: usize, stack: &mut Vec<Frame>) -> Result<(), Cancelled> {
        for cell in start..self.choices.len() {
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
        if self
            .rows
            .iter()
            .chain(&self.remaining)
            .all(|&count| count == 0)
        {
            self.result.push(
                CondensedConfiguration::new(self.parts.clone()).expect("product preserves degree"),
            );
        }
        Ok(())
    }

    fn visit(&mut self, start: usize) -> Result<(), Cancelled> {
        let mut stack = Vec::new();
        self.advance(start, &mut stack)?;
        while let Some(frame) = stack.last_mut() {
            self.cancellation.check()?;
            let row = frame.cell / self.columns;
            let column = frame.cell % self.columns;
            if let Some(previous) = frame.chosen.take() {
                self.rows[row] += previous;
                self.remaining[column] += previous;
                if previous != 0 {
                    self.parts.pop();
                }
            }
            let Some(count) = frame.values.next() else {
                stack.pop();
                continue;
            };
            self.rows[row] -= count;
            self.remaining[column] -= count;
            if count != 0 {
                self.parts.push(
                    Part::new(self.choices[frame.cell].clone(), count).expect("compatible choices"),
                );
            }
            frame.chosen = Some(count);
            let next = frame.cell + 1;
            self.advance(next, &mut stack)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::configurations::concrete_multisets;

    #[test]
    fn many_parts_use_heap_frames_and_skip_forced_zero_cells() {
        let configuration = CondensedConfiguration::new(
            (0..200u16)
                .map(|label| Part::new(LabelSet::new(vec![label]), 1).unwrap())
                .collect(),
        )
        .unwrap();
        let constraint = Constraint::new(vec![configuration]);
        let product = product_with(
            &constraint,
            &constraint,
            |a, b| (a == b).then_some((a, b)),
            &Cancellation::default(),
        )
        .unwrap();
        let configurations = product.configurations_at_degree(200);
        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].parts().len(), 200);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(
            product_with(&constraint, &constraint, |a, b| Some((a, b)), &cancellation).is_err()
        );
    }
    #[test]
    fn product_keeps_counts_and_permits_all_alignments() {
        let make = |groups: Vec<(Vec<u8>, u32)>| {
            Constraint::new(vec![
                CondensedConfiguration::new(
                    groups
                        .into_iter()
                        .map(|(labels, count)| Part::new(LabelSet::new(labels), count).unwrap())
                        .collect(),
                )
                .unwrap(),
            ])
        };
        let left = make(vec![(vec![0], 1), (vec![1], 1)]);
        let right = make(vec![(vec![2], 1), (vec![3], 1)]);
        let result =
            product_with(&left, &right, |a, b| Some((a, b)), &Cancellation::default()).unwrap();
        assert_eq!(concrete_multisets(&result, &[2]).len(), 2);
        let large = make(vec![(vec![0], u32::MAX)]);
        let result = product_with(
            &large,
            &large,
            |a, b| Some((a, b)),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(result.configurations().count(), 1);
        assert_eq!(
            result.configurations().next().unwrap().parts()[0].multiplicity(),
            u32::MAX
        );
        assert!(
            product_with(&left, &right, |_, _| None::<u8>, &Cancellation::default())
                .unwrap()
                .configurations()
                .next()
                .is_none()
        );
    }

    #[test]
    fn condensed_product_agrees_with_independent_projections() {
        use crate::algorithms::configurations::accepts;
        use crate::model::Configuration;
        for a in 1..4 {
            for b in 1..4 {
                let labels = |mask: u8, offset: u8| {
                    LabelSet::new(
                        (0u8..2)
                            .filter(|i| mask & (1 << i) != 0)
                            .map(|i| i + offset)
                            .collect(),
                    )
                };
                let left = Constraint::new(vec![
                    CondensedConfiguration::new(vec![
                        Part::new(labels(a, 0), 2).unwrap(),
                        Part::new(labels(b, 0), 1).unwrap(),
                    ])
                    .unwrap(),
                ]);
                let right = Constraint::new(vec![
                    CondensedConfiguration::new(vec![
                        Part::new(labels(b, 2), 1).unwrap(),
                        Part::new(labels(a, 2), 2).unwrap(),
                    ])
                    .unwrap(),
                ]);
                for mask in 0..16 {
                    let permitted = |a: u8, b: u8| mask & (1 << (a * 2 + b - 2)) != 0;
                    let result = product_with(
                        &left,
                        &right,
                        |a, b| permitted(a, b).then_some((a, b)),
                        &Cancellation::default(),
                    )
                    .unwrap();
                    for mut code in 0..64 {
                        let mut pairs = Vec::new();
                        for _ in 0..3 {
                            let value = code % 4;
                            code /= 4;
                            pairs.push((value / 2, 2 + value % 2));
                        }
                        let expected = pairs.iter().all(|&(a, b)| permitted(a, b))
                            && accepts(
                                &left,
                                &Configuration::from_occurrences(pairs.iter().map(|p| p.0))
                                    .unwrap(),
                            )
                            && accepts(
                                &right,
                                &Configuration::from_occurrences(pairs.iter().map(|p| p.1))
                                    .unwrap(),
                            );
                        assert_eq!(
                            accepts(&result, &Configuration::from_occurrences(pairs).unwrap()),
                            expected
                        );
                    }
                }
            }
        }
    }
}
