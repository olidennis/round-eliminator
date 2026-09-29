//! Validated problem definitions built from ordinary or input/output constraint pairs.
//! Each concrete constraint-pair type owns and validates its label tables.

mod constraint;
mod constraint_pair;
mod graph;

use std::fmt;

use serde::Serialize;
use ts_rs::TS;

use crate::labels::{LabelId, LabelSet};
pub use constraint::{CondensedConfiguration, Constraint, DegreeGroup, Part};
pub use constraint_pair::{ConstraintPair, InputOutputConstraintPair};
pub use graph::GraphClass;

/// a ValidationError represents an error obtained when constructing a problem
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError(String);

impl ValidationError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ValidationError {}

/// A problem can be of four types:
/// - Plain: just two constraints
/// - Independent: we have an input and an output, but the output validity does not depend on the input
/// - Paired: the output validity depends on the input, and we list valid configurations of input-output pairs
/// - Mapped: we have an input and an output, and for each input label we list valid output labels
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Problem {
    Plain(PlainProblem),
    Independent(IndependentProblem),
    Paired(PairedProblem),
    Mapped(MappedProblem),
}

/// PlainProblem: it is just a pair of constraints,
/// but we also remember in which graph class we want to solve the problem,
/// that is, just a list of possible degrees for each side.
/// Output label names are stored with the output constraints.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct PlainProblem {
    graph_class: GraphClass,
    output: ConstraintPair,
}

impl PlainProblem {
    pub fn new(graph_class: GraphClass, output: ConstraintPair) -> Self {
        Self {
            graph_class,
            output,
        }
    }
    pub fn graph_class(&self) -> &GraphClass {
        &self.graph_class
    }
    pub fn output(&self) -> &ConstraintPair {
        &self.output
    }
}

/// IndependentProblem has two pairs of constraints,
/// one for the input and one for the output.
/// This problem represents the situation in which we want to solve the output problem,
/// under the promise that we are given a solution for the input problem.
/// As in the case of PlainProblem,
/// we remember in which graph class we want to solve the problem.
/// Each constraint pair stores its own label names.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct IndependentProblem {
    graph_class: GraphClass,
    input: ConstraintPair,
    output: ConstraintPair,
}

impl IndependentProblem {
    pub fn new(graph_class: GraphClass, input: ConstraintPair, output: ConstraintPair) -> Self {
        Self {
            graph_class,
            input,
            output,
        }
    }
    pub fn graph_class(&self) -> &GraphClass {
        &self.graph_class
    }
    pub fn input(&self) -> &ConstraintPair {
        &self.input
    }
    pub fn output(&self) -> &ConstraintPair {
        &self.output
    }
}

/// MappedProblem is similar to IndependentProblem.
/// The difference is that now the output validity may depend on the input
/// We describe it by listing, for each label i,
/// the set allowed_output[i] of allowed outputs
/// that can go on edges labeled i.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct MappedProblem {
    graph_class: GraphClass,
    input: ConstraintPair,
    output: ConstraintPair,
    allowed_outputs: Vec<LabelSet>,
}

impl MappedProblem {
    pub fn new(
        graph_class: GraphClass,
        input: ConstraintPair,
        output: ConstraintPair,
        allowed_outputs: Vec<LabelSet>,
    ) -> Result<Self, ValidationError> {
        if allowed_outputs.len() != input.labels().len() {
            return Err(ValidationError::new(
                "Specify allowed outputs for every input label.",
            ));
        }
        if allowed_outputs
            .iter()
            .flat_map(LabelSet::iter)
            .any(|id| id.0 as usize >= output.labels().len())
        {
            return Err(ValidationError::new(
                "The input/output map refers to an unknown output label.",
            ));
        }
        Ok(Self {
            graph_class,
            input,
            output,
            allowed_outputs,
        })
    }
    pub fn graph_class(&self) -> &GraphClass {
        &self.graph_class
    }
    pub fn input(&self) -> &ConstraintPair {
        &self.input
    }
    pub fn output(&self) -> &ConstraintPair {
        &self.output
    }
    pub fn allowed_outputs(&self) -> &[LabelSet] {
        &self.allowed_outputs
    }
}

/// A pair of Labels. `input` and `output` will index separate string tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, TS)]
pub struct LabelPair {
    pub input: LabelId,
    pub output: LabelId,
}

/// PairedProblem is a problem with input,
/// where a problem is described using configurations of input-output pairs.
/// When constructing a problem, we extract the input problem explicitly,
/// and we store it as `input`.
/// As in the other cases, the user can specify which degrees we care about,
/// and it is assumed that the graph comes with a solution for the input problem.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct PairedProblem {
    graph_class: GraphClass,
    input: ConstraintPair,
    constraints: InputOutputConstraintPair,
}

impl PairedProblem {
    pub fn new(
        graph_class: GraphClass,
        constraints: InputOutputConstraintPair,
    ) -> Result<Self, ValidationError> {
        let input = constraints.input_projection()?;
        Ok(Self {
            graph_class,
            input,
            constraints,
        })
    }

    pub fn graph_class(&self) -> &GraphClass {
        &self.graph_class
    }
    pub fn input(&self) -> &ConstraintPair {
        &self.input
    }
    pub fn constraints(&self) -> &InputOutputConstraintPair {
        &self.constraints
    }
}

#[cfg(test)]
mod tests;
