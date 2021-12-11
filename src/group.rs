use std::{ops::Deref, collections::HashSet};

use itertools::Itertools;


#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Group(pub Vec<usize>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum GroupType {
    One,
    Many(usize),
    Star,
}

impl Deref for Group {
    type Target = Vec<usize>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Group {
    pub fn as_set(&self) -> HashSet<usize> {
        self.0.iter().cloned().collect()
    } 

    pub fn from_set(h : &HashSet<usize>) -> Self {
        Group(h.iter().cloned().sorted().collect())
    }
}

impl GroupType {
    pub fn to_string(&self) -> String {
        use GroupType::*;
        match self {
            One => String::new(),
            Many(n) => format!("^{}", n),
            Star => String::from('*'),
        }
    }

    pub fn value(&self) -> usize {
        use GroupType::*;
        match self {
            One => 1,
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
        assert_eq!(GroupType::One.value(), 1);
        assert_eq!(GroupType::Many(100).value(), 100);
    }
}
