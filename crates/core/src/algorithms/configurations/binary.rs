//! LLM: Exact coverage of two-position domains by a union of condensed rows.

use crate::{
    computation::{Cancellation, Cancelled},
    model::{Constraint, LabelSet},
};

/// LLM: Find a forbidden pair in the Cartesian product of two label domains. Used by
/// LLM: passive verification without expanding pairs or assuming a maximized constraint.
/// LLM: Temporary bitsets describe row coverage of the right domain, not SAT variables.
pub(crate) fn forbidden_pair_with<L: Copy + Ord>(
    constraint: &Constraint<L>,
    left: &LabelSet<L>,
    right: &LabelSet<L>,
    cancellation: &Cancellation,
) -> Result<Option<(L, L)>, Cancelled> {
    // LLM: Empty domains produce no pairs. Symmetry lets the shorter domain drive the
    // LLM: outer traversal; reverse any counterexample to preserve endpoint provenance.
    cancellation.check()?;
    if left.is_empty() || right.is_empty() {
        return Ok(None);
    }
    if left.len() > right.len() {
        return Ok(forbidden_pair_with(constraint, right, left, cancellation)?.map(|(b, a)| (a, b)));
    }
    let choices: Vec<_> = right.iter().collect();
    let words = choices.len().div_ceil(64);

    // LLM: A degree-two row is either S^2 or S T. Cache each part's intersection
    // LLM: with the right domain once, allowing coverage unions a machine word at a time.
    let mut rows = Vec::new();
    for row in constraint.configurations_at_degree(2) {
        cancellation.check()?;
        let first = row.parts()[0].labels();
        let second = row.parts().get(1).map_or(first, |part| part.labels());
        let mut first_bits = vec![0u64; words];
        let mut second_bits = vec![0u64; words];
        for (index, &label) in choices.iter().enumerate() {
            if index % 256 == 0 {
                cancellation.check()?;
            }
            if first.contains(label) {
                first_bits[index / 64] |= 1 << (index % 64);
            }
            if second.contains(label) {
                second_bits[index / 64] |= 1 << (index % 64);
            }
        }
        rows.push((first, second, first_bits, second_bits));
    }

    // LLM: For each left label, subtract every matching row's neighbor set from
    // LLM: the right domain. The union across rows is exact, including overlapping parts.
    let mut uncovered = vec![0u64; words];
    for a in left.iter() {
        cancellation.check()?;
        uncovered.fill(u64::MAX);
        if choices.len() % 64 != 0 {
            uncovered[words - 1] = (1 << (choices.len() % 64)) - 1;
        }
        for (first, second, first_bits, second_bits) in &rows {
            cancellation.check()?;
            let in_first = first.contains(a);
            let in_second = second.contains(a);
            for ((remaining, &a_bits), &b_bits) in
                uncovered.iter_mut().zip(first_bits).zip(second_bits)
            {
                if in_first {
                    *remaining &= !b_bits;
                }
                if in_second {
                    *remaining &= !a_bits;
                }
            }
            if uncovered.iter().all(|&word| word == 0) {
                break;
            }
        }
        // LLM: An uncovered bit identifies an actual forbidden output on the other
        // LLM: endpoint. Callers can recover its original emission and blocking literal.
        if let Some((word, &bits)) = uncovered.iter().enumerate().find(|(_, bits)| **bits != 0) {
            return Ok(Some((
                a,
                choices[word * 64 + bits.trailing_zeros() as usize],
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        algorithms::configurations::accepts,
        model::{CondensedConfiguration, Configuration, Part},
    };

    /// LLM: Compare binary coverage with independent concrete membership over all
    /// LLM: endpoint domains. Shared by tests of explicit and condensed relations.
    fn compare_domains(constraint: &Constraint<u8>) {
        // LLM: Include empty, overlapping, identical, and differently sized domains,
        // LLM: exercising endpoint reversal and repeated-label pairs as well.
        for left in 0u8..8 {
            for right in 0u8..8 {
                let domain =
                    |mask: u8| LabelSet::new((0..3).filter(|i| mask & (1 << i) != 0).collect());
                let left = domain(left);
                let right = domain(right);
                let forbidden =
                    forbidden_pair_with(constraint, &left, &right, &Cancellation::default())
                        .unwrap();
                let expected = left.iter().any(|a| {
                    right.iter().any(|b| {
                        !accepts(
                            constraint,
                            &Configuration::from_occurrences([a, b]).unwrap(),
                        )
                    })
                });
                assert_eq!(forbidden.is_some(), expected);
                // LLM: A returned pair must belong to the original endpoint domains
                // LLM: and genuinely violate the whole union of allowed configurations.
                if let Some((a, b)) = forbidden {
                    assert!(left.contains(a) && right.contains(b));
                    assert!(!accepts(
                        constraint,
                        &Configuration::from_occurrences([a, b]).unwrap()
                    ));
                }
            }
        }
    }

    /// LLM: Exhaust every binary multiset relation on three labels against concrete
    /// LLM: matching, protecting union semantics and endpoint orientation.
    #[test]
    fn all_small_binary_relations_agree_with_concrete_membership() {
        // LLM: Six unordered pairs, including three loops, give 64 distinct relations.
        let pairs: Vec<_> = (0u8..3).flat_map(|a| (a..3).map(move |b| (a, b))).collect();
        for mask in 0u8..64 {
            let rows = pairs
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, &(a, b))| {
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![a]), 1).unwrap(),
                        Part::new(LabelSet::new(vec![b]), 1).unwrap(),
                    ])
                    .unwrap()
                })
                .collect();
            compare_domains(&Constraint::new(rows));
        }
    }

    /// LLM: Check overlapping condensed rows, repeated parts, irrelevant degrees,
    /// LLM: and bitsets spanning machine-word boundaries against concrete matching.
    #[test]
    fn condensed_rows_and_multiple_words_preserve_exact_coverage() {
        // LLM: Combine two different rectangles; some endpoint products are covered
        // LLM: by their union although neither row contains the entire product.
        let row = |a, b| {
            CondensedConfiguration::new(vec![
                Part::new(LabelSet::new(a), 1).unwrap(),
                Part::new(LabelSet::new(b), 1).unwrap(),
            ])
            .unwrap()
        };
        compare_domains(&Constraint::new(vec![
            row(vec![0, 1], vec![1, 2]),
            row(vec![0, 2], vec![0, 1]),
        ]));
        compare_domains(&Constraint::new(vec![
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![0, 2]), 2).unwrap()])
                .unwrap(),
        ]));

        // LLM: Leave one uncovered label at each word boundary to check padding and
        // LLM: recovery of the original right-domain label rather than a bit index.
        let left = LabelSet::new(vec![250u8]);
        let right = LabelSet::new((0..130).collect());
        for missing in [0, 63, 64, 127, 129] {
            let allowed = (0..130).filter(|&label| label != missing).collect();
            let constraint = Constraint::new(vec![
                row(vec![250], allowed),
                CondensedConfiguration::new(vec![Part::new(right.clone(), 1).unwrap()]).unwrap(),
            ]);
            assert_eq!(
                forbidden_pair_with(&constraint, &left, &right, &Cancellation::default()).unwrap(),
                Some((250, missing))
            );
        }
        // LLM: Cancellation applies even when empty domains would otherwise finish immediately.
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(
            forbidden_pair_with(
                &Constraint::<u8>::new(vec![]),
                &LabelSet::default(),
                &right,
                &cancelled
            )
            .is_err()
        );
    }
}
