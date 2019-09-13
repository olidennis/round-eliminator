use crate::bignum::BigNum;
use crate::line::Line;
use std::collections::HashSet;

/// A trait for a set that can contain lines efficiently.
pub trait LineSet {
    fn contains(&self, line: Line) -> bool;
    fn insert(&mut self, line: Line);
    fn new(delta: usize, bits: usize) -> Self;
}

const BITS_PER_ELEM: usize = 8 * std::mem::size_of::<usize>();

/// A set that can contain lines using a bitvector.
/// It is fast to get/set.
/// It is slow to initialize and it uses a lot of memory.
pub struct BigBitSet {
    v: Vec<usize>,
}

impl LineSet for BigBitSet {
    fn new(delta: usize, bits: usize) -> Self {
        let sz = 1 << (delta * bits);
        Self {
            //TODO: why +1? I don't remember.
            v: vec![0; sz / BITS_PER_ELEM + 1],
        }
    }
    fn contains(&self, line: Line) -> bool {
        //this will panic if a line does not fit an usize
        let x = line.inner.as_usize();
        ((self.v[x / BITS_PER_ELEM] >> (x % BITS_PER_ELEM)) & 1) != 0
    }
    fn insert(&mut self, line: Line) {
        //his will panic if a line does not fit an usize
        let x = line.inner.as_usize();
        self.v[x / BITS_PER_ELEM] |= 1 << (x % BITS_PER_ELEM);
    }
}

/// HashSet can be used to contain lines.
/// It is a bit slower to get/set compared to a BigBitSet.
/// It is fast to initialize and does not use much memory.
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
