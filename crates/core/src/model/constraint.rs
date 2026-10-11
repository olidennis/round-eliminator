//! Collections of allowed configurations, grouped by degree.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CondensedConfiguration, LabelId};

/// Allowed condensed configurations, grouped by their degree.
/// Missing degrees have no allowed configurations.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Constraint<L = LabelId> {
    degrees: Vec<DegreeGroup<L>>,
}

impl<L: Copy + Ord> Constraint<L> {
    /// Group by degree and discard structurally identical configurations,
    /// preserving the first occurrence within each degree. Parts are not normalized.
    pub fn new(configurations: Vec<CondensedConfiguration<L>>) -> Self {
        let mut degrees: BTreeMap<u32, Vec<CondensedConfiguration<L>>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for configuration in configurations {
            if !seen.insert(configuration.clone()) {
                continue;
            }
            degrees
                .entry(configuration.degree())
                .or_default()
                .push(configuration);
        }
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

    pub fn degrees(&self) -> &[DegreeGroup<L>] {
        &self.degrees
    }

    pub fn configurations(&self) -> impl Iterator<Item = &CondensedConfiguration<L>> {
        self.degrees.iter().flat_map(|group| &group.configurations)
    }

    /// Return the allowed configurations of exactly this degree, or an empty
    /// slice if the constraint has none. Degree groups are stored in order.
    pub fn configurations_at_degree(&self, degree: u32) -> &[CondensedConfiguration<L>] {
        self.degrees
            .binary_search_by_key(&degree, DegreeGroup::degree)
            .map(|index| self.degrees[index].configurations())
            .unwrap_or(&[])
    }

    pub fn labels(&self) -> impl Iterator<Item = L> + '_ {
        self.configurations().flat_map(|configuration| {
            configuration
                .parts()
                .iter()
                .flat_map(|part| part.labels().iter())
        })
    }
}

/// All stored configurations in this group have exactly this degree.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DegreeGroup<L = LabelId> {
    degree: u32,
    configurations: Vec<CondensedConfiguration<L>>,
}

impl<L> DegreeGroup<L> {
    pub fn degree(&self) -> u32 {
        self.degree
    }
    pub fn configurations(&self) -> &[CondensedConfiguration<L>] {
        &self.configurations
    }
}
