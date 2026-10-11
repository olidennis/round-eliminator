//! Domain model: problems, constraints, configurations, labels, and graph classes.
//! Implementation modules are private; callers import types through `model`.

mod configuration;
mod constraint;
mod constraint_pair;
mod graph;
mod labels;
mod problem;

pub use configuration::{CondensedConfiguration, Configuration, Part};
pub use constraint::{Constraint, DegreeGroup};
pub use constraint_pair::ConstraintPair;
pub use graph::GraphClass;
pub(crate) use labels::validate_names;
pub use labels::{LabelId, LabelSet};
pub use problem::{Input, OutputMapping, Problem, ValidationError};

#[cfg(test)]
mod tests;
