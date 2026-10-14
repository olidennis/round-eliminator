//! LLM: Collections of allowed configurations, grouped by degree.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CondensedConfiguration, LabelId};

/// LLM: The union of allowed condensed configurations, partitioned into sorted degree groups.
/// LLM: Missing degrees have no allowed configurations. Exact duplicate rows are discarded,
/// LLM: but equivalent differently condensed rows may remain.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Constraint<L = LabelId> {
    degrees: Vec<DegreeGroup<L>>,
}

impl<L: Copy + Ord> Constraint<L> {
    /// LLM: Group rows by degree and remove exact structural duplicates while preserving the
    /// LLM: first row within each degree. Used by parsing and computed constraint
    /// LLM: construction.
    pub fn new(configurations: Vec<CondensedConfiguration<L>>) -> Self {
        // LLM: Collect degree groups in sorted-key order and track exact rows separately for
        // LLM: deduplication.
        let mut degrees: BTreeMap<u32, Vec<CondensedConfiguration<L>>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        // LLM: Keep the first occurrence of each structural row and place it in its
        // LLM: exact-degree group.
        for configuration in configurations {
            // LLM: Skip only exact duplicates; this does not normalize differently condensed
            // LLM: representations.
            if !seen.insert(configuration.clone()) {
                continue;
            }
            degrees
                .entry(configuration.degree())
                .or_default()
                .push(configuration);
        }
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            degrees: degrees
                .into_iter()
                .map(|(degree, configurations)| DegreeGroup {
                    degree,
                    configurations,
                })
                .collect(),
        }
    }

    /// LLM: Return the ordered degree groups from Constraint. Used by the problem model and
    /// LLM: its algorithms without exposing mutable storage.
    pub fn degrees(&self) -> &[DegreeGroup<L>] {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        &self.degrees
    }

    /// LLM: Return the stored allowed configuration rows from Constraint. Used by the problem
    /// LLM: model and its algorithms without exposing mutable storage.
    pub fn configurations(&self) -> impl Iterator<Item = &CondensedConfiguration<L>> {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.degrees.iter().flat_map(|group| &group.configurations)
    }

    /// LLM: Borrow exactly the rows of a requested degree, or an empty slice when none exist.
    /// LLM: Used by algorithms that enforce the independently stored graph class.
    pub fn configurations_at_degree(&self, degree: u32) -> &[CondensedConfiguration<L>] {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.degrees
            .binary_search_by_key(&degree, DegreeGroup::degree)
            .map(|index| self.degrees[index].configurations())
            .unwrap_or(&[])
    }

    /// LLM: Visit labels occurring in stored parts, possibly more than once. Used to collect
    /// LLM: relevant alphabets; callers deduplicate when needed.
    pub fn labels(&self) -> impl Iterator<Item = L> + '_ {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.configurations().flat_map(|configuration| {
            configuration
                .parts()
                .iter()
                .flat_map(|part| part.labels().iter())
        })
    }
}

/// LLM: The allowed condensed configurations of one exact degree. Every stored row has that
/// LLM: degree; its position inside the enclosing constraint follows increasing degree order.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DegreeGroup<L = LabelId> {
    degree: u32,
    configurations: Vec<CondensedConfiguration<L>>,
}

impl<L> DegreeGroup<L> {
    /// LLM: Return the sum of occurrence counts from DegreeGroup. Used by the problem model
    /// LLM: and its algorithms without exposing mutable storage.
    pub fn degree(&self) -> u32 {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.degree
    }
    /// LLM: Return the stored allowed configuration rows from DegreeGroup. Used by the problem
    /// LLM: model and its algorithms without exposing mutable storage.
    pub fn configurations(&self) -> &[CondensedConfiguration<L>] {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        &self.configurations
    }
}
