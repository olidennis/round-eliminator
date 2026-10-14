//! LLM: Label identifiers and sets used by the problem model.

use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet};

use super::ValidationError;
use ts_rs::TS;

/// LLM: A numeric identifier in the name table of one constraint pair: `LabelId(i)` names
/// LLM: `labels[i]`. Input and output tables have separate ID spaces; the ID alone stores no
/// LLM: text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct LabelId(pub u32);

/// LLM: An input/output combination in a computed joint constraint or diagram. The two IDs
/// LLM: refer to the original input and output tables; `None` is the implicit trivial input or
/// LLM: an output-only vertex.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct InputOutputLabel {
    pub input: Option<LabelId>,
    pub output: LabelId,
}

/// LLM: A mathematical set backed by a sorted vector with no duplicates. Most problem sets
/// LLM: contain label IDs; generic algorithms can store other ordered values. Set operations
/// LLM: preserve canonical storage, and JSON represents it as an array.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct LabelSet<L = LabelId>(Vec<L>);

impl<L> Default for LabelSet<L> {
    /// LLM: Create the empty initial label set state. Used before collecting values or
    /// LLM: admitting computed rows.
    fn default() -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self(Vec::new())
    }
}

impl<L: Copy + Ord> LabelSet<L> {
    /// LLM: Sort and deduplicate values into a canonical set. Used by model construction and
    /// LLM: all algorithms producing label domains.
    pub fn new(mut labels: Vec<L>) -> Self {
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        labels.sort_unstable();
        labels.dedup();
        Self(labels)
    }

    /// LLM: Return the number of stored entries from LabelSet. Used by the retained comparison
    /// LLM: benchmarks without exposing mutable storage.
    pub fn len(&self) -> usize {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.0.len()
    }

    /// LLM: Report whether LabelSet has no stored entries. Used by the retained comparison
    /// LLM: benchmarks for empty-domain and empty-history cases.
    pub fn is_empty(&self) -> bool {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.0.is_empty()
    }

    /// LLM: Iterate copied values in increasing order. Used by model consumers without
    /// LLM: exposing mutable set storage.
    pub fn iter(&self) -> impl Iterator<Item = L> + '_ {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.0.iter().copied()
    }

    /// LLM: Test membership by binary search in the canonical vector. Used by mapping,
    /// LLM: matching, and condensed choice checks.
    pub fn contains(&self, label: L) -> bool {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.0.binary_search(&label).is_ok()
    }

    /// LLM: Test inclusion by scanning the two sorted sets without constructing a difference.
    /// LLM: Used by containment and permission proofs.
    pub fn is_subset_of(&self, other: &Self) -> bool {
        // LLM: Prepare `right` for the following computation; it stores the right for this
        // LLM: stage.
        let mut right = 0;
        for label in &self.0 {
            // LLM: Process the relevant label choices while preserving their table identities.
            while other
                .0
                .get(right)
                .is_some_and(|candidate| candidate < label)
            {
                right += 1;
            }
            // LLM: A missing left label disproves inclusion; no difference set needs to be
            // LLM: allocated.
            if other.0.get(right) != Some(label) {
                return false;
            }
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        true
    }

    /// LLM: Merge sorted domains into their canonical union. Used by maximization joins and
    /// LLM: emission aggregation.
    pub fn union(&self, other: &Self) -> Self {
        // LLM: Merge the ordered streams, emitting each distinct value exactly once.
        let mut labels = Vec::with_capacity(self.len() + other.len());
        let (mut left, mut right) = (0, 0);
        // LLM: Process the relevant label choices while preserving their table identities.
        while left < self.len() && right < other.len() {
            // LLM: Emit the smaller value, or one copy of an equal pair, and advance the
            // LLM: corresponding cursors.
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
        // LLM: Retain the newly constructed choices in the result for this stage.
        labels.extend_from_slice(&self.0[left..]);
        labels.extend_from_slice(&other.0[right..]);
        Self(labels)
    }

    /// LLM: Collect the common sorted values as a canonical set. Used by maximization meets
    /// LLM: and domain restriction.
    pub fn intersection(&self, other: &Self) -> Self {
        // LLM: Scan the ordered streams and allocate only enough space for their possible
        // LLM: common values.
        let mut labels = Vec::with_capacity(self.len().min(other.len()));
        let (mut left, mut right) = (0, 0);
        // LLM: Process the relevant label choices while preserving their table identities.
        while left < self.len() && right < other.len() {
            // LLM: Skip unequal values and retain equal ones, preserving sorted uniqueness.
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
        // LLM: Apply the representation's set/order operation, preserving the algebra used by
        // LLM: containment and closure.
        Self(labels)
    }

    /// LLM: Collect values present only in this set. Used by algorithms that remove label
    /// LLM: alternatives.
    pub fn difference(&self, other: &Self) -> Self {
        // LLM: Scan both domains while collecting only values not present in the right-hand
        // LLM: set.
        let mut labels = Vec::with_capacity(self.len());
        let (mut left, mut right) = (0, 0);
        // LLM: Process the relevant label choices while preserving their table identities.
        while left < self.len() && right < other.len() {
            // LLM: Keep an unmatched left value, skip a smaller right value, or discard an
            // LLM: equal pair.
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
        // LLM: Retain the newly constructed choices in the result for this stage.
        labels.extend_from_slice(&self.0[left..]);
        Self(labels)
    }
}

/// LLM: Reject empty or duplicate names within one alphabet. Used by checked model
/// LLM: constructors; input and output alphabets are validated independently.
pub(crate) fn validate_names(labels: &[String]) -> Result<(), ValidationError> {
    // LLM: Retain exact generated history, including rows already covered by stronger
    // LLM: configurations.
    let mut seen = HashSet::new();
    for label in labels {
        if label.is_empty() || !seen.insert(label) {
            return Err(ValidationError::new(
                "Label names must be nonempty and distinct within their table.",
            ));
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// LLM: Construct the four-label choice set selected by a bit mask. Used by exhaustive
    /// LLM: comparisons with the standard-library set oracle.
    fn set(mask: u8) -> LabelSet {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        LabelSet::new(
            (0..4)
                .filter(|id| mask & (1 << id) != 0)
                .map(LabelId)
                .collect(),
        )
    }

    /// LLM: Convert a label set to the standard-library set oracle. Used to test set
    /// LLM: operations independently of sorted-vector storage.
    fn as_btree(set: &LabelSet) -> BTreeSet<LabelId> {
        // LLM: Build a standard-library set as the independent oracle for the vector-backed
        // LLM: set implementation.
        set.iter().collect()
    }

    /// LLM: Regression test: verify that label set operations match set operations. Used by
    /// LLM: the test suite to protect this behavior while implementations change.
    #[test]
    fn label_set_operations_match_set_operations() {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for left in 0..16 {
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for right in 0..16 {
                // LLM: Prepare `a` for the following computation; it stores the a for this
                // LLM: stage.
                let a = set(left);
                let b = set(right);
                let expected_a = as_btree(&a);
                let expected_b = as_btree(&b);

                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
                assert_eq!(
                    a.union(&b).iter().collect::<Vec<_>>(),
                    expected_a.union(&expected_b).copied().collect::<Vec<_>>()
                );
                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
                assert_eq!(
                    a.intersection(&b).iter().collect::<Vec<_>>(),
                    expected_a
                        .intersection(&expected_b)
                        .copied()
                        .collect::<Vec<_>>()
                );
                // LLM: Compare the implementation result with the independently prepared
                // LLM: reference for this fixture.
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

    /// LLM: Regression test: verify that label set is canonical and serializes as an array.
    /// LLM: Used by the test suite to protect this behavior while implementations change.
    #[test]
    fn label_set_is_canonical_and_serializes_as_an_array() {
        // LLM: Prepare `set` for the following computation; it stores the set for this stage.
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
