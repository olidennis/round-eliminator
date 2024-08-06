use std::{
    cmp::Ordering, collections::HashSet, ops::{Deref, DerefMut}
};

use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;

pub type Label = u32;
pub type Exponent = u8;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Group {
    v : Vec<Label>,
    //as_bitvec : Option<u64>
} 

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum GroupType {
    Many(Exponent),
    Star,
}

impl GroupType {
    pub const ONE: GroupType = GroupType::Many(1);
}




impl Group {


    pub fn from(v : Vec<Label>) -> Self {
        /*let as_bitvec = if v.iter().all(|&x|x < 64) {
            Some(v.iter().fold(0,|v,x| v | (1 << x)))
        } else {
            None
        };*/
        Self{v/* , as_bitvec*/}
    }

    pub fn cmp(&self, other : &Self) -> Ordering {
        self.v.cmp(&other.v)
    }

    pub fn len(&self) -> usize {
        self.v.len()
    }

    pub fn iter(&self) -> impl Iterator<Item=&Label> {
        self.v.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }

    pub fn contains(&self, l : &Label) -> bool {
        self.v.contains(l)
    }

    pub fn first(&self) -> Label {
        self.v[0]
    }

    pub fn as_vec(&self) -> Vec<Label> {
        self.v.clone()
    }

    pub fn ensure_sorted(&mut self) {
        if !self.v.is_sorted() {
            self.v.sort_unstable();
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.v.shrink_to_fit();
    }

    pub fn as_set(&self) -> HashSet<Label> {
        self.iter().cloned().collect()
    }

    pub fn from_set(h: &HashSet<Label>) -> Self {
        Group::from(h.iter().cloned().sorted().collect())
    }

    #[inline(never)]
    pub fn is_superset(&self, other: &Group) -> bool {
        //match (self.as_bitvec, other.as_bitvec) {
        //    (Some(v1),Some(v2)) => {  return (v1 | v2) == v1; }
        //    _ => { }
        //}
        //assert!(self.is_sorted());
        //assert!(other.is_sorted());
        let mut it1 = self.iter();

        for &elem in other.iter() {
            if !it1.any(|&x| x == elem) {
                return false;
            }
        }
        true
    }

    pub fn difference(&self, other: &Group) -> Self {
        let mut i = 0;
        let mut j = 0;
        let mut v = Vec::with_capacity(std::cmp::min(self.len(), other.len()));
        while i < self.len() && j < other.len() {
            match self.v[i].cmp(&other.v[j]) {
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => {
                    v.push(self.v[i]);
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    j += 1;
                }
            }
        }
        v.extend(self.v[i..].iter().cloned());
        Group::from(v)
    }

    pub fn intersection(&self, other: &Group) -> Self {
        //assert!(self.is_sorted());
        //assert!(other.is_sorted());
        /*
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
        }*/
        /*Group(
            self.as_set()
                .intersection(&other.as_set())
                .cloned()
                .sorted()
                .collect(),
        )*/
        let mut i = 0;
        let mut j = 0;
        let mut v = Vec::with_capacity(std::cmp::min(self.len(), other.len()));
        while i < self.len() && j < other.len() {
            match self.v[i].cmp(&other.v[j]) {
                std::cmp::Ordering::Equal => {
                    v.push(self.v[i]);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => {
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    j += 1;
                }
            }
        }
        Group::from(v)
    }

    pub fn union(&self, other: &Group) -> Self {
        Group::from(
            self.as_set()
                .union(&other.as_set())
                .cloned()
                .sorted()
                .collect(),
        )
    }
}

impl std::fmt::Display for GroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use GroupType::*;
        match self {
            &GroupType::ONE => Ok(()),
            Many(n) => write!(f, "^{}", n),
            Star => write!(f, "*"),
        }
    }
}

impl GroupType {
    pub fn value(&self) -> usize {
        use GroupType::*;
        match self {
            //One => 1,
            Many(n) => *n as usize,
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
