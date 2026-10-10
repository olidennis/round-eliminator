//! Concrete configurations store one multiplicity per distinct label.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ValidationError;
use crate::labels::LabelId;

/// One concrete multiset, such as `A^2 B`, rather than a family of choices.
/// Entries are sorted by label, unique, and have positive multiplicities.
/// The empty configuration has degree zero. Input/output pairs can be labels too.
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
