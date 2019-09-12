#![allow(dead_code)]

use crate::bignum::BigNum;
use itertools::Itertools;
use permutator::Permutation;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Line {
    pub inner: BigNum,
    pub delta: u8,
    pub bits: u8,
}

impl Line {
    /// Creates a new zero initialized line
    pub fn new(delta: u8, bits: u8) -> Self {
        Self::from(delta, bits, BigNum::zero())
    }

    /// Creates a new line
    pub fn from(delta: u8, bits: u8, num: BigNum) -> Self {
        Self {
            inner: num,
            delta,
            bits,
        }
    }

    /// Create a line from an iterator of groups
    pub fn from_groups(delta: u8, bits: u8, groups: impl Iterator<Item = BigNum>) -> Line {
        let mut new = BigNum::zero();
        let mut shift = 0;
        for group in groups {
            new = new | (group << shift);
            shift += bits;
        }
        Self::from(delta, bits, new)
    }

    /// produce an iterator over the delta-ary the cartesian power of `set`
    pub fn forall_over<'a>(
        delta: u8,
        bits: u8,
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

    /// produce an iterator over all possible lines such that each group has a single element
    /// where mask specify the allowed elements
    pub fn forall_single(delta: u8, bits: u8, mask: BigNum) -> impl Iterator<Item = Self> {
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

    /// Relax label number `from` to label number `to`
    pub fn relax(&self, from: usize, to: usize) -> Line {
        let one = BigNum::one();
        let from = one << from;
        let to = one << to;

        self.edited(|group| {
            if !(group & from).is_zero() {
                (group & (!from)) | to
            } else {
                group
            }
        })
    }

    /// Replace label number `from` to label number `to`
    /// assumes that each group contains a single label
    pub fn replace(&self, from: usize, to: usize) -> Line {
        let one = BigNum::one();
        let from = one << from;
        let to = one << to;

        self.edited(|group| if group == from { to } else { group })
    }

    /// keeps only the labels satisfying the given bitmask
    /// if a group becomes empty, it returns None
    pub fn harden(&self, keepmask: BigNum) -> Option<Line> {
        let newline = self.edited(|group| group & keepmask);
        if !newline.contains_empty_group() {
            Some(newline)
        } else {
            None
        }
    }

    /// Create a new line where each group is replaced with the result of `f`
    pub fn edited(&self, f: impl FnMut(BigNum) -> BigNum) -> Line {
        let newgroups = self.groups().map(f);
        Line::from_groups(self.delta, self.bits, newgroups)
    }

    /// Returns an iterator over the delta groups
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

    /// Replace i-th group with the new value `group`
    pub fn with_group(&self, i: usize, group: BigNum) -> Line {
        let one = BigNum::one();
        let bits = self.bits;
        let inner = self.inner;
        let innerzeroed = inner & (!(((one << bits) - one) << (bits as usize * i)));
        let new = innerzeroed | (group << (bits as usize * i));
        Line::from(self.delta, bits, new)
    }

    /// returns true if there exists a group that has all bits unset
    pub fn contains_empty_group(&self) -> bool {
        self.groups().any(|g| g == BigNum::zero())
    }

    /// returns true if the current line allows at least what is allowed by `other`
    pub fn includes(&self, other: &Self) -> bool {
        (self.inner | other.inner) == self.inner
    }

    /// returns all the possible lines obtained by permuting the groups of the current line
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

    pub fn parse(line : &str) -> Vec<Vec<String>> {
        line.split_whitespace().map(|w|{
            w.chars().batching(|it|{
                match it.next() {
                    Some('<') => {
                        Some(format!("<{}>",it.take_while(|&c|c!='>').collect::<String>()))
                    },
                    Some(c) => Some(format!("{}",c)),
                    None => None
                }
            }).collect()
        }).collect()
    }

    /// creates a line starting from its character representation
    pub fn from_parsed(v : &Vec<Vec<String>>, mapping: &HashMap<String, usize>) -> Self {

        let delta = v.len() as u8;
        let bits = mapping.len() as u8;
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

    /// creates a character representation of a line
    pub fn to_text(&self, mapping: &HashMap<usize, String>) -> String {
        let bits = self.bits as usize;
        self.groups()
            .map(|g| {
                (0..bits)
                    .filter(|&i| g.bit(i))
                    .map(|i| &mapping[&(i as usize)])
                    .join("")
            })
            .join(" ")
    }

    /// rename the labels, that is, each possible group value gets a single bit in the new line
    pub fn renamed(&self, mapping: &HashMap<BigNum, usize>) -> Self {
        let newbits = mapping.len() as u8;
        let newgroups = self.groups().map(|g| BigNum::one() << mapping[&g]);
        Self::from_groups(self.delta, newbits, newgroups)
    }

    /// start from a line
    /// for each group, consider all sets that contain at least one label of the group
    /// create a new line where the renamed label of each set is allowed
    pub fn anymap(&self, mapping: &HashMap<BigNum, usize>) -> Line {
        let newbits = mapping.len() as u8;
        let newgroups = self.groups().map(|g| {
            mapping
                .keys()
                .filter(|&&t| !(g & t).is_zero())
                .map(|o| mapping[o])
                .fold(BigNum::zero(), |r, x| r | (BigNum::one() << x))
        });
        Self::from_groups(self.delta, newbits, newgroups)
    }

    /// perform an or of all groups, returning a mask of used labels
    pub fn mask(&self) -> BigNum {
        self.groups().fold(BigNum::zero(), |a, b| a | b)
    }

    /// checks if the current line is a possible action, that is,
    /// for each group there is only one bit that is set
    /// it would be faster to return self.count_ones() == delta,
    /// that works if we assume that the line is well formed (no empty group)
    pub fn is_action(&self) -> bool {
        self.groups().all(|group| group.count_ones() == 1)
    }
}

pub struct LinePermutationIter {
    groups: Vec<BigNum>,
    bits: u8,
    delta: u8,
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
