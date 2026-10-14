//! LLM: Concrete multisets and condensed configurations with sets of label choices.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{LabelId, LabelSet, ValidationError};

/// LLM: One concrete multiset stored as sorted, distinct `(value, multiplicity)` entries with
/// LLM: positive counts. Repeated values merge, the sum is the degree, and an empty list
/// LLM: represents degree zero. A value can itself be a set in the maximization algebra; it is
/// LLM: still one concrete value here.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct Configuration<L = LabelId> {
    multiplicities: Vec<(L, u32)>,
}

impl<L> Default for Configuration<L> {
    /// LLM: Create the empty initial configuration state. Used before collecting values or
    /// LLM: admitting computed rows.
    fn default() -> Self {
        // LLM: Represent the unique concrete degree-zero multiset with no entries.
        Self {
            multiplicities: Vec::new(),
        }
    }
}

impl<L: Ord> Configuration<L> {
    /// LLM: Canonicalize a concrete counted multiset and reject degree overflow. Used wherever
    /// LLM: algorithms construct concrete configurations or algebraic counted rows.
    pub fn new(mut multiplicities: Vec<(L, u32)>) -> Result<Self, ValidationError> {
        // LLM: Drop zero counts and sort by value so equal entries can be merged in a single
        // LLM: pass.
        multiplicities.retain(|(_, count)| *count != 0);
        multiplicities.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let mut entries: Vec<(L, u32)> = Vec::with_capacity(multiplicities.len());
        let mut degree = 0u32;
        // LLM: Reject degree/count overflow rather than silently wrapping a model invariant.
        for (label, count) in multiplicities {
            // LLM: Reject degree/count overflow rather than silently wrapping a model
            // LLM: invariant.
            degree = degree
                .checked_add(count)
                .ok_or_else(|| ValidationError::new("Configuration degree is too large."))?;
            match entries.last_mut() {
                Some((previous, total)) if *previous == label => *total += count,
                _ => entries.push((label, count)),
            }
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            multiplicities: entries,
        })
    }

    /// LLM: Convert ordered occurrences to their underlying counted multiset. Used when port
    /// LLM: assignments must be checked against permutation-invariant constraints.
    pub fn from_occurrences(
        occurrences: impl IntoIterator<Item = L>,
    ) -> Result<Self, ValidationError> {
        // LLM: Count each occurrence once and let the checked multiset constructor merge
        // LLM: duplicates.
        Self::new(occurrences.into_iter().map(|label| (label, 1)).collect())
    }
}

impl<L> Configuration<L> {
    /// LLM: Return the canonical value/count entries from Configuration. Used by the problem
    /// LLM: model and its algorithms without exposing mutable storage.
    pub fn multiplicities(&self) -> &[(L, u32)] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.multiplicities
    }

    /// LLM: Return the sum of occurrence counts from Configuration. Used by the problem model
    /// LLM: and its algorithms without exposing mutable storage.
    pub fn degree(&self) -> u32 {
        // LLM: Sum the stored multiplicities; construction has already checked that this total
        // LLM: fits u32.
        self.multiplicities.iter().map(|(_, count)| count).sum()
    }
}

impl<L: Clone> Configuration<L> {
    /// LLM: Expand counted values lazily into individual occurrences. Used only when a caller
    /// LLM: needs port-level assignments or a small explicit oracle.
    pub fn occurrences(&self) -> impl Iterator<Item = L> + '_ {
        // LLM: Yield the stored value once per occurrence, expanding lazily only when the
        // LLM: caller consumes the iterator.
        self.multiplicities
            .iter()
            .flat_map(|(label, count)| std::iter::repeat_n(label.clone(), *count as usize))
    }
}

/// LLM: A family of multisets represented by parts. Each occurrence independently chooses a
/// LLM: label from its part, so `AB^2` permits AA, AB, and BB. Zero-count parts are dropped;
/// LLM: stored part order and grouping are not semantically normalized.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct CondensedConfiguration<L = LabelId> {
    parts: Vec<Part<L>>,
}

impl<L> CondensedConfiguration<L> {
    /// LLM: Drop zero-count parts and reject degree overflow while retaining the remaining
    /// LLM: condensed structure. Used by parsing and transformations; no semantic
    /// LLM: normalization occurs.
    pub fn new(mut parts: Vec<Part<L>>) -> Result<Self, ValidationError> {
        // LLM: Omit zero-occurrence parts, then check the total degree without expanding any
        // LLM: occurrences.
        parts.retain(|part| part.multiplicity != 0);
        parts
            .iter()
            .try_fold(0u32, |sum, part| sum.checked_add(part.multiplicity))
            .ok_or_else(|| ValidationError::new("Configuration degree is too large."))?;
        Ok(Self { parts })
    }

    /// LLM: Return the stored condensed parts from CondensedConfiguration. Used by the problem
    /// LLM: model and its algorithms without exposing mutable storage.
    pub fn parts(&self) -> &[Part<L>] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.parts
    }

    /// LLM: Return the sum of occurrence counts from CondensedConfiguration. Used by the
    /// LLM: problem model and its algorithms without exposing mutable storage.
    pub fn degree(&self) -> u32 {
        // LLM: Sum the stored multiplicities; construction has already checked that this total
        // LLM: fits u32.
        self.parts.iter().map(|part| part.multiplicity).sum()
    }
}

/// LLM: A nonempty set of label choices and a multiplicity. Each of these occurrences chooses
/// LLM: independently from the same set. A zero count is accepted at construction and omitted
/// LLM: when forming a condensed configuration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct Part<L = LabelId> {
    labels: LabelSet<L>,
    multiplicity: u32,
}

impl<L: Copy + Ord> Part<L> {
    /// LLM: Store a nonempty choice set and its occurrence count. Used to build condensed
    /// LLM: rows; zero counts are allowed here and removed by the configuration constructor.
    pub fn new(labels: LabelSet<L>, multiplicity: u32) -> Result<Self, ValidationError> {
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if labels.is_empty() {
            return Err(ValidationError::new("A part needs at least one label."));
        }
        Ok(Self {
            labels,
            multiplicity,
        })
    }

    /// LLM: Return the label data from Part. Used by the problem model and its algorithms
    /// LLM: without exposing mutable storage.
    pub fn labels(&self) -> &LabelSet<L> {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.labels
    }
    /// LLM: Return the number of independent occurrences from Part. Used by the problem model
    /// LLM: and its algorithms without exposing mutable storage.
    pub fn multiplicity(&self) -> u32 {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.multiplicity
    }
}
