//! Benchmark-only wider fixed/dynamic word sets at the algorithm boundary.

use super::engine::{Operations, maximize_with_minimum};
use crate::{computation::Computation, model::{Configuration, Constraint, LabelSet}};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct FourWords([u64; 4]);
impl Ord for FourWords {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.iter().rev().cmp(other.0.iter().rev()) }
}
impl PartialOrd for FourWords {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
struct FourOperations;
impl Operations<FourWords> for FourOperations {
    fn union(&self, a: &FourWords, b: &FourWords) -> FourWords {
        FourWords(std::array::from_fn(|i| a.0[i] | b.0[i]))
    }
    fn intersection(&self, a: &FourWords, b: &FourWords) -> FourWords {
        FourWords(std::array::from_fn(|i| a.0[i] & b.0[i]))
    }
    fn is_superset(&self, a: &FourWords, b: &FourWords) -> bool {
        a.0.iter().zip(b.0).all(|(&a, b)| b & !a == 0)
    }
    fn is_admissible(&self, a: &FourWords) -> bool { a.0.iter().any(|&w| w != 0) }
}

#[derive(Clone, Eq, PartialEq)]
struct Words(Arc<[u64]>);
impl Ord for Words {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.iter().rev().cmp(other.0.iter().rev()) }
}
impl PartialOrd for Words {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
struct WordOperations;
impl Operations<Words> for WordOperations {
    fn union(&self, a: &Words, b: &Words) -> Words {
        Words(a.0.iter().zip(b.0.iter()).map(|(&a,&b)| a | b).collect())
    }
    fn intersection(&self, a: &Words, b: &Words) -> Words {
        Words(a.0.iter().zip(b.0.iter()).map(|(&a,&b)| a & b).collect())
    }
    fn is_superset(&self, a: &Words, b: &Words) -> bool {
        a.0.iter().zip(b.0.iter()).all(|(&a,&b)| b & !a == 0)
    }
    fn is_admissible(&self, a: &Words) -> bool { a.0.iter().any(|&w| w != 0) }
}

pub(super) fn maximum_with<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>, computation: &mut Computation<'_>,
) -> anyhow::Result<Option<Vec<Configuration<LabelSet<L>>>>> {
    let mut alphabet = BTreeSet::new();
    for label in constraint.labels() { computation.cancellation.check()?; alphabet.insert(label); }
    let alphabet: Vec<_> = alphabet.into_iter().collect();
    if crate::four_flags::enabled("fixed") {
        if alphabet.len() > 256 { return Ok(None); }
        maximum(constraint, &alphabet, &FourOperations,
            |labels| { let mut words = [0;4]; for &i in labels { words[i/64] |= 1 << (i%64); } FourWords(words) },
            |words, i| words.0[i/64] & (1 << (i%64)) != 0, computation).map(Some)
    } else {
        maximum(constraint, &alphabet, &WordOperations,
            |labels| { let mut words = vec![0; alphabet.len().div_ceil(64)]; for &i in labels { words[i/64] |= 1 << (i%64); } Words(words.into()) },
            |words, i| words.0[i/64] & (1 << (i%64)) != 0, computation).map(Some)
    }
}

fn maximum<L: Copy + Ord, M: Clone + Ord + Send + Sync>(
    constraint: &Constraint<L>, alphabet: &[L], operations: &impl Operations<M>,
    encode: impl Fn(&[usize]) -> M, contains: impl Fn(&M, usize) -> bool,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelSet<L>>>> {
    let mut initial = Vec::new();
    for row in constraint.configurations() {
        computation.cancellation.check()?;
        let parts = row.parts().iter().map(|part| {
            let indices: Vec<_> = part.labels().iter().map(|label| alphabet.binary_search(&label).unwrap()).collect();
            (encode(&indices), part.multiplicity())
        }).collect();
        initial.push(Configuration::new(parts)?);
    }
    let maximum = maximize_with_minimum(&initial, operations, computation, None, 256)?;
    let mut decoded = Vec::new();
    for row in maximum {
        computation.cancellation.check()?;
        decoded.push(Configuration::new(row.multiplicities().iter().map(|(mask,count)| {
            (LabelSet::new(alphabet.iter().enumerate().filter_map(|(i,&label)| contains(mask,i).then_some(label)).collect()), *count)
        }).collect())?);
    }
    decoded.sort_by(|a,b| a.degree().cmp(&b.degree()).then_with(|| a.cmp(b)));
    Ok(decoded)
}
