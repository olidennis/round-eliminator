use serde::{de,Deserializer,Deserialize, Serialize, Serializer};
use uint::*;

pub trait BigNum : Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Ord + PartialOrd
        + std::ops::Shr<usize,Output=Self> 
        + std::ops::Shl<usize,Output=Self> 
        + std::ops::BitOr<Self,Output=Self> 
        + std::ops::BitAnd<Self,Output=Self> 
        + std::ops::Sub<Self,Output=Self> 
        + std::ops::Not<Output=Self>
        + std::ops::ShrAssign<usize>
        + std::ops::ShlAssign<usize>
        + From<u64>
        {
    type OneBitsOutput : DoubleEndedIterator<Item = usize> + 'static;

    fn count_ones(&self) -> u32;
    fn is_superset(&self, other: Self) -> bool;
    fn one_bits(&self) -> Self::OneBitsOutput;
    fn bits(&self) -> usize;
    fn bit(&self, i : usize) -> bool ;
    fn one() -> Self;
    fn zero() -> Self;
    fn is_zero(&self) -> bool;
    fn maxbits() -> usize;
}



macro_rules! uint_with_size {
    ($bn:ident,$bni:ident,$sz:tt) => {
        construct_uint! {
            pub struct $bn($sz);
        }

        impl Serialize for $bn {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_str(self)
            }
        }
        
        impl<'de> Deserialize<'de> for $bn {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                let x = Self::from_dec_str(&s).map_err(|e| de::Error::custom(format!("{:?}", e)));
                x
            }
        }

        impl BigNum for $bn {
            fn count_ones(&self) -> u32 {
                self.0.iter().map(|x| x.count_ones()).sum()
            }
            fn is_superset(&self, other: Self) -> bool{
                (*self | other) == *self
            }
            type OneBitsOutput = $bni;
            fn one_bits(&self) -> Self::OneBitsOutput {
                let bits = self.bits();
                $bni{ num : self.clone(), range : 0..bits }
            }
            fn bits(&self) -> usize{
                self.bits()
            }
            fn bit(&self, i : usize) -> bool {
                self.bit(i)
            }
            fn one() -> Self {
                $bn::one()
            }
            fn zero() -> Self{
                $bn::zero()
            }
            fn is_zero(&self) -> bool {
                self.is_zero()
            }
            fn maxbits() -> usize {
                $sz * 64
            }
        }

        pub struct $bni {
            num : $bn,
            range : std::ops::Range<usize>
        }
        
        impl Iterator for $bni{
            type Item = usize;
            fn next(&mut self) -> Option<Self::Item> {
                let range = &mut self.range;
                let num = &self.num;
                range.find(|&i|num.bit(i))
            }
        }

        impl DoubleEndedIterator for $bni{
            fn next_back(&mut self) -> Option<Self::Item> {
                let range = &mut self.range;
                let num = &self.num;
                range.rfind(|&i|num.bit(i))
            }
        }
        

    }
}

uint_with_size!(BigNum1,BigNum1BitsIterator,1);


#[derive(Clone,Debug,Eq,PartialEq,Hash,Ord,PartialOrd)]
pub struct BigBigNum(ramp::Int);


impl Serialize for BigBigNum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0.to_str_radix(16,true))
    }
}

impl<'de> Deserialize<'de> for BigBigNum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let x = ramp::Int::from_str_radix(&s,16).map_err(|e| de::Error::custom(format!("{:?}", e)));
        x.map(|x|BigBigNum(x))
    }
}

impl BigNum for BigBigNum {
    fn count_ones(&self) -> u32 {
        self.0.count_ones() as u32
    }

    fn is_superset(&self, other: BigBigNum) -> bool {
        (self.0.clone() | other.0) == self.0
    }

    type OneBitsOutput = BigBigNumBitsIterator;


    fn one_bits(&self) -> Self::OneBitsOutput {
        let bits = self.bits();
        BigBigNumBitsIterator{ num : self.clone(), range : 0..bits }
    }

    fn bits(&self) -> usize {
        self.0.bit_length() as usize
    }

    fn bit(&self, i : usize) -> bool {
        self.0.bit(i as u32)
    }
    
    fn one() -> Self {
        Self( ramp::Int::from(1) )
    }

    fn zero() -> Self {
        Self( ramp::Int::from(0) )
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    fn maxbits() -> usize {
        std::usize::MAX
    }
}

pub struct BigBigNumBitsIterator {
    num : BigBigNum,
    range : std::ops::Range<usize>
}

impl Iterator for BigBigNumBitsIterator{
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        let range = &mut self.range;
        let num = &self.num;
        range.find(|&i|num.bit(i))
    }
}

impl DoubleEndedIterator for BigBigNumBitsIterator{
    fn next_back(&mut self) -> Option<Self::Item> {
        let range = &mut self.range;
        let num = &self.num;
        range.rfind(|&i|num.bit(i))
    }
}

impl std::ops::Shl<usize> for BigBigNum {
    type Output = Self;

    fn shl(self, x : usize) -> Self {
        Self(self.0 << x)
    }
}

impl std::ops::Shr<usize> for BigBigNum {
    type Output = Self;

    fn shr(self, x : usize) -> Self {
        Self(self.0 >> x)
    }
}


impl std::ops::BitOr<BigBigNum> for BigBigNum {
    type Output = Self;

    fn bitor(self, x : BigBigNum) -> Self {
        Self(self.0 | x.0)
    }
}


impl std::ops::Not for BigBigNum {
    type Output = Self;

    fn not(self) -> Self{
        BigBigNum(-self.0 -1)
    }
}

impl std::ops::BitAnd<BigBigNum> for BigBigNum {
    type Output = Self;

    fn bitand(self, x : BigBigNum) -> Self {
        Self(self.0 & x.0)
    }
}

impl std::ops::Sub<BigBigNum> for BigBigNum {
    type Output = Self;

    fn sub(self, x : BigBigNum) -> Self {
        BigBigNum(self.0 - x.0)
    }
}


impl std::ops::ShrAssign<usize> for BigBigNum {
    fn shr_assign(&mut self, x : usize){
        self.0 >>= x;
    }
}

impl std::ops::ShlAssign<usize> for BigBigNum {
    fn shl_assign(&mut self, x : usize){
        self.0 <<= x;
    }
}

impl<T> From<T> for BigBigNum  where T : Into<ramp::Int>{
    fn from(x: T) -> Self {
        Self(x.into())
    }
}

