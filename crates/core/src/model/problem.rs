//! Definition of a problem. Constraints can depend on inputs.
//! As a special case, constraints do not depend on inputs.
//! As a special case of that, there are no inputs.
//! Constructors validate. However, when deserializing, we fully trust the client.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ConstraintPair, GraphClass, LabelId, LabelSet};

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

/// The input promise. `Trivial` means the same input everywhere (or equivalently, no input).
/// `Explicit` means that we have a pair of constraints representing the input problem.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Input {
    Trivial,
    Explicit(ConstraintPair),
}

/// For problems where the constraints do not depend on the input, we use `Unrestricted`.
/// Otherwise, we use `Restricted`,
/// and in position `i`` we store the set of allowed outputs for the ports with input label `i``.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum OutputMapping {
    Unrestricted,
    Restricted(Vec<LabelSet>),
}

/// A `Problem`. We are given a `graph_class` where we want to solve it,
/// we are promised to be given a valid `input`,
/// and we are required to produce an `output`,
/// that respects the requirements of `mapping`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Problem {
    graph_class: GraphClass,
    input: Input,
    output: ConstraintPair,
    mapping: OutputMapping,
}

impl Problem {
    pub fn new(
        graph_class: GraphClass,
        input: Input,
        output: ConstraintPair,
        mapping: OutputMapping,
    ) -> Result<Self, ValidationError> {
        if let OutputMapping::Restricted(entries) = &mapping {
            // Trivial has one implicit input label. Its ID, when needed by an
            // operation, is zero; it is not stored in a string table.
            let inputs = match &input {
                Input::Trivial => 1,
                Input::Explicit(input) => input.labels().len(),
            };
            if entries.len() != inputs {
                return Err(ValidationError::new(
                    "Specify allowed outputs for every input label.",
                ));
            }
            if entries
                .iter()
                .flat_map(LabelSet::iter)
                .any(|id| id.0 as usize >= output.labels().len())
            {
                return Err(ValidationError::new(
                    "The input/output map refers to an unknown output label.",
                ));
            }
        }
        Ok(Self {
            graph_class,
            input,
            output,
            mapping,
        })
    }

    pub fn graph_class(&self) -> &GraphClass {
        &self.graph_class
    }

    pub fn input(&self) -> &Input {
        &self.input
    }

    pub fn output(&self) -> &ConstraintPair {
        &self.output
    }

    pub fn mapping(&self) -> &OutputMapping {
        &self.mapping
    }

    /// Check the incidence restriction. `None` denotes the trivial input and
    /// addresses its single implicit mapping entry.
    pub fn output_allowed(&self, input: Option<LabelId>, output: LabelId) -> bool {
        match &self.mapping {
            OutputMapping::Unrestricted => true,
            OutputMapping::Restricted(entries) => {
                entries[input.unwrap_or(LabelId(0)).0 as usize].contains(output)
            }
        }
    }
}
