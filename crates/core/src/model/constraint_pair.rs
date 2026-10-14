//! LLM: Concrete constraint pairs own the label tables needed to interpret their IDs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Constraint, GraphClass, ValidationError, validate_names};

/// LLM: The active and passive constraints over one shared alphabet, together with its
/// LLM: readable names. ID i means `labels[i]` on either side. An input constraint pair and an
/// LLM: output constraint pair own independent alphabets.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ConstraintPair {
    labels: Vec<String>,
    active: Constraint,
    passive: Constraint,
}

impl ConstraintPair {
    /// LLM: Validate one alphabet and both sides' label references before storing the pair.
    /// LLM: Used by parsing and transformations; input/output pairs keep separate name tables.
    pub fn new(
        labels: Vec<String>,
        active: Constraint,
        passive: Constraint,
    ) -> Result<Self, ValidationError> {
        // LLM: we check that label names are unique

        validate_names(&labels)?;
        // LLM: and that each label has a name

        if active
            .labels()
            .chain(passive.labels())
            .any(|id| id.0 as usize >= labels.len())
        {
            return Err(ValidationError::new(
                "A constraint refers to a label outside its label table.",
            ));
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            labels,
            active,
            passive,
        })
    }

    /// LLM: Return the label data from ConstraintPair. Used by the problem model and its
    /// LLM: algorithms without exposing mutable storage.
    pub fn labels(&self) -> &[String] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.labels
    }
    /// LLM: Return the active-side constraint or degree list from ConstraintPair. Used by the
    /// LLM: problem model and its algorithms without exposing mutable storage.
    pub fn active(&self) -> &Constraint {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.active
    }
    /// LLM: Return the passive-side constraint or degree list from ConstraintPair. Used by the
    /// LLM: problem model and its algorithms without exposing mutable storage.
    pub fn passive(&self) -> &Constraint {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.passive
    }

    /// LLM: Extract the distinct degrees represented by the two constraints. Used only when
    /// LLM: resolving omitted graph-class fields during construction.
    pub fn inferred_graph_class(&self) -> GraphClass {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        GraphClass::from_constraints(&self.active, &self.passive)
    }
}
