//! The source definition of a problem, without computed properties or UI history.

use serde::Serialize;
use ts_rs::TS;

/// A compact label reference. Both sides of an input-free problem use one table.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, TS)]
pub struct LabelId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct PlainProblem {
    pub labels: Vec<String>,
    pub active: Constraint<LabelId>,
    pub passive: Constraint<LabelId>,
}

/// Allowed configurations, grouped by exact degree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Constraint<L> {
    pub degrees: Vec<DegreeGroup<L>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct DegreeGroup<L> {
    pub degree: u32,
    pub configurations: Vec<Configuration<L>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Configuration<L> {
    pub parts: Vec<Part<L>>,
}

/// A multiset part: one label is chosen from `labels` at each occurrence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Part<L> {
    pub labels: Vec<L>,
    pub multiplicity: u32,
}
