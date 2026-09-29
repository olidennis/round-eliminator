//! Compact constraints shared by ordinary labels and input/output pairs.

use std::collections::BTreeMap;

use serde::Serialize;
use ts_rs::TS;

use super::ValidationError;
use crate::labels::{LabelId, LabelSet};

/// Allowed condensed configurations, grouped by their degree.
/// Missing degrees have no allowed configurations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Constraint<L = LabelId> {
    degrees: Vec<DegreeGroup<L>>,
}

impl<L: Copy + Ord> Constraint<L> {
    pub fn new(configurations: Vec<CondensedConfiguration<L>>) -> Self {
        let mut degrees: BTreeMap<u32, Vec<CondensedConfiguration<L>>> = BTreeMap::new();
        for configuration in configurations {
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

    pub fn labels(&self) -> impl Iterator<Item = L> + '_ {
        self.configurations().flat_map(|configuration| {
            configuration
                .parts
                .iter()
                .flat_map(|part| part.labels.iter())
        })
    }
}

/// All stored configurations in this group have exactly this degree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
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

/// A condensed configuration: each occurrence chooses independently from its part.
/// For example, `AB^2` denotes the multisets `AA`, `AB`, and `BB`.
/// Zero-count parts are discarded. Otherwise, equality compares stored parts;
/// it does not perform semantic normalization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
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
