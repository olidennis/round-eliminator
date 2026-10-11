//! Convert paired entry to the shared mapped model without expanding multisets.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    computation::Cancellation,
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, GraphClass, Input, LabelId, LabelSet,
        OutputMapping, Part, Problem, ValidationError, validate_names,
    },
};

/// A temporary pair indexing separate input and output tables during text entry.
/// Conversion replaces each distinct pair with an ordinary output `LabelId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct LabelPair {
    pub(super) input: LabelId,
    pub(super) output: LabelId,
}

/// Temporary constraints for paired text entry, with separate name tables.
/// The parser projects their inputs and converts each pair to an output label.
/// `Problem` never stores this type: algorithms use ordinary constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InputOutputConstraintPair {
    input_labels: Vec<String>,
    output_labels: Vec<String>,
    active: Constraint<LabelPair>,
    passive: Constraint<LabelPair>,
}

impl InputOutputConstraintPair {
    pub(super) fn new(
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

    fn input_labels(&self) -> &[String] {
        &self.input_labels
    }
    fn output_labels(&self) -> &[String] {
        &self.output_labels
    }
    fn active(&self) -> &Constraint<LabelPair> {
        &self.active
    }
    fn passive(&self) -> &Constraint<LabelPair> {
        &self.passive
    }

    /// We extract the degrees appearing in the constraints
    pub(super) fn inferred_graph_class(&self) -> GraphClass {
        GraphClass::from_constraints(&self.active, &self.passive)
    }

    /// Extract the input problem
    fn input_projection(&self) -> Result<ConstraintPair, ValidationError> {
        ConstraintPair::new(
            self.input_labels.clone(),
            project_inputs(&self.active)?,
            project_inputs(&self.passive)?,
        )
    }
}

pub(super) fn convert(
    graph: GraphClass,
    constraints: InputOutputConstraintPair,
    cancellation: &Cancellation,
) -> Result<Problem, super::ParseError> {
    let input = constraints.input_projection()?;
    let mut pairs = BTreeSet::new();
    for pair in constraints
        .active()
        .labels()
        .chain(constraints.passive().labels())
    {
        cancellation.check()?;
        pairs.insert(pair);
    }
    let pairs: Vec<_> = pairs.into_iter().collect();
    let ids: BTreeMap<_, _> = pairs
        .iter()
        .copied()
        .enumerate()
        .map(|(id, pair)| {
            Ok((
                pair,
                LabelId(
                    u32::try_from(id).map_err(|_| ValidationError::new("Too many pair labels."))?,
                ),
            ))
        })
        .collect::<Result<_, ValidationError>>()?;
    let mut allowed = vec![Vec::new(); input.labels().len()];
    for (id, pair) in pairs.iter().enumerate() {
        cancellation.check()?;
        allowed[pair.input.0 as usize].push(LabelId(id as u32));
    }
    // A tag identifies a full pair, not just its output component. The names
    // remain readable in the ordinary mapped display. Paired entry forbids
    // commas inside either name, so this naming is unambiguous.
    let labels = pairs
        .iter()
        .map(|pair| {
            format!(
                "{},{}",
                constraints.input_labels()[pair.input.0 as usize],
                constraints.output_labels()[pair.output.0 as usize]
            )
        })
        .collect();
    let output = ConstraintPair::new(
        labels,
        translate(constraints.active(), &ids, cancellation)?,
        translate(constraints.passive(), &ids, cancellation)?,
    )?;
    Ok(Problem::new(
        graph,
        Input::Explicit(input),
        output,
        OutputMapping::Restricted(allowed.into_iter().map(LabelSet::new).collect()),
    )?)
}

fn translate(
    constraint: &Constraint<LabelPair>,
    ids: &BTreeMap<LabelPair, LabelId>,
    cancellation: &Cancellation,
) -> Result<Constraint, super::ParseError> {
    let mut configurations = Vec::new();
    for configuration in constraint.configurations() {
        cancellation.check()?;
        let mut parts = Vec::new();
        for part in configuration.parts() {
            cancellation.check()?;
            let labels = part.labels().iter().map(|pair| ids[&pair]).collect();
            parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
        }
        configurations.push(CondensedConfiguration::new(parts)?);
    }
    Ok(Constraint::new(configurations))
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

#[cfg(test)]
mod tests;
