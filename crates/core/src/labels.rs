//! Label identifiers and sets used by the problem model.

use serde::Serialize;
use std::cmp::Ordering;
use ts_rs::TS;

/// A label is just a number.
/// Its text representation is stored somewhere else.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, TS)]
pub struct LabelId(pub u32);

/// A set of labels (or of L), stored in increasing order without duplicates.
///
/// Set operations preserve this invariant. The storage is private so algorithms
/// can use set semantics without depending on its representation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct LabelSet<L = LabelId>(Vec<L>);

impl<L> Default for LabelSet<L> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<L: Copy + Ord> LabelSet<L> {
    pub fn new(mut labels: Vec<L>) -> Self {
        labels.sort_unstable();
        labels.dedup();
        Self(labels)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = L> + '_ {
        self.0.iter().copied()
    }

    pub fn contains(&self, label: L) -> bool {
        self.0.binary_search(&label).is_ok()
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        let mut right = 0;
        for label in &self.0 {
            while other
                .0
                .get(right)
                .is_some_and(|candidate| candidate < label)
            {
                right += 1;
            }
            if other.0.get(right) != Some(label) {
                return false;
            }
        }
        true
    }

    pub fn union(&self, other: &Self) -> Self {
        let mut labels = Vec::with_capacity(self.len() + other.len());
        let (mut left, mut right) = (0, 0);
        while left < self.len() && right < other.len() {
            match self.0[left].cmp(&other.0[right]) {
                Ordering::Less => {
                    labels.push(self.0[left]);
                    left += 1;
                }
                Ordering::Greater => {
                    labels.push(other.0[right]);
                    right += 1;
                }
                Ordering::Equal => {
                    labels.push(self.0[left]);
                    left += 1;
                    right += 1;
                }
            }
        }
        labels.extend_from_slice(&self.0[left..]);
        labels.extend_from_slice(&other.0[right..]);
        Self(labels)
    }

    pub fn intersection(&self, other: &Self) -> Self {
        let mut labels = Vec::with_capacity(self.len().min(other.len()));
        let (mut left, mut right) = (0, 0);
        while left < self.len() && right < other.len() {
            match self.0[left].cmp(&other.0[right]) {
                Ordering::Less => left += 1,
                Ordering::Greater => right += 1,
                Ordering::Equal => {
                    labels.push(self.0[left]);
                    left += 1;
                    right += 1;
                }
            }
        }
        Self(labels)
    }

    pub fn difference(&self, other: &Self) -> Self {
        let mut labels = Vec::with_capacity(self.len());
        let (mut left, mut right) = (0, 0);
        while left < self.len() && right < other.len() {
            match self.0[left].cmp(&other.0[right]) {
                Ordering::Less => {
                    labels.push(self.0[left]);
                    left += 1;
                }
                Ordering::Greater => right += 1,
                Ordering::Equal => {
                    left += 1;
                    right += 1;
                }
            }
        }
        labels.extend_from_slice(&self.0[left..]);
        Self(labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn set(mask: u8) -> LabelSet {
        LabelSet::new(
            (0..4)
                .filter(|id| mask & (1 << id) != 0)
                .map(LabelId)
                .collect(),
        )
    }

    fn as_btree(set: &LabelSet) -> BTreeSet<LabelId> {
        set.iter().collect()
    }

    #[test]
    fn label_set_operations_match_set_operations() {
        for left in 0..16 {
            for right in 0..16 {
                let a = set(left);
                let b = set(right);
                let expected_a = as_btree(&a);
                let expected_b = as_btree(&b);

                assert_eq!(
                    a.union(&b).iter().collect::<Vec<_>>(),
                    expected_a.union(&expected_b).copied().collect::<Vec<_>>()
                );
                assert_eq!(
                    a.intersection(&b).iter().collect::<Vec<_>>(),
                    expected_a
                        .intersection(&expected_b)
                        .copied()
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    a.difference(&b).iter().collect::<Vec<_>>(),
                    expected_a
                        .difference(&expected_b)
                        .copied()
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.is_subset_of(&b), expected_a.is_subset(&expected_b));
            }
        }
    }

    #[test]
    fn label_set_is_canonical_and_serializes_as_an_array() {
        let set = LabelSet::new(vec![LabelId(2), LabelId(1), LabelId(2)]);
        assert_eq!(set.iter().collect::<Vec<_>>(), [LabelId(1), LabelId(2)]);
        assert_eq!(set.len(), 2);
        assert!(set.contains(LabelId(1)));
        assert!(!set.contains(LabelId(3)));
        assert!(!set.is_empty());
        assert!(LabelSet::<LabelId>::default().is_empty());
        assert_eq!(serde_json::to_string(&set).unwrap(), "[1,2]");
    }
}
