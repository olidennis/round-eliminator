#![allow(dead_code)]

use crate::bignum::BigNum;
use crate::line::Line;
use std::collections::HashSet;

pub trait LineSet {
    fn contains(&self, line: Line) -> bool;
    fn insert(&mut self, line: Line);
    fn new(delta: usize, bits: usize) -> Self;
}

const BITS_PER_ELEM: usize = 8 * std::mem::size_of::<usize>();

pub struct BigBitSet {
    v: Vec<usize>,
}

impl BigBitSet {
    pub fn with_capacity(sz: usize) -> Self {
        Self {
            v: vec![0; sz / BITS_PER_ELEM + 1],
        }
    }
}

impl LineSet for BigBitSet {
    fn new(delta: usize, bits: usize) -> Self {
        let sz = 1 << (delta * bits);
        Self {
            v: vec![0; sz / BITS_PER_ELEM + 1],
        }
    }
    fn contains(&self, line: Line) -> bool {
        //BigNum performs an overflow check
        let x = line.inner.as_usize();
        ((self.v[x / BITS_PER_ELEM] >> (x % BITS_PER_ELEM)) & 1) != 0
    }
    fn insert(&mut self, line: Line) {
        //BigNum performs an overflow check
        let x = line.inner.as_usize();
        self.v[x / BITS_PER_ELEM] |= 1 << (x % BITS_PER_ELEM);
    }
}

impl LineSet for HashSet<BigNum> {
    fn new(_: usize, _: usize) -> Self {
        HashSet::new()
    }
    fn contains(&self, line: Line) -> bool {
        HashSet::contains(self, &line.inner)
    }
    fn insert(&mut self, line: Line) {
        HashSet::insert(self, line.inner);
    }
}
