use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};

use itertools::Itertools;
use serde::Serialize;
use serde::Deserialize;

pub type Label = u8;


#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Group(pub Vec<Label>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum GroupType {
    Many(usize),
    Star,
}

impl GroupType {
    pub const ONE: GroupType = GroupType::Many(1);
}

impl Deref for Group {
    type Target = Vec<Label>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Group {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Group {
    pub fn as_set(&self) -> HashSet<Label> {
        self.iter().cloned().collect()
    }

    pub fn from_set(h: &HashSet<Label>) -> Self {
        Group(h.iter().cloned().sorted().collect())
    }

    pub fn intersection(&self, other: &Group) -> Self {
        assert!(self.is_sorted());
        assert!(other.is_sorted());
        let mut it1 = self.iter();
        let mut it2 = other.iter();
        let mut v = Vec::with_capacity(std::cmp::min(self.len(),other.len()));


        let mut last1 = if let Some(&x) = it1.next() { x } else { return Self(v); };
        let mut last2 = if let Some(&x) = it2.next() { x } else { return Self(v); };

        loop {
            while last1 < last2 {
                last1 = if let Some(&x) = it1.next() { x } else { return Self(v); };
            }
            while last2 < last1 {
                last2 = if let Some(&x) = it2.next() { x } else { return Self(v); };
            }
            if last1 == last2 {
                v.push(last1);
                last2 = if let Some(&x) = it2.next() { x } else { return Self(v); };
            }
        }
        /*Group(
            self.as_set()
                .intersection(&other.as_set())
                .cloned()
                .sorted()
                .collect(),
        )*/
    }

    pub fn union(&self, other: &Group) -> Self {
        Group(
            self.as_set()
                .union(&other.as_set())
                .cloned()
                .sorted()
                .collect(),
        )
    }
}

impl GroupType {
    pub fn to_string(&self) -> String {
        use GroupType::*;
        match self {
            &GroupType::ONE => String::new(),
            Many(n) => format!("^{}", n),
            Star => String::from('*'),
        }
    }

    pub fn value(&self) -> usize {
        use GroupType::*;
        match self {
            //One => 1,
            Many(n) => *n,
            Star => {
                panic!("Should not call value() on Star")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupType;

    #[test]
    #[should_panic]
    fn value_of_star() {
        GroupType::Star.value();
    }

    #[test]
    fn valid_values() {
        assert_eq!(GroupType::ONE.value(), 1);
        assert_eq!(GroupType::Many(100).value(), 100);
    }
}
