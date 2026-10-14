//! LLM: The shared row/repetition grammar, with separate readers for labels and pairs.

use super::{Field, Location, ParseError, check_cancellation, paired::LabelPair};
use crate::computation::Cancellation;
use crate::model::{CondensedConfiguration, Constraint, LabelId, LabelSet, Part};
use std::{collections::HashMap, ops::RangeInclusive};

/// LLM: The temporary name-to-ID interner used while parsing one alphabet. `names[id]` gives
/// LLM: the text, while the hash map reuses an existing ID whenever that name occurs again.
#[derive(Default)]
pub(super) struct Alphabet {
    pub names: Vec<String>,
    ids: HashMap<String, LabelId>,
}

impl Alphabet {
    /// LLM: Reuse a name's ID or append a new name with its table index, rejecting ID
    /// LLM: overflow. Used by ordinary and paired label parsing.
    fn intern(&mut self, name: &str, location: Location) -> Result<LabelId, ParseError> {
        // LLM: Reuse the existing name-table index; new names are allocated only on their
        // LLM: first occurrence.
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = LabelId(
            u32::try_from(self.names.len())
                .map_err(|_| ParseError::at("Too many distinct labels.", location))?,
        );
        self.names.push(name.to_owned());
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.ids.insert(name.to_owned(), id);
        Ok(id)
    }
}

/// LLM: Parse condensed rows and expand inclusive repetition ranges into exact multiplicities
/// LLM: with cancellation. Shared by ordinary labels and pair-label readers; blank text is no
/// LLM: rows, while `()` is one empty multiset.
pub(super) fn parse_constraint<L: Copy + Ord>(
    text: &str,
    field: Field,
    choices: &mut impl FnMut(&str, Location) -> Result<LabelSet<L>, ParseError>,
    cancellation: &Cancellation,
) -> Result<Constraint<L>, ParseError> {
    // LLM: Collect expanded exact-degree rows while retaining a location for each nonblank
    // LLM: editor line.
    let mut configurations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        check_cancellation(cancellation)?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // LLM: Prepare `location` for the following computation; it stores the location for
        // LLM: this stage.
        let location = Location {
            field,
            line: Some(index + 1),
        };
        // LLM: Blank text has no configurations; () is one empty multiset (degree zero).

        let mut parts = Vec::new();
        let mut maximum_degree = 0u32;
        // LLM: Reject degree/count overflow rather than silently wrapping a model invariant.
        if line != "()" {
            // LLM: Reject degree/count overflow rather than silently wrapping a model
            // LLM: invariant.
            for token in line.split_whitespace() {
                // LLM: Reject starred notation before interpreting the supported exact or
                // LLM: ranged exponent.
                if token.contains('*') {
                    return Err(ParseError::at(
                        "Starred configurations are not supported.",
                        location,
                    ));
                }
                // LLM: Separate each part's label choices from its inclusive exponent range,
                // LLM: defaulting to one occurrence.
                let (token, counts) = match token.split_once('^') {
                    None => (token, 1..=1),
                    Some((token, exponent)) => (token, parse_repetition(exponent, location)?),
                };
                let labels = choices(token, location)?;
                if labels.is_empty() {
                    return Err(ParseError::at("A part needs at least one label.", location));
                }
                // LLM: Reject degree/count overflow rather than silently wrapping a model
                // LLM: invariant.
                maximum_degree = maximum_degree.checked_add(*counts.end()).ok_or_else(|| {
                    ParseError::at("Configuration degree is too large.", location)
                })?;
                parts.push((labels, counts));
            }
        }
        // LLM: Enumerate the Cartesian product with one counter per part. This needs
        // LLM: no product of range widths (which could overflow, especially on wasm).

        let mut counts: Vec<u32> = parts.iter().map(|(_, range)| *range.start()).collect();
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        loop {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            check_cancellation(cancellation)?;
            let mut expanded = Vec::with_capacity(parts.len());
            // LLM: Explore the next counted alternative while preserving the required total
            // LLM: capacities.
            for ((labels, _), &count) in parts.iter().zip(&counts) {
                // LLM: Zero occurrences contribute no part; stored parts stay positive.

                if count != 0 {
                    expanded.push(
                        Part::new(labels.clone(), count)
                            .map_err(|e| ParseError::at(e, location))?,
                    );
                }
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            configurations.push(
                CondensedConfiguration::new(expanded).map_err(|e| ParseError::at(e, location))?,
            );
            // LLM: Advance the first counter that has not reached its endpoint,
            // LLM: resetting earlier counters as we carry. An empty row runs once.

            let mut has_next = false;
            // LLM: Explore the next counted alternative while preserving the required total
            // LLM: capacities.
            for ((_, range), count) in parts.iter().zip(&mut counts) {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if *count < *range.end() {
                    *count += 1;
                    has_next = true;
                    break;
                }
                *count = *range.start();
            }
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if !has_next {
                break;
            }
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(configurations))
}

/// LLM: Read a nonnegative exponent or inclusive range and reject malformed, reversed, or
/// LLM: overflowing bounds. Used by the shared constraint grammar.
fn parse_repetition(text: &str, location: Location) -> Result<RangeInclusive<u32>, ParseError> {
    // LLM: Validate decimal nonnegative integer bounds before converting them to u32.
    let integer = |digits: &str| {
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ParseError::at(
                "Expected a nonnegative integer or an inclusive range such as '5..8' after '^'.",
                location,
            ));
        }
        // LLM: Update the current working state before continuing this operation.
        digits
            .parse::<u32>()
            .map_err(|_| ParseError::at("Repetition is too large.", location))
    };
    // LLM: Interpret a single exponent as a singleton interval and otherwise parse both
    // LLM: inclusive endpoints.
    let (start, end) = match text.split_once("..") {
        Some((start, end)) => (integer(start)?, integer(end)?),
        None => {
            let count = integer(text)?;
            (count, count)
        }
    };
    // LLM: Reject a reversed interval before expansion; equal endpoints are valid.
    if start > end {
        return Err(ParseError::at(
            "The start of an exponent range must not exceed its end.",
            location,
        ));
    }
    Ok(start..=end)
}

/// LLM: Read single-character and parenthesized label names into one canonical choice set.
/// LLM: Used by ordinary constraint parts and mapping entries.
pub(super) fn parse_labels(
    text: &str,
    alphabet: &mut Alphabet,
    location: Location,
) -> Result<LabelSet, ParseError> {
    // LLM: Scan single-character names and parenthesized long names into one shared alphabet.
    let mut chars = text.chars();
    let mut labels = Vec::new();
    // LLM: Process the relevant label choices while preserving their table identities.
    while let Some(character) = chars.next() {
        // LLM: Read a parenthesized name as one label rather than separate characters,
        // LLM: rejecting invalid or unclosed names.
        if character == '(' {
            // LLM: Prepare `name` for the following computation; it stores the name for this
            // LLM: stage.
            let mut name = String::new();
            let mut closed = false;
            // LLM: Process the relevant label choices while preserving their table identities.
            for next in chars.by_ref() {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if next == ')' {
                    closed = true;
                    break;
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if matches!(next, '(' | '^' | '*') || next.is_whitespace() {
                    return Err(ParseError::at(
                        "Invalid character inside a parenthesized label.",
                        location,
                    ));
                }
                name.push(next);
            }
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if !closed || name.is_empty() {
                return Err(ParseError::at(
                    "A parenthesized label must be nonempty and closed.",
                    location,
                ));
            }
            labels.push(alphabet.intern(&name, location)?);
        } else {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if matches!(character, ')' | '^' | '*') || character.is_whitespace() {
                return Err(ParseError::at(
                    "Unexpected character in label choices.",
                    location,
                ));
            }
            labels.push(alphabet.intern(&character.to_string(), location)?);
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(LabelSet::new(labels))
}

/// LLM: Read concatenated `(input,output)` choices with separate input/output interners. Used
/// LLM: only by paired text entry before canonical conversion.
pub(super) fn parse_pairs(
    mut text: &str,
    inputs: &mut Alphabet,
    outputs: &mut Alphabet,
    location: Location,
) -> Result<LabelSet<LabelPair>, ParseError> {
    // LLM: Queue the parent pairs for this bounded generation batch.
    let mut pairs = Vec::new();
    while !text.is_empty() {
        // LLM: Require a closed parenthesized pair and a separating comma before interning its
        // LLM: two names separately.
        let rest = text
            .strip_prefix('(')
            .ok_or_else(|| ParseError::at("Write pairs as (input,output).", location))?;
        let (pair, rest) = rest
            .split_once(')')
            .ok_or_else(|| ParseError::at("An input/output pair must be closed.", location))?;
        // LLM: Update the current working state before continuing this operation.
        let (input, output) = pair.split_once(',').ok_or_else(|| {
            ParseError::at(
                "A pair needs an input and an output separated by a comma.",
                location,
            )
        })?;
        // LLM: Process the relevant label choices while preserving their table identities.
        for name in [input, output] {
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if name.is_empty()
                || name
                    .chars()
                    .any(|c| matches!(c, '(' | ')' | ',' | '^' | '*') || c.is_whitespace())
            {
                return Err(ParseError::at(
                    "Pair labels must be nonempty and contain no whitespace, commas, parentheses, '^', or '*'.",
                    location,
                ));
            }
        }
        // LLM: Retain the newly constructed choices in the result for this stage.
        pairs.push(LabelPair {
            input: inputs.intern(input, location)?,
            output: outputs.intern(output, location)?,
        });
        text = rest;
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(LabelSet::new(pairs))
}
