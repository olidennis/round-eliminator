use serde::{Deserialize, Serialize};
use rug::Integer;

#[derive(Clone,Serialize,Deserialize,Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BigNum (Integer);

impl BigNum {
    pub fn count_ones(&self) -> u32 {
        self.0.count_ones().unwrap() as u32
    }

    pub fn is_superset(&self, other: BigNum) -> bool {
        (self.0 | other.0) == self.0
    }

    pub fn one_bits(&self) -> impl DoubleEndedIterator<Item = usize> + 'static {
        let slf = *self;
        let bits = slf.bits();
        (0..bits).filter(move |&i| slf.bit(i))
    }

    pub fn bits(&self) -> usize {
        self.0.significant_bits() as usize
    }

    pub fn bit(&self, i : usize) -> bool {
        self.0.get_bit(i as u32)
    }
    
    pub fn one() -> Self {
        Self( Integer::from(1) )
    }

    pub fn zero() -> Self {
        Self( Integer::from(0) )
    }

    pub fn is_zero(&self) -> bool {
        self.bits() == 0
    }
}


impl std::ops::Shl<usize> for BigNum {
    type Output = Self;

    fn shl(self, x : usize) -> Self {
        Self(self.0 << x as u32)
    }
}


impl std::ops::Shr<usize> for BigNum {
    type Output = Self;

    fn shr(self, x : usize) -> Self {
        Self(self.0 >> x as u32)
    }
}

impl std::ops::BitOr<BigNum> for BigNum {
    type Output = Self;

    fn bitor(self, x : BigNum) -> Self {
        Self(self.0 | x.0)
    }
}

impl std::ops::Not for BigNum {
    type Output = Self;

    fn not(self) -> Self{
        Self(!self.0)
    }
}

impl std::ops::BitAnd<BigNum> for BigNum {
    type Output = Self;

    fn bitand(self, x : BigNum) -> Self {
        Self(self.0 & x.0)
    }
}

impl std::ops::Sub<BigNum> for BigNum {
    type Output = Self;

    fn sub(self, x : BigNum) -> Self {
        Self(self.0 - x.0)
    }
}

impl std::ops::ShrAssign<usize> for BigNum {
    fn shr_assign(&mut self, x : usize){
        self.0 >>= x as u32;
    }
}

impl std::ops::ShlAssign<usize> for BigNum {
    fn shl_assign(&mut self, x : usize){
        self.0 <<= x as u32;
    }
}

impl<T> From<T> for BigNum  where T : Into<Integer>{
    fn from(x: T) -> Self {
        Self(x.into())
    }
}