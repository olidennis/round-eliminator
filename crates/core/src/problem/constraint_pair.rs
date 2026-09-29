//! Concrete constraint pairs own the label tables needed to interpret their IDs.

use std::collections::HashSet;

use serde::Serialize;
use ts_rs::TS;

use super::{
    CondensedConfiguration, Constraint, DegreeGroup, GraphClass, LabelPair, Part, ValidationError,
};
use crate::labels::LabelSet;

/// ConstraintPair is a pair of constraints, active and passive.
/// We also store the names of the labels.
/// label i has name labels[i]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
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
        inferred_graph_class(&self.active, &self.passive)
    }
}

/// For constraints composed of input output pairs,
/// we have an explicit type,
/// where we store input and output label names separately.
/// Though, we reuse the Constraint struct, using a pair as an element
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct InputOutputConstraintPair {
    input_labels: Vec<String>,
    output_labels: Vec<String>,
    active: Constraint<LabelPair>,
    passive: Constraint<LabelPair>,
}

impl InputOutputConstraintPair {
    pub fn new(
        input_labels: Vec<String>,
        output_labels: Vec<String>,
        active: Constraint<LabelPair>,
        passive: Constraint<LabelPair>,
    ) -> Result<Self, ValidationError> {
        // we check that names are unique
        validate_names(&input_labels)?;
        validate_names(&output_labels)?;
        // and that all labels have a name
        for pair in active.labels().chain(passive.labels()) {
            if pair.input.0 as usize >= input_labels.len()
                || pair.output.0 as usize >= output_labels.len()
            {
                return Err(ValidationError::new(
                    "A pair refers to a label outside its input or output table.",
                ));
            }
        }
        Ok(Self {
            input_labels,
            output_labels,
            active,
            passive,
        })
    }

    pub fn input_labels(&self) -> &[String] {
        &self.input_labels
    }
    pub fn output_labels(&self) -> &[String] {
        &self.output_labels
    }
    pub fn active(&self) -> &Constraint<LabelPair> {
        &self.active
    }
    pub fn passive(&self) -> &Constraint<LabelPair> {
        &self.passive
    }

    /// We extract the degrees appearing in the constraints
    pub fn inferred_graph_class(&self) -> GraphClass {
        inferred_graph_class(&self.active, &self.passive)
    }

    /// Extract the input problem
    pub fn input_projection(&self) -> Result<ConstraintPair, ValidationError> {
        ConstraintPair::new(
            self.input_labels.clone(),
            project_inputs(&self.active)?,
            project_inputs(&self.passive)?,
        )
    }
}

/// We go through each constraint and extract the degrees of the configurations
fn inferred_graph_class<L: Copy + Ord>(
    active: &Constraint<L>,
    passive: &Constraint<L>,
) -> GraphClass {
    GraphClass::new(
        active.degrees().iter().map(DegreeGroup::degree).collect(),
        passive.degrees().iter().map(DegreeGroup::degree).collect(),
    )
}

/// Check that all label names within one table are different and nonempty.
fn validate_names(labels: &[String]) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    for label in labels {
        if label.is_empty() || !seen.insert(label) {
            return Err(ValidationError::new(
                "Label names must be nonempty and distinct within their table.",
            ));
        }
    }
    Ok(())
}

/// Extract the input constraint from input-output pairs.
fn project_inputs(constraint: &Constraint<LabelPair>) -> Result<Constraint, ValidationError> {
    let configurations = constraint
        .configurations()
        .map(|configuration| {
            let parts = configuration
                .parts()
                .iter()
                .map(|part| {
                    let inputs =
                        LabelSet::new(part.labels().iter().map(|pair| pair.input).collect());
                    Part::new(inputs, part.multiplicity())
                })
                .collect::<Result<Vec<_>, _>>()?;
            CondensedConfiguration::new(parts)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Constraint::new(configurations))
}
