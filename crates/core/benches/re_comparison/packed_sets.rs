//! Experimental packed-set adapter; the model and generic closure are unchanged.

use super::*;
use std::collections::BTreeSet;

struct PackedOperations;
impl Operations<u64> for PackedOperations {
    fn union(&self, a: &u64, b: &u64) -> u64 {
        a | b
    }
    fn intersection(&self, a: &u64, b: &u64) -> u64 {
        a & b
    }
    fn is_superset(&self, a: &u64, b: &u64) -> bool {
        b & !a == 0
    }
    fn is_admissible(&self, value: &u64) -> bool {
        *value != 0
    }
}

pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() {
        computation.cancellation.check()?;
        alphabet.insert(label);
        if alphabet.len() > 64 {
            return Ok(None);
        }
    }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        computation.cancellation.check()?;
        let mut parts = Vec::new();
        for part in row.parts() {
            let mut bits = 0u64;
            for label in part.labels().iter() {
                bits |= 1u64 << alphabet.binary_search(&label).unwrap();
            }
            parts.push((bits, part.multiplicity()));
        }
        initial.push(Configuration::new(parts)?);
    }
    let maximum = maximize_custom_with(&initial, &PackedOperations, computation, None)?;
    let mut decoded = Vec::new();
    for row in maximum {
        computation.cancellation.check()?;
        let parts = row
            .multiplicities()
            .iter()
            .map(|&(bits, count)| {
                let labels = alphabet
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &label)| (bits & (1u64 << i) != 0).then_some(label))
                    .collect();
                (LabelSet::new(labels), count)
            })
            .collect();
        decoded.push(Configuration::new(parts)?);
    }
    decoded.sort_by(|a, b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(Some(decoded))
}
