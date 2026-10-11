//! Concrete multisets and condensed configurations with sets of label choices.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{LabelId, LabelSet, ValidationError};

/// One concrete multiset, such as `A^2 B`, rather than a family of choices.
/// Entries are sorted by label, unique, and have positive multiplicities.
/// The empty configuration has degree zero. 
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct Configuration<L = LabelId> {
    multiplicities: Vec<(L, u32)>,
}

impl<L> Default for Configuration<L> {
    fn default() -> Self {
        Self {
            multiplicities: Vec::new(),
        }
    }
}

impl<L: Copy + Ord> Configuration<L> {
    /// Merge repeated labels, discard zero counts, and check degree overflow.
    pub fn new(mut multiplicities: Vec<(L, u32)>) -> Result<Self, ValidationError> {
        multiplicities.retain(|&(_, count)| count != 0);
        multiplicities.sort_unstable_by_key(|&(label, _)| label);
        let mut entries: Vec<(L, u32)> = Vec::with_capacity(multiplicities.len());
        let mut degree = 0u32;
        for (label, count) in multiplicities {
            degree = degree
                .checked_add(count)
                .ok_or_else(|| ValidationError::new("Configuration degree is too large."))?;
            match entries.last_mut() {
                Some((previous, total)) if *previous == label => *total += count,
                _ => entries.push((label, count)),
            }
        }
        Ok(Self {
            multiplicities: entries,
        })
    }

    /// Convert an ordered list of occurrences into its underlying multiset.
    pub fn from_occurrences(
        occurrences: impl IntoIterator<Item = L>,
    ) -> Result<Self, ValidationError> {
        Self::new(occurrences.into_iter().map(|label| (label, 1)).collect())
    }

    pub fn multiplicities(&self) -> &[(L, u32)] {
        &self.multiplicities
    }

    pub fn degree(&self) -> u32 {
        self.multiplicities.iter().map(|&(_, count)| count).sum()
    }

    /// Expand only when an algorithm needs an entry for each occurrence or port.
    pub fn occurrences(&self) -> impl Iterator<Item = L> + '_ {
        self.multiplicities
            .iter()
            .flat_map(|&(label, count)| std::iter::repeat_n(label, count as usize))
    }
}

/// A condensed configuration: each occurrence chooses independently from its part.
/// For example, `AB^2` denotes the multisets `AA`, `AB`, and `BB`.
/// Zero-count parts are discarded. Otherwise, equality compares stored parts;
/// it does not perform semantic normalization.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct CondensedConfiguration<L = LabelId> {
    parts: Vec<Part<L>>,
}

impl<L> CondensedConfiguration<L> {
    /// An empty list of parts represents the unique degree-zero configuration.
    /// Parts with zero multiplicity are discarded before storing the configuration.
    pub fn new(mut parts: Vec<Part<L>>) -> Result<Self, ValidationError> {
        parts.retain(|part| part.multiplicity != 0);
        parts
            .iter()
            .try_fold(0u32, |sum, part| sum.checked_add(part.multiplicity))
            .ok_or_else(|| ValidationError::new("Configuration degree is too large."))?;
        Ok(Self { parts })
    }

    pub fn parts(&self) -> &[Part<L>] {
        &self.parts
    }

    pub fn degree(&self) -> u32 {
        self.parts.iter().map(|part| part.multiplicity).sum()
    }
}

/// A part is a set with an exponent.
/// The set must be nonempty.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, TS)]
pub struct Part<L = LabelId> {
    labels: LabelSet<L>,
    multiplicity: u32,
}

impl<L: Copy + Ord> Part<L> {
    pub fn new(labels: LabelSet<L>, multiplicity: u32) -> Result<Self, ValidationError> {
        if labels.is_empty() {
            return Err(ValidationError::new("A part needs at least one label."));
        }
        Ok(Self {
            labels,
            multiplicity,
        })
    }

    pub fn labels(&self) -> &LabelSet<L> {
        &self.labels
    }
    pub fn multiplicity(&self) -> u32 {
        self.multiplicity
    }
}
