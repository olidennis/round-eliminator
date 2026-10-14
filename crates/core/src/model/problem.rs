//! LLM: Definition of a problem. Constraints can depend on inputs.
//! LLM: As a special case, constraints do not depend on inputs.
//! LLM: As a special case of that, there are no inputs.
//! LLM: Constructors validate. However, when deserializing, we fully trust the client.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ConstraintPair, GraphClass, LabelId, LabelSet};

/// LLM: A model-construction failure with a human-readable explanation, such as an invalid
/// LLM: label reference or degree overflow. It is distinct from a text location or a transport
/// LLM: error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError(String);

impl ValidationError {
    /// LLM: Store a model-construction explanation as a validation error. Used by checked
    /// LLM: constructors without attaching a parser-specific location.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self(message.into())
    }
}

impl fmt::Display for ValidationError {
    /// LLM: Write the stored explanation through Rust's Display formatter. Used when
    /// LLM: presenting or converting this error to a human-readable message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.0.fmt(f)
    }
}

impl std::error::Error for ValidationError {}

/// LLM: The promise supplied to the algorithm. `Trivial` denotes no input information without
/// LLM: allocating dummy constraints; `Explicit` stores the valid input configurations and
/// LLM: their own alphabet.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Input {
    Trivial,
    Explicit(ConstraintPair),
}

/// LLM: The per-incidence permission relation from inputs to outputs. `Unrestricted` allows
/// LLM: every output for every input; `Restricted[i]` contains the allowed output IDs for
/// LLM: input i, with one implicit entry for trivial input.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum OutputMapping {
    Unrestricted,
    Restricted(Vec<LabelSet>),
}

/// LLM: The canonical mathematical problem: a graph class, a promised input solution, required
/// LLM: output constraints, and per-incidence input/output permissions. All entry forms become
/// LLM: this representation. Constructors check invariants; deserialization trusts client
/// LLM: snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Problem {
    graph_class: GraphClass,
    input: Input,
    output: ConstraintPair,
    mapping: OutputMapping,
}

impl Problem {
    /// LLM: Validate total restricted input mappings and their output references before
    /// LLM: storing the canonical problem. Used by parsers and transformations; trusted
    /// LLM: deserialization does not repeat these checks.
    pub fn new(
        graph_class: GraphClass,
        input: Input,
        output: ConstraintPair,
        mapping: OutputMapping,
    ) -> Result<Self, ValidationError> {
        // LLM: Select the representation path that preserves the supplied input promise and
        // LLM: incidence permissions.
        if let OutputMapping::Restricted(entries) = &mapping {
            // LLM: Trivial has one implicit input label. Its ID, when needed by an
            // LLM: operation, is zero; it is not stored in a string table.

            let inputs = match &input {
                Input::Trivial => 1,
                Input::Explicit(input) => input.labels().len(),
            };
            // LLM: Require one permission set per input label, including the implicit
            // LLM: trivial-input entry.
            if entries.len() != inputs {
                return Err(ValidationError::new(
                    "Specify allowed outputs for every input label.",
                ));
            }
            // LLM: Reject output IDs that have no name in the output constraint pair.
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
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            graph_class,
            input,
            output,
            mapping,
        })
    }

    /// LLM: Return the independently stored graph degrees from Problem. Used by the problem
    /// LLM: model and its algorithms without exposing mutable storage.
    pub fn graph_class(&self) -> &GraphClass {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.graph_class
    }

    /// LLM: Return the original input promise from Problem. Used by the problem model and its
    /// LLM: algorithms without exposing mutable storage.
    pub fn input(&self) -> &Input {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.input
    }

    /// LLM: Return the required output constraints from Problem. Used by the problem model and
    /// LLM: its algorithms without exposing mutable storage.
    pub fn output(&self) -> &ConstraintPair {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.output
    }

    /// LLM: Return the incidence permission relation from Problem. Used by the problem model
    /// LLM: and its algorithms without exposing mutable storage.
    pub fn mapping(&self) -> &OutputMapping {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.mapping
    }

    /// LLM: Check per-incidence input/output permission. Used by diagram, transformation, and
    /// LLM: solvability code; `None` addresses the implicit trivial-input mapping entry.
    pub fn output_allowed(&self, input: Option<LabelId>, output: LabelId) -> bool {
        // LLM: Select the representation path that preserves the supplied input promise and
        // LLM: incidence permissions.
        match &self.mapping {
            OutputMapping::Unrestricted => true,
            OutputMapping::Restricted(entries) => {
                entries[input.unwrap_or(LabelId(0)).0 as usize].contains(output)
            }
        }
    }
}
