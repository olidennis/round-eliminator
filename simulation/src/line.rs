use crate::bignum::BigNum;
use itertools::Itertools;
use permutator::Permutation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// This struct represent a single line of the constraints of a problem.
/// It is internally represented by a vector of bits of type BigNum,
/// that can be seen as `delta` groups of `bits` bits
/// For example, if Delta=3 and Bits=2, the bitvector 011111 represents
/// the constraint where there are 3 groups, 01, 11, and 11, that if we call the first bit A and the second bit B,
/// corresponds to a constraint like B AB AB.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Hash, Ord, PartialOrd)]
pub struct Line {
    pub inner: BigNum,
    pub delta: usize,
    pub bits: usize,
}

impl Line {
    /// Creates a new line, with `delta` groups of `bits` bits, where the bits are initialized
    /// using the given `num` value.
    pub fn from(delta: usize, bits: usize, num: BigNum) -> Self {
        Self {
            inner: num,
            delta,
            bits,
        }
    }

    /// Creates a new line, with `delta` groups of `bits` bits,
    /// where the bits are set to what the `groups` iterator gives.
    /// It is assumed that `groups` is an iterator returning `delta` elements.
    pub fn from_groups(delta: usize, bits: usize, groups: impl Iterator<Item = BigNum>) -> Line {
        let mut new = BigNum::zero();
        let mut shift = 0;
        for group in groups {
            new = new | (group << shift);
            shift += bits;
        }
        Self::from(delta, bits, new)
    }

    /// This functions produces an iterator that iterates over all possible Lines that can be created by
    /// combining elements of the given `set` in all possible ways.
    pub fn forall_over<'a>(
        delta: usize,
        bits: usize,
        set: &'a Vec<BigNum>,
    ) -> impl Iterator<Item = Self> + 'a {
        //in case of overflow, just abort
        //iterating over more than 2^64 requires too much time anyway
        set.len().checked_pow(delta as u32).unwrap();

        (0..set.len().pow(delta as u32)).map(move |mut x| {
            let groups = (0..delta).map(|_| {
                let cur = set[x % set.len()];
                x /= set.len();
                cur
            });
            Self::from_groups(delta, bits, groups)
        })
    }

    /// This functions produces an iterator that iterates over all possible Lines that can be created by
    /// allowing only a single element for each group.
    /// `mask` is a bit vector that specifies which elements are allowed.
    pub fn forall_single(delta: usize, bits: usize, mask: BigNum) -> impl Clone + DoubleEndedIterator<Item = Self> {
        //in case of overflow, just abort
        //iterating over more than 2^64 requires too much time anyway
        (bits as usize).checked_pow(delta as u32).unwrap();

        // it could be optimized by iterating over mask.count_ones() to the delta instead of bits to the delta
        (0..(bits as usize).pow(delta as u32)).filter_map(move |mut x| {
            let groups = (0..delta).map(|_| {
                let cur = BigNum::one() << (x % bits as usize);
                x /= bits as usize;
                cur
            });
            let line = Self::from_groups(delta, bits, groups);
            if line.groups().any(|g| (g & mask).is_zero()) {
                return None;
            }
            Some(line)
        })
    }

    /// Creates a new line where the label `from` is replaced by the label `to`.
    pub fn replace(&self, from: usize, to: usize) -> Line {
        self.replace_with_group(from, BigNum::one() << to)
    }

    pub fn replace_with_group(&self, from: usize, to: BigNum) -> Line {
        let one = BigNum::one();
        let from = one << from;

        self.edited(|group| {
            if !(group & from).is_zero() {
                (group & (!from)) | to
            } else {
                group
            }
        })
    }

    /// Creates a new line where the label `from` is replaced by the label `to`,
    /// assuming that each group contains a single label.
    #[allow(dead_code)]
    pub fn replace_fast(&self, from: usize, to: usize) -> Line {
        let one = BigNum::one();
        let from = one << from;
        let to = one << to;

        self.edited(|group| if group == from { to } else { group })
    }

    /// This method assumes that each group contains a single label.
    /// Given a line where `from` may appear multiple times, it returns an iterator of lines
    /// where at each step one additional appearence of `from` is replaced by `to`.
    /// For example, given A A C, where from=A and to=B, it returns B A C, B B C.
    pub fn replace_one_fast(&self, from: usize, to: usize) -> impl Iterator<Item = Line> {
        let one = BigNum::one();
        let from = one << from;
        let to = one << to;
        let mut state = self.clone();
        self.groups()
            .enumerate()
            .filter(move |&(_, x)| x == from)
            .map(move |(i, _)| {
                state = state.with_group(i, to);
                state
            })
    }

    /// Creates a new line where only labels allowed by the given mask are kept.
    /// If a group becomes empty, it returns None.
    pub fn harden(&self, keepmask: BigNum) -> Option<Line> {
        let newline = self.edited(|group| group & keepmask);
        if !newline.contains_empty_group() {
            Some(newline)
        } else {
            None
        }
    }

    /// Creates a new line where each group is replaced by the result of `f`.
    pub fn edited(&self, f: impl FnMut(BigNum) -> BigNum) -> Line {
        let newgroups = self.groups().map(f);
        Line::from_groups(self.delta, self.bits, newgroups)
    }

    /// add the label to each time from is allowed
    pub fn imply(&self, from : usize, to : usize) -> Line {
        let one = BigNum::one();
        let from = one << from;
        let to = one << to;
        self.edited(|group| {
            if !(group & from).is_zero() {
                group | to
            } else {
                group
            }
        })
    }


    /// Returns an iterator over the groups of the line (starting from the least significant bits).
    /// The iterator will contain `delta` elements.
    pub fn groups(&self) -> impl Iterator<Item = BigNum> {
        let mut sets = self.inner;
        let delta = self.delta;
        let bits = self.bits;

        let one = BigNum::one();

        (0..delta).map(move |_| {
            let r = sets & ((one << bits) - one);
            sets >>= bits;
            r
        })
    }

    /// Returns the ith group
    pub fn group(&self, i : usize) -> BigNum {
        let bits = self.bits;
        let one = BigNum::one();
        (self.inner >> (i * bits)) & ((one << bits) - one)
    }

    /// Replaces the i-th group with the new value `group` (starting to count from the least significant bits).
    pub fn with_group(&self, i: usize, group: BigNum) -> Line {
        let one = BigNum::one();
        let bits = self.bits;
        let inner = self.inner;
        let innerzeroed = inner & (!(((one << bits) - one) << (bits as usize * i)));
        let new = innerzeroed | (group << (bits as usize * i));
        Line::from(self.delta, bits, new)
    }

    /// Returns true if there exists a group that has all bits unset (meaning that this line does not allow anything).
    pub fn contains_empty_group(&self) -> bool {
        self.groups().any(|g| g == BigNum::zero())
    }

    /// Returns true if the current line allows at least what is allowed by `other` (with the same order).
    pub fn includes(&self, other: &Self) -> bool {
        (self.inner | other.inner) == self.inner
    }

    /// Returns an iterator over all the lines that can be obtained by permuting the groups of the current line.
    pub fn permutations(&self) -> LinePermutationIter {
        let g: Vec<_> = self.groups().collect();
        let bits = self.bits;
        let delta = self.delta;

        LinePermutationIter {
            groups: g,
            bits,
            delta,
        }
    }

    /// Given a string that represents a line, the string is parsed and split in a vector representation,
    /// Each resulting vector represents a single group of the line.
    /// Each group is represented by a vector of strings.
    pub fn string_to_vec(line: &str) -> Vec<Vec<String>> {
        line.split_whitespace()
            .map(|w| {
                w.chars()
                    .batching(|it| match it.next() {
                        Some('(') => Some(format!(
                            "({})",
                            it.take_while(|&c| c != ')').collect::<String>()
                        )),
                        Some(c) => Some(format!("{}", c)),
                        None => None,
                    })
                    .collect()
            })
            .collect()
    }

    /// Creates a line starting from its vector representation.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn from_vec(v: &Vec<Vec<String>>, mapping: &HashMap<String, usize>) -> Self {
        let delta = v.len();
        let bits = mapping.len();
        let mut line = BigNum::zero();
        for x in v {
            assert!(x.len() != 0);
            let t = x
                .iter()
                .map(|c| BigNum::one() << mapping[c])
                .fold(BigNum::zero(), |r, x| r | x);
            line <<= bits;
            line = line | t;
        }
        Self::from(delta, bits, line)
    }

    /// Creates a vector representation of a line.
    /// Each resulting vector represents a single group of the line.
    /// Each group is represented by a vector of strings.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn to_vec(&self, mapping: &HashMap<usize, String>) -> Vec<Vec<String>> {
        let bits = self.bits;
        self.groups()
            .map(|g| {
                (0..bits)
                    .filter(|&i| g.bit(i))
                    .map(|i| mapping[&(i as usize)].to_owned())
                    .collect()
            })
            .collect()
    }

    /// Rename the labels, that is, each possible group value gets a single bit in the new line.
    /// `mapping` indicates how to map groups to labels.
    /// For example, if 011010 should be mapped to 1000, then mapping should map `011010` to 3 (the position of the unique bit with value 1).
    pub fn renamed(&self, mapping: &HashMap<BigNum, usize>) -> Self {
        let newbits = mapping.len();
        let newgroups = self.groups().map(|g| BigNum::one() << mapping[&g]);
        Self::from_groups(self.delta, newbits, newgroups)
    }

    /// Given a line, for each group, consider all sets (the keys in `mapping`) that contain at least one label of the group.
    /// This function creates a new line where the renamed label (given by `mapping`) of each set is allowed.
    pub fn anymap(&self, mapping: &HashMap<BigNum, usize>) -> Line {
        let newbits = mapping.len();
        let newgroups = self.groups().map(|g| {
            mapping
                .keys()
                .filter(|&&t| !(g & t).is_zero())
                .map(|o| mapping[o])
                .fold(BigNum::zero(), |r, x| r | (BigNum::one() << x))
        });
        Self::from_groups(self.delta, newbits, newgroups)
    }

    /// Performs an or of all groups, returning a mask of used labels.
    pub fn mask(&self) -> BigNum {
        self.groups().fold(BigNum::zero(), |a, b| a | b)
    }

    /// Checks if the current line is a possible action, that is,
    /// for each group there is only one bit that is set.
    pub fn is_action(&self) -> bool {
        // it would be faster to return self.count_ones() == delta,
        // that works if we assume that the line is well formed (no empty group)
        self.groups().all(|group| group.count_ones() == 1)
    }


    pub fn sorted(&self) -> Self {
        let sg = self.groups().sorted();
        let delta = self.delta;
        let bits = self.bits;
        Self::from_groups(delta, bits, sg)
    }

    pub fn add_column(&self, x : usize) -> Self {
        let delta = self.delta+1;
        let bits = self.bits;
        let inner = (self.inner << bits ) | (BigNum::one() << x);
        Self { delta, bits, inner }
    }
}

pub struct LinePermutationIter {
    groups: Vec<BigNum>,
    bits: usize,
    delta: usize,
}

impl LinePermutationIter {
    pub fn iter<'a>(&'a mut self) -> impl Iterator<Item = Line> + 'a {
        let delta = self.delta;
        let bits = self.bits;
        std::iter::once(self.groups.clone())
            .chain(self.groups.permutation())
            .map(move |p| Line::from_groups(delta, bits, p.into_iter()))
    }
}
