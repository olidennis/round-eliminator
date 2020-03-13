use serde::{de,Deserializer,Deserialize, Serialize, Serializer};
use ramp::Int as Integer;

#[derive(Clone,Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BigNum (Integer);


impl Serialize for BigNum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0.to_str_radix(16,true))
    }
}

impl<'de> Deserialize<'de> for BigNum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let x = Integer::from_str_radix(&s,16).map_err(|e| de::Error::custom(format!("{:?}", e)));
        x.map(|x|BigNum(x))
    }
}

impl BigNum {
    pub fn count_ones(&self) -> u32 {
        self.0.count_ones() as u32
    }

    pub fn is_superset(&self, other: &BigNum) -> bool {
        &(&self.0 | &other.0) == &self.0
    }

    pub fn one_bits(&self) -> impl DoubleEndedIterator<Item = usize> + '_{
        let bits = self.bits();
        (0..bits).filter(move |&i| self.bit(i))
    }

    pub fn bits(&self) -> usize {
        self.0.bit_length() as usize
    }

    pub fn bit(&self, i : usize) -> bool {
        self.0.bit(i as u32)
    }
    
    pub fn one() -> Self {
        Self( Integer::from(1) )
    }

    pub fn zero() -> Self {
        Self( Integer::from(0) )
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}


impl std::ops::Shl<usize> for BigNum {
    type Output = Self;

    fn shl(self, x : usize) -> Self {
        Self(self.0 << x)
    }
}

impl std::ops::Shl<usize> for &BigNum {
    type Output = BigNum;

    fn shl(self, x : usize) -> BigNum {
        BigNum(&self.0 << x)
    }
}

impl std::ops::Shr<usize> for BigNum {
    type Output = Self;

    fn shr(self, x : usize) -> Self {
        Self(self.0 >> x)
    }
}

impl std::ops::Shr<usize> for &BigNum {
    type Output = BigNum;

    fn shr(self, x : usize) -> BigNum {
        BigNum(&self.0 >> x)
    }
}

impl std::ops::BitOr<BigNum> for BigNum {
    type Output = Self;

    fn bitor(self, x : BigNum) -> Self {
        Self(self.0 | x.0)
    }
}

impl std::ops::BitOr<&BigNum> for BigNum {
    type Output = Self;

    fn bitor(self, x : &BigNum) -> Self {
        Self(&self.0 | &x.0)
    }
}

impl std::ops::BitOr<&BigNum> for &BigNum {
    type Output = BigNum;

    fn bitor(self, x : &BigNum) -> BigNum {
        BigNum(&self.0 | &x.0)
    }
}

impl std::ops::Not for BigNum {
    type Output = Self;

    fn not(self) -> Self{
        BigNum(-self.0 -1)
    }
}

impl std::ops::Not for &BigNum {
    type Output = BigNum;

    fn not(self) -> BigNum{
        BigNum(-&self.0 -1)
    }
}

impl std::ops::BitAnd<BigNum> for BigNum {
    type Output = Self;

    fn bitand(self, x : BigNum) -> Self {
        Self(self.0 & x.0)
    }
}

impl std::ops::BitAnd<BigNum> for &BigNum {
    type Output = BigNum;

    fn bitand(self, x : BigNum) -> BigNum {
        BigNum(&self.0 & &x.0)
    }
}

impl std::ops::BitAnd<&BigNum> for &BigNum {
    type Output = BigNum;

    fn bitand(self, x : &BigNum) -> BigNum {
        BigNum(&self.0 & &x.0)
    }
}

impl std::ops::Sub<BigNum> for BigNum {
    type Output = Self;

    fn sub(self, x : BigNum) -> Self {
        BigNum(self.0 - x.0)
    }
}

impl std::ops::Sub<&BigNum> for BigNum {
    type Output = Self;

    fn sub(self, x : &BigNum) -> Self {
        BigNum(&self.0 - &x.0)
    }
}

impl std::ops::ShrAssign<usize> for BigNum {
    fn shr_assign(&mut self, x : usize){
        self.0 >>= x;
    }
}

impl std::ops::ShlAssign<usize> for BigNum {
    fn shl_assign(&mut self, x : usize){
        self.0 <<= x;
    }
}

impl<T> From<T> for BigNum  where T : Into<Integer>{
    fn from(x: T) -> Self {
        Self(x.into())
    }
}