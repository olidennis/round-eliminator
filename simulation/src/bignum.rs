use serde::{de,Deserializer,Deserialize, Serialize, Serializer};
use uint::*;
use num_bigint::BigInt;

pub trait BigNum : Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Ord + PartialOrd
        + std::ops::Shr<usize,Output=Self> 
        + std::ops::Shl<usize,Output=Self> 
        + std::ops::BitOr<Self,Output=Self> 
        + std::ops::BitAnd<Self,Output=Self> 
        + std::ops::ShrAssign<usize>
        + std::ops::ShlAssign<usize>
        + From<u64>
        {
    type OneBitsOutput : DoubleEndedIterator<Item = usize> + 'static;

    fn remove(&self, other: &Self) -> Self;
    fn create_mask(bits : usize) -> Self;
    fn count_ones(&self) -> u32;
    fn is_superset(&self, other: &Self) -> bool;
    fn one_bits(&self) -> Self::OneBitsOutput;
    fn bits(&self) -> usize;
    fn bit(&self, i : usize) -> bool ;
    fn one() -> Self;
    fn zero() -> Self;
    fn is_zero(&self) -> bool;
    fn maxbits() -> usize;
    fn intoo<T:BigNum>(&self) -> T {
        let ones = self.one_bits();
        let mut r = T::zero();
        for x in ones {
            if x >= T::maxbits() {
                panic!("bad conversion");
            }
            r = r | (T::one() << x);
        }
        r
    }
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
            fn remove(&self, other : &Self) -> Self {
                *self & !*other
            }
            fn create_mask(bits: usize) -> Self {
                ($bn::one() << bits) - $bn::one()
            }
            fn count_ones(&self) -> u32 {
                self.0.iter().map(|x| x.count_ones()).sum()
            }
            fn is_superset(&self, other: &Self) -> bool{
                (*self | *other) == *self
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
uint_with_size!(BigNum2,BigNum2BitsIterator,2);
uint_with_size!(BigNum3,BigNum3BitsIterator,3);
uint_with_size!(BigNum4,BigNum4BitsIterator,4);
uint_with_size!(BigNum8,BigNum8BitsIterator,8);
uint_with_size!(BigNum16,BigNum16BitsIterator,16);


#[derive(Clone,Debug,Eq,PartialEq,Hash,Ord,PartialOrd)]
pub struct BigBigNum(BigInt,(num_bigint::Sign,Vec<u32>));

impl BigBigNum{
    pub fn new(x : BigInt) -> Self {
        let data = x.clone().to_u32_digits();
        Self(x,data)
    }
}

impl Serialize for BigBigNum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0.to_str_radix(16))
    }
}

impl<'de> Deserialize<'de> for BigBigNum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let x = BigInt::parse_bytes(s.as_bytes(),16).ok_or_else(||de::Error::custom(format!("error parsing number")));
        x.map(|x|BigBigNum::new(x))
    }
}


impl BigNum for BigBigNum {
    fn remove(&self, other : &Self) -> Self {
        self & !other
    }
    fn create_mask(bits: usize) -> Self {
        (Self::one() << bits) - Self::one()
    }

    fn count_ones(&self) -> u32 {
        (self.1).1.iter().map(|x|x.count_ones()).sum()
        //self.one_bits().count() as u32
    }

    fn is_superset(&self, other: &BigBigNum) -> bool {
        let (sign1,data1) = &self.1;
        let (sign2,data2) = &other.1;
        assert!(*sign1 != num_bigint::Sign::Minus);
        assert!(*sign2 != num_bigint::Sign::Minus);
        let len1 = data1.len();
        let len2 = data2.len();
        if len2 > len1 && data2[len1..len2].iter().any(|&x|x!=0) {
            return false;
        }
        if data1.iter().zip(data2.iter()).any(|(&a,&b)| a|b != a ) {
            return false;
        }
        return true;
    }

    type OneBitsOutput = BigBigNumBitsIterator;


    fn one_bits(&self) -> Self::OneBitsOutput {
        let bits = self.bits();
        BigBigNumBitsIterator{ num : self.clone(), range : 0..bits }
    }

    fn bits(&self) -> usize {
        self.0.bits()
    }

    fn bit(&self, i : usize) -> bool {
        let a = i / 32;
        let b = i % 32;
        let target = if a < ((self.1).1).len() { (self.1).1[a] } else { 0 };
        let bit = (target >> b) & 1;
        let sign = match (self.1).0 {
            num_bigint::Sign::Minus => {1}
            num_bigint::Sign::Plus => {0}
            num_bigint::Sign::NoSign => {0}
        };
        bit != sign
        //((self.0.clone() >> i) & Self::one().0) != Self::zero().0
    }
    
    fn one() -> Self {
        Self::new(1.into())
    }

    fn zero() -> Self {
        Self::new(0.into())
    }

    fn is_zero(&self) -> bool {
        self.0 == Self::zero().0
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
        Self::new(self.0 << x)
    }
}

impl std::ops::Shr<usize> for BigBigNum {
    type Output = Self;

    fn shr(self, x : usize) -> Self {
        Self::new(self.0 >> x)
    }
}


impl std::ops::BitOr<BigBigNum> for BigBigNum {
    type Output = Self;

    fn bitor(self, x : BigBigNum) -> Self {
        Self::new(self.0 | x.0)
    }
}


impl std::ops::Not for BigBigNum {
    type Output = Self;

    fn not(self) -> Self{
        Self::new(!self.0)
    }
}

impl std::ops::Not for &BigBigNum {
    type Output = BigBigNum;

    fn not(self) -> BigBigNum{
        BigBigNum::new(!&self.0)
    }
}

impl std::ops::BitAnd<BigBigNum> for BigBigNum {
    type Output = Self;

    fn bitand(self, x : BigBigNum) -> Self {
        Self::new(self.0 & x.0)
    }
}

impl std::ops::BitAnd<BigBigNum> for &BigBigNum {
    type Output = BigBigNum;

    fn bitand(self, x : BigBigNum) -> BigBigNum {
        BigBigNum::new(&self.0 & x.0)
    }
}

impl std::ops::Sub<BigBigNum> for BigBigNum {
    type Output = Self;

    fn sub(self, x : BigBigNum) -> Self {
        Self::new(self.0 - x.0)
    }
}


impl std::ops::ShrAssign<usize> for BigBigNum {
    fn shr_assign(&mut self, x : usize){
        self.0 >>= x;
        self.1 = self.0.to_u32_digits();
    }
}

impl std::ops::ShlAssign<usize> for BigBigNum {
    fn shl_assign(&mut self, x : usize){
        self.0 <<= x;
        self.1 = self.0.to_u32_digits();
    }
}

impl<T> From<T> for BigBigNum  where T : Into<BigInt>{
    fn from(x: T) -> Self {
        Self::new(x.into())
    }
}

