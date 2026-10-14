//! LLM: Convert paired entry to the shared mapped model without expanding multisets.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    computation::Cancellation,
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, GraphClass, Input, LabelId, LabelSet,
        OutputMapping, Part, Problem, ValidationError, validate_names,
    },
};

/// LLM: A temporary pair of IDs into separate input and output name tables while parsing
/// LLM: paired entry. Conversion interns each distinct pair as an ordinary output label;
/// LLM: algorithms do not store this parser type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct LabelPair {
    pub(super) input: LabelId,
    pub(super) output: LabelId,
}

/// LLM: Temporary paired-entry active/passive constraints with separate input and output
/// LLM: alphabets. Their input projection supplies the original promise; conversion creates
/// LLM: the canonical problem and discards this wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InputOutputConstraintPair {
    input_labels: Vec<String>,
    output_labels: Vec<String>,
    active: Constraint<LabelPair>,
    passive: Constraint<LabelPair>,
}

impl InputOutputConstraintPair {
    /// LLM: Validate the two name tables and every pair's separate input/output references.
    /// LLM: Used only while accepting paired entry.
    pub(super) fn new(
        input_labels: Vec<String>,
        output_labels: Vec<String>,
        active: Constraint<LabelPair>,
        passive: Constraint<LabelPair>,
    ) -> Result<Self, ValidationError> {
        // LLM: we check that names are unique

        validate_names(&input_labels)?;
        validate_names(&output_labels)?;
        // LLM: and that all labels have a name

        for pair in active.labels().chain(passive.labels()) {
            // LLM: Validate the input and output ID against their respective temporary name
            // LLM: tables.
            if pair.input.0 as usize >= input_labels.len()
                || pair.output.0 as usize >= output_labels.len()
            {
                return Err(ValidationError::new(
                    "A pair refers to a label outside its input or output table.",
                ));
            }
        }
        // LLM: Store the constructed fields together after their invariants have been
        // LLM: established.
        Ok(Self {
            input_labels,
            output_labels,
            active,
            passive,
        })
    }

    /// LLM: List the original input descriptors, or the one implicit trivial input. Used to
    /// LLM: size and index coloring behavior tables without inventing a dummy alphabet.
    fn input_labels(&self) -> &[String] {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        &self.input_labels
    }
    /// LLM: Return the temporary output name table from InputOutputConstraintPair. Used by
    /// LLM: text-entry parsing without exposing mutable storage.
    fn output_labels(&self) -> &[String] {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        &self.output_labels
    }
    /// LLM: Return the active-side constraint or degree list from InputOutputConstraintPair.
    /// LLM: Used by text-entry parsing without exposing mutable storage.
    fn active(&self) -> &Constraint<LabelPair> {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.active
    }
    /// LLM: Return the passive-side constraint or degree list from InputOutputConstraintPair.
    /// LLM: Used by text-entry parsing without exposing mutable storage.
    fn passive(&self) -> &Constraint<LabelPair> {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.passive
    }

    /// LLM: We extract the degrees appearing in the constraints Used by text-entry parsing.
    pub(super) fn inferred_graph_class(&self) -> GraphClass {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        GraphClass::from_constraints(&self.active, &self.passive)
    }

    /// LLM: Project the paired active/passive constraints onto their input IDs. Used to retain
    /// LLM: the original promise before converting pairs to ordinary outputs.
    fn input_projection(&self) -> Result<ConstraintPair, ValidationError> {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        ConstraintPair::new(
            self.input_labels.clone(),
            project_inputs(&self.active)?,
            project_inputs(&self.passive)?,
        )
    }
}

/// LLM: Convert paired constraints into a canonical mapped problem while preserving their
/// LLM: original input projection. Used at the paired-entry boundary; pair IDs become ordinary
/// LLM: output labels.
pub(super) fn convert(
    graph: GraphClass,
    constraints: InputOutputConstraintPair,
    cancellation: &Cancellation,
) -> Result<Problem, super::ParseError> {
    // LLM: Retain or prepare the original promised input semantics for this stage.
    let input = constraints.input_projection()?;
    let mut pairs = BTreeSet::new();
    // LLM: Process the relevant label choices while preserving their table identities.
    for pair in constraints
        .active()
        .labels()
        .chain(constraints.passive().labels())
    {
        cancellation.check()?;
        pairs.insert(pair);
    }
    // LLM: Queue the parent pairs for this bounded generation batch.
    let pairs: Vec<_> = pairs.into_iter().collect();
    let ids: BTreeMap<_, _> = pairs
        .iter()
        .copied()
        .enumerate()
        .map(|(id, pair)| {
            // LLM: Check that computed indices and counts fit the model's integer
            // LLM: representation.
            Ok((
                pair,
                LabelId(
                    u32::try_from(id).map_err(|_| ValidationError::new("Too many pair labels."))?,
                ),
            ))
        })
        .collect::<Result<_, ValidationError>>()?;
    // LLM: Build one permission set per original input, pointing to the new output IDs that
    // LLM: represent its pairs.
    let mut allowed = vec![Vec::new(); input.labels().len()];
    for (id, pair) in pairs.iter().enumerate() {
        cancellation.check()?;
        allowed[pair.input.0 as usize].push(LabelId(id as u32));
    }
    // LLM: A tag identifies a full pair, not just its output component. The names
    // LLM: remain readable in the ordinary mapped display. Paired entry forbids
    // LLM: commas inside either name, so this naming is unambiguous.

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
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(
        labels,
        translate(constraints.active(), &ids, cancellation)?,
        translate(constraints.passive(), &ids, cancellation)?,
    )?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Problem::new(
        graph,
        Input::Explicit(input),
        output,
        OutputMapping::Restricted(allowed.into_iter().map(LabelSet::new).collect()),
    )?)
}

/// LLM: Replace parser-local input/output pairs with interned ordinary output IDs without
/// LLM: losing condensed choices or counts. Used by paired conversion for each side.
fn translate(
    constraint: &Constraint<LabelPair>,
    ids: &BTreeMap<LabelPair, LabelId>,
    cancellation: &Cancellation,
) -> Result<Constraint, super::ParseError> {
    // LLM: Prepare `configurations` for the following computation; it stores the
    // LLM: configurations for this stage.
    let mut configurations = Vec::new();
    for configuration in constraint.configurations() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let mut parts = Vec::new();
        for part in configuration.parts() {
            cancellation.check()?;
            let labels = part.labels().iter().map(|pair| ids[&pair]).collect();
            parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
        }
        configurations.push(CondensedConfiguration::new(parts)?);
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(configurations))
}

/// LLM: Project each pair choice to its input ID while retaining counts and letting the common
/// LLM: constructor remove duplicate rows. Used to establish the paired problem's promised
/// LLM: input language.
fn project_inputs(constraint: &Constraint<LabelPair>) -> Result<Constraint, ValidationError> {
    // LLM: Prepare `configurations` for the following computation; it stores the
    // LLM: configurations for this stage.
    let configurations = constraint
        .configurations()
        .map(|configuration| {
            // LLM: Prepare `parts` for the following computation; it stores the parts for this
            // LLM: stage.
            let parts = configuration
                .parts()
                .iter()
                .map(|part| {
                    let inputs =
                        LabelSet::new(part.labels().iter().map(|pair| pair.input).collect());
                    Part::new(inputs, part.multiplicity())
                })
                .collect::<Result<Vec<_>, _>>()?;
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            CondensedConfiguration::new(parts)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(configurations))
}

#[cfg(test)]
mod tests;
