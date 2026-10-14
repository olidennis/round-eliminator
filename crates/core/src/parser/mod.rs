//! LLM: Text entry for all problem variants. Parsing errors are independent of the API.

mod notation;
mod paired;

use crate::computation::Cancellation;
use crate::model::{
    ConstraintPair, GraphClass, Input, LabelSet, OutputMapping, Problem, ValidationError,
};
use notation::{Alphabet, parse_constraint, parse_labels, parse_pairs};
use paired::InputOutputConstraintPair;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};
use ts_rs::TS;

/// LLM: The active and passive editor text for a single constraint pair. Parsing interns both
/// LLM: sides into the same alphabet.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConstraintText {
    pub active: String,
    pub passive: String,
}

/// LLM: Optional active and passive degree-list text. A blank side asks the parser to infer
/// LLM: that side from the input promise, or from outputs for a plain problem.
#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DegreeText {
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub passive: String,
}

/// LLM: The four supported text-entry forms and their associated editor fields. Parsing
/// LLM: converts each form to the single canonical mapped problem; this enum is not an
/// LLM: algorithm-facing representation.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProblemText {
    Plain {
        constraints: ConstraintText,
        #[serde(default)]
        degrees: DegreeText,
    },
    Independent {
        input: ConstraintText,
        output: ConstraintText,
        #[serde(default)]
        degrees: DegreeText,
    },
    Paired {
        constraints: ConstraintText,
        #[serde(default)]
        degrees: DegreeText,
    },
    Mapped {
        input: ConstraintText,
        output: ConstraintText,
        mapping: String,
        #[serde(default)]
        degrees: DegreeText,
    },
}

/// LLM: The editor field associated with a parsing location. Separate input, output, mapping,
/// LLM: and degree fields let the UI point to the relevant text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Active,
    Passive,
    InputActive,
    InputPassive,
    Mapping,
    ActiveDegrees,
    PassiveDegrees,
}

/// LLM: A parsing location identified by editor field and optional one-based line number. No
/// LLM: line means the whole field rather than an individual row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Location {
    pub field: Field,
    pub line: Option<usize>,
}

/// LLM: A text-entry failure with an explanation and optional field/line location. Constructor
/// LLM: and cancellation errors can be converted here without inventing a text location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub location: Option<Location>,
}

impl ParseError {
    /// LLM: Attach a parser explanation to a known editor location. Used by notation, mapping,
    /// LLM: and degree readers.
    fn at(message: impl ToString, location: Location) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            message: message.to_string(),
            location: Some(location),
        }
    }
}

/// LLM: It could be that text passes the parsing,
/// LLM: but some semantic constraints are violated,
/// LLM: and we only discover that when creating the problem.

impl From<ValidationError> for ParseError {
    /// LLM: Convert the source value into ParseError, preserving its error explanation and any
    /// LLM: applicable location or cause. Used by the standard From conversion path.
    fn from(error: ValidationError) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            message: error.to_string(),
            location: None,
        }
    }
}

impl fmt::Display for ParseError {
    /// LLM: Write the stored explanation through Rust's Display formatter. Used when
    /// LLM: presenting or converting this error to a human-readable message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.message.fmt(f)
    }
}

impl std::error::Error for ParseError {}

/// LLM: Parse any supported text-entry form into the canonical problem without an external
/// LLM: cancellation handle. Used by direct Rust callers and fixtures.
pub fn parse_problem(text: ProblemText) -> Result<Problem, ParseError> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    parse_problem_with_cancellation(text, &Cancellation::default())
}

/// LLM: Parse all four entry forms with cancellation during potentially large exponent
/// LLM: expansion. Used by Start on both transports; omitted graph degrees come from inputs
/// LLM: when present.
pub fn parse_problem_with_cancellation(
    text: ProblemText,
    cancellation: &Cancellation,
) -> Result<Problem, ParseError> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    check_cancellation(cancellation)?;
    match text {
        ProblemText::Plain {
            constraints,
            degrees,
        } => {
            // LLM: Prepare required output data without modifying the original problem
            // LLM: snapshot.
            let output = parse_constraint_pair(&constraints, false, cancellation)?;
            let graph_class = resolve_degrees(&degrees, output.inferred_graph_class())?;
            Ok(Problem::new(
                graph_class,
                Input::Trivial,
                output,
                OutputMapping::Unrestricted,
            )?)
        }
        ProblemText::Independent {
            input,
            output,
            degrees,
        } => {
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let input = parse_constraint_pair(&input, true, cancellation)?;
            let output = parse_constraint_pair(&output, false, cancellation)?;
            let graph_class = resolve_degrees(&degrees, input.inferred_graph_class())?;
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(Problem::new(
                graph_class,
                Input::Explicit(input),
                output,
                OutputMapping::Unrestricted,
            )?)
        }
        ProblemText::Paired {
            constraints,
            degrees,
        } => {
            // LLM: Prepare `inputs` for the following computation; it stores the inputs for
            // LLM: this stage.
            let mut inputs = Alphabet::default();
            let mut outputs = Alphabet::default();
            let mut pairs =
                |token: &str, location| parse_pairs(token, &mut inputs, &mut outputs, location);
            let active =
                parse_constraint(&constraints.active, Field::Active, &mut pairs, cancellation)?;
            // LLM: Read same-alphabet condensed rows with their editor locations and exponent
            // LLM: grammar.
            let passive = parse_constraint(
                &constraints.passive,
                Field::Passive,
                &mut pairs,
                cancellation,
            )?;
            let constraints =
                InputOutputConstraintPair::new(inputs.names, outputs.names, active, passive)?;
            // LLM: Resolve explicit graph degrees independently from output feasibility.
            let graph_class = resolve_degrees(&degrees, constraints.inferred_graph_class())?;
            paired::convert(graph_class, constraints, cancellation)
        }
        ProblemText::Mapped {
            input,
            output,
            mapping,
            degrees,
        } => {
            // LLM: Prepare `inputs` for the following computation; it stores the inputs for
            // LLM: this stage.
            let mut inputs = Alphabet::default();
            let mut outputs = Alphabet::default();
            let (input_active, input_passive) =
                parse_sides(&input, true, &mut inputs, cancellation)?;
            let (output_active, output_passive) =
                parse_sides(&output, false, &mut outputs, cancellation)?;
            // LLM: Mapping entries may name labels that have no allowed configurations.

            let allowed_outputs = parse_mapping(&mapping, &mut inputs, &mut outputs)?;
            let input = ConstraintPair::new(inputs.names, input_active, input_passive)?;
            let output = ConstraintPair::new(outputs.names, output_active, output_passive)?;
            let graph_class = resolve_degrees(&degrees, input.inferred_graph_class())?;
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(Problem::new(
                graph_class,
                Input::Explicit(input),
                output,
                OutputMapping::Restricted(allowed_outputs),
            )?)
        }
    }
}

/// LLM: Parse both ordinary constraint sides with one shared name interner and validate their
/// LLM: references. Used by plain and independent entry.
fn parse_constraint_pair(
    text: &ConstraintText,
    input: bool,
    cancellation: &Cancellation,
) -> Result<ConstraintPair, ParseError> {
    // LLM: Prepare the local alphabet shared by the representation conversion.
    let mut alphabet = Alphabet::default();
    let (active, passive) = parse_sides(text, input, &mut alphabet, cancellation)?;
    Ok(ConstraintPair::new(alphabet.names, active, passive)?)
}

/// LLM: Read active/passive rows with the appropriate editor locations and one shared
/// LLM: alphabet. Used by ordinary and mapped entry before constructing a constraint pair.
fn parse_sides(
    text: &ConstraintText,
    input: bool,
    alphabet: &mut Alphabet,
    cancellation: &Cancellation,
) -> Result<(crate::model::Constraint, crate::model::Constraint), ParseError> {
    // LLM: Associate errors with input or output editors while sharing one alphabet between
    // LLM: the two sides.
    let fields = if input {
        (Field::InputActive, Field::InputPassive)
    } else {
        (Field::Active, Field::Passive)
    };
    let mut labels = |token: &str, location| parse_labels(token, alphabet, location);
    // LLM: Read same-alphabet condensed rows with their editor locations and exponent grammar.
    Ok((
        parse_constraint(&text.active, fields.0, &mut labels, cancellation)?,
        parse_constraint(&text.passive, fields.1, &mut labels, cancellation)?,
    ))
}

impl From<crate::computation::Cancelled> for ParseError {
    /// LLM: Convert the source value into ParseError, preserving its error explanation and any
    /// LLM: applicable location or cause. Used by the standard From conversion path.
    fn from(error: crate::computation::Cancelled) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            message: error.to_string(),
            location: None,
        }
    }
}

/// LLM: Translate cooperative interruption into a parser error. Used while parsing and
/// LLM: expanding ranges; the API recognizes the cancellation flag separately.
fn check_cancellation(cancellation: &Cancellation) -> Result<(), ParseError> {
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(cancellation.check()?)
}

/// LLM: Combine optional explicit degree lists with inferred active/passive degrees. Used by
/// LLM: every entry form before constructing the immutable graph class.
fn resolve_degrees(text: &DegreeText, inferred: GraphClass) -> Result<GraphClass, ParseError> {
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(GraphClass::new(
        parse_degrees(&text.active, Field::ActiveDegrees)?
            .unwrap_or_else(|| inferred.active().to_vec()),
        parse_degrees(&text.passive, Field::PassiveDegrees)?
            .unwrap_or_else(|| inferred.passive().to_vec()),
    ))
}

/// LLM: Read nonnegative degrees separated by spaces or commas; blank text means inference.
/// LLM: Used independently for the two graph-class editor fields.
fn parse_degrees(text: &str, field: Field) -> Result<Option<Vec<u32>>, ParseError> {
    // LLM: Handle the empty case explicitly before the general traversal or allocation path.
    if text.trim().is_empty() {
        return Ok(None);
    }
    let location = Location { field, line: None };
    let mut degrees = Vec::new();
    // LLM: Read comma/space-separated nonnegative degrees, leaving sorting and deduplication
    // LLM: to GraphClass.
    for token in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        // LLM: Reject any degree token that is not an unsigned decimal integer, with a
        // LLM: whole-field location.
        if !token.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ParseError::at(
                "Degrees must be nonnegative integers separated by commas or spaces.",
                location,
            ));
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        degrees.push(
            token
                .parse()
                .map_err(|_| ParseError::at("Degree is too large.", location))?,
        );
    }
    // LLM: Handle the empty case explicitly before the general traversal or allocation path.
    if degrees.is_empty() {
        return Err(ParseError::at(
            "Enter at least one degree, or leave the field blank to infer degrees.",
            location,
        ));
    }
    Ok(Some(degrees))
}

/// LLM: Parse one total input-to-output permission table, interning labels that need not
/// LLM: appear in a constraint row. Used by mapped entry and reports missing or duplicate
/// LLM: input mappings.
fn parse_mapping(
    text: &str,
    inputs: &mut Alphabet,
    outputs: &mut Alphabet,
) -> Result<Vec<LabelSet>, ParseError> {
    // LLM: Publish immutable antichain handles separately from their monotone removal flags.
    let mut entries = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if line.trim().is_empty() {
            continue;
        }
        let location = Location {
            field: Field::Mapping,
            line: Some(index + 1),
        };
        let (left, right) = split_mapping(line, location)?;
        // LLM: Intern readable names so equal labels share one ID within their alphabet.
        let input = parse_labels(left.trim(), inputs, location)?;
        if input.len() != 1 {
            return Err(ParseError::at(
                "A mapping must name exactly one input label.",
                location,
            ));
        }
        let id = input.iter().next().expect("one input label");
        // LLM: Intern readable names so equal labels share one ID within their alphabet.
        let outputs = parse_labels(right.trim(), outputs, location)?;
        if entries.insert(id, outputs).is_some() {
            return Err(ParseError::at(
                "This input label already has a mapping.",
                location,
            ));
        }
    }
    // LLM: Update the current working state before continuing this operation.
    inputs
        .names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            // LLM: Update the current working state before continuing this operation.
            entries
                .remove(&crate::model::LabelId(index as u32))
                .ok_or_else(|| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    ParseError::at(
                        format!("No output mapping for input label '{name}'."),
                        Location {
                            field: Field::Mapping,
                            line: None,
                        },
                    )
                })
        })
        .collect()
}

/// LLM: Find the single mapping arrow outside parenthesized label names. Used before parsing
/// LLM: each mapping line so arrows inside names are not mistaken for separators.
fn split_mapping(line: &str, location: Location) -> Result<(&str, &str), ParseError> {
    // LLM: Track whether the scan is inside a parenthesized name so only an external arrow
    // LLM: separates the mapping.
    let mut in_label = false;
    let mut separator = None;
    // LLM: Parenthesized names may themselves contain arrows. Only an arrow outside
    // LLM: a name separates input from outputs; parse_labels checks name syntax.

    for (index, character) in line.char_indices() {
        // LLM: Select the representation path that preserves the supplied input promise and
        // LLM: incidence permissions.
        match character {
            '(' => in_label = true,
            ')' => in_label = false,
            '-' if !in_label && line[index..].starts_with("->") => {
                // LLM: Remember the already covered state so this stage processes only the
                // LLM: missing work.
                let previous_separator = separator.replace(index);
                if previous_separator.is_some() {
                    return Err(ParseError::at(
                        "A mapping must contain exactly one separating '->'.",
                        location,
                    ));
                }
            }
            _ => {}
        }
    }
    // LLM: Require one separator and return the two slices without copying their text.
    let index = separator
        .ok_or_else(|| ParseError::at("Write a mapping as input -> outputs.", location))?;
    Ok((&line[..index], &line[index + 2..]))
}

#[cfg(test)]
mod tests;
