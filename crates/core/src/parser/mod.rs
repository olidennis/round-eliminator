//! Text entry for all problem variants. Parsing errors are independent of the API.

mod notation;

use crate::{
    labels::LabelSet,
    problem::{
        ConstraintPair, GraphClass, IndependentProblem, InputOutputConstraintPair, MappedProblem,
        PairedProblem, PlainProblem, Problem, ValidationError,
    },
};
use notation::{Alphabet, parse_constraint, parse_labels, parse_pairs};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};
use ts_rs::TS;


/// The UI sends us 2 strings for the 2 constraints
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConstraintText {
    pub active: String,
    pub passive: String,
}

/// To specify the degrees of the graph class, 
/// the UI sends 2 strings.
#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DegreeText {
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub passive: String,
}

/// For each possible problem type, 
/// we receive different text fields.
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

/// These are all the possible text fields that the user could fill.
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

/// A text-entry location. Line numbers are one-based; None refers to the whole field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Location {
    pub field: Field,
    pub line: Option<usize>,
}

/// In case of error, we give a message and a location, which is a text field and a line number.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub location: Option<Location>,
}

impl ParseError {
    fn at(message: impl ToString, location: Location) -> Self {
        Self {
            message: message.to_string(),
            location: Some(location),
        }
    }
}

/// It could be that text passes the parsing,
/// but some semantic constraints are violated,
/// and we only discover that when creating the problem.
impl From<ValidationError> for ParseError {
    fn from(error: ValidationError) -> Self {
        Self {
            message: error.to_string(),
            location: None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for ParseError {}


/// depending on the problem type, we have different strings to parse
pub fn parse_problem(text: ProblemText) -> Result<Problem, ParseError> {
    match text {
        ProblemText::Plain {
            constraints,
            degrees,
        } => {
            let output = parse_constraint_pair(&constraints, false)?;
            let graph_class = resolve_degrees(&degrees, output.inferred_graph_class())?;
            Ok(Problem::Plain(PlainProblem::new(graph_class, output)))
        }
        ProblemText::Independent {
            input,
            output,
            degrees,
        } => {
            let input = parse_constraint_pair(&input, true)?;
            let output = parse_constraint_pair(&output, false)?;
            let graph_class = resolve_degrees(&degrees, input.inferred_graph_class())?;
            Ok(Problem::Independent(IndependentProblem::new(
                graph_class,
                input,
                output,
            )))
        }
        ProblemText::Paired {
            constraints,
            degrees,
        } => {
            let mut inputs = Alphabet::default();
            let mut outputs = Alphabet::default();
            let mut pairs =
                |token: &str, location| parse_pairs(token, &mut inputs, &mut outputs, location);
            let active = parse_constraint(&constraints.active, Field::Active, &mut pairs)?;
            let passive = parse_constraint(&constraints.passive, Field::Passive, &mut pairs)?;
            let constraints =
                InputOutputConstraintPair::new(inputs.names, outputs.names, active, passive)?;
            let graph_class = resolve_degrees(&degrees, constraints.inferred_graph_class())?;
            Ok(Problem::Paired(PairedProblem::new(
                graph_class,
                constraints,
            )?))
        }
        ProblemText::Mapped {
            input,
            output,
            mapping,
            degrees,
        } => {
            let mut inputs = Alphabet::default();
            let mut outputs = Alphabet::default();
            let (input_active, input_passive) = parse_sides(&input, true, &mut inputs)?;
            let (output_active, output_passive) = parse_sides(&output, false, &mut outputs)?;
            // Mapping entries may name labels that have no allowed configurations.
            let allowed_outputs = parse_mapping(&mapping, &mut inputs, &mut outputs)?;
            let input = ConstraintPair::new(inputs.names, input_active, input_passive)?;
            let output = ConstraintPair::new(outputs.names, output_active, output_passive)?;
            let graph_class = resolve_degrees(&degrees, input.inferred_graph_class())?;
            Ok(Problem::Mapped(MappedProblem::new(
                graph_class,
                input,
                output,
                allowed_outputs,
            )?))
        }
    }
}

fn parse_constraint_pair(text: &ConstraintText, input: bool) -> Result<ConstraintPair, ParseError> {
    let mut alphabet = Alphabet::default();
    let (active, passive) = parse_sides(text, input, &mut alphabet)?;
    Ok(ConstraintPair::new(alphabet.names, active, passive)?)
}

fn parse_sides(
    text: &ConstraintText,
    input: bool,
    alphabet: &mut Alphabet,
) -> Result<(crate::problem::Constraint, crate::problem::Constraint), ParseError> {
    let fields = if input {
        (Field::InputActive, Field::InputPassive)
    } else {
        (Field::Active, Field::Passive)
    };
    let mut labels = |token: &str, location| parse_labels(token, alphabet, location);
    Ok((
        parse_constraint(&text.active, fields.0, &mut labels)?,
        parse_constraint(&text.passive, fields.1, &mut labels)?,
    ))
}

fn resolve_degrees(text: &DegreeText, inferred: GraphClass) -> Result<GraphClass, ParseError> {
    Ok(GraphClass::new(
        parse_degrees(&text.active, Field::ActiveDegrees)?
            .unwrap_or_else(|| inferred.active().to_vec()),
        parse_degrees(&text.passive, Field::PassiveDegrees)?
            .unwrap_or_else(|| inferred.passive().to_vec()),
    ))
}

fn parse_degrees(text: &str, field: Field) -> Result<Option<Vec<u32>>, ParseError> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let location = Location { field, line: None };
    let mut degrees = Vec::new();
    for token in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        if !token.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ParseError::at(
                "Degrees must be nonnegative integers separated by commas or spaces.",
                location,
            ));
        }
        degrees.push(
            token
                .parse()
                .map_err(|_| ParseError::at("Degree is too large.", location))?,
        );
    }
    if degrees.is_empty() {
        return Err(ParseError::at(
            "Enter at least one degree, or leave the field blank to infer degrees.",
            location,
        ));
    }
    Ok(Some(degrees))
}

fn parse_mapping(
    text: &str,
    inputs: &mut Alphabet,
    outputs: &mut Alphabet,
) -> Result<Vec<LabelSet>, ParseError> {
    let mut entries = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let location = Location {
            field: Field::Mapping,
            line: Some(index + 1),
        };
        let (left, right) = split_mapping(line, location)?;
        let input = parse_labels(left.trim(), inputs, location)?;
        if input.len() != 1 {
            return Err(ParseError::at(
                "A mapping must name exactly one input label.",
                location,
            ));
        }
        let id = input.iter().next().expect("one input label");
        let outputs = parse_labels(right.trim(), outputs, location)?;
        if entries.insert(id, outputs).is_some() {
            return Err(ParseError::at(
                "This input label already has a mapping.",
                location,
            ));
        }
    }
    inputs
        .names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            entries
                .remove(&crate::labels::LabelId(index as u32))
                .ok_or_else(|| {
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

fn split_mapping(line: &str, location: Location) -> Result<(&str, &str), ParseError> {
    let mut in_label = false;
    let mut separator = None;
    // Parenthesized names may themselves contain arrows. Only an arrow outside
    // a name separates input from outputs; parse_labels checks name syntax.
    for (index, character) in line.char_indices() {
        match character {
            '(' => in_label = true,
            ')' => in_label = false,
            '-' if !in_label && line[index..].starts_with("->") => {
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
    let index = separator
        .ok_or_else(|| ParseError::at("Write a mapping as input -> outputs.", location))?;
    Ok((&line[..index], &line[index + 2..]))
}

#[cfg(test)]
mod tests;
