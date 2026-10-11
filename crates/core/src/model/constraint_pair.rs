//! Concrete constraint pairs own the label tables needed to interpret their IDs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Constraint, GraphClass, ValidationError, validate_names};

/// ConstraintPair is a pair of constraints, active and passive.
/// We also store the names of the labels.
/// label `i`` has name `labels[i]``
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ConstraintPair {
    labels: Vec<String>,
    active: Constraint,
    passive: Constraint,
}

impl ConstraintPair {
    pub fn new(
        labels: Vec<String>,
        active: Constraint,
        passive: Constraint,
    ) -> Result<Self, ValidationError> {
        // we check that label names are unique
        validate_names(&labels)?;
        // and that each label has a name
        if active
            .labels()
            .chain(passive.labels())
            .any(|id| id.0 as usize >= labels.len())
        {
            return Err(ValidationError::new(
                "A constraint refers to a label outside its label table.",
            ));
        }
        Ok(Self {
            labels,
            active,
            passive,
        })
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }
    pub fn active(&self) -> &Constraint {
        &self.active
    }
    pub fn passive(&self) -> &Constraint {
        &self.passive
    }

    /// We extract the degrees appearing in the constraints.
    pub fn inferred_graph_class(&self) -> GraphClass {
        GraphClass::from_constraints(&self.active, &self.passive)
    }
}
