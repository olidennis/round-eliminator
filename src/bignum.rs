use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use uint::*;

construct_uint! {
    pub struct BigNum(4);
}

impl Serialize for BigNum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BigNum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

impl BigNum {
    pub fn count_ones(&self) -> u32 {
        self.0.iter().map(|x| x.count_ones()).sum()
    }

    pub fn is_superset(&self, other: BigNum) -> bool {
        (*self | other) == *self
    }

    pub fn one_bits(&self) -> impl Iterator<Item = usize> + '_ {
        let bits = self.bits();
        (0..bits).filter(move |&i| self.bit(i))
    }
}