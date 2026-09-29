//! The shared row/repetition grammar, with separate readers for labels and pairs.

use super::{Field, Location, ParseError};
use crate::{
    labels::{LabelId, LabelSet},
    problem::{CondensedConfiguration, Constraint, LabelPair, Part},
};
use std::{collections::HashMap, ops::RangeInclusive};

#[derive(Default)]
pub(super) struct Alphabet {
    pub names: Vec<String>,
    ids: HashMap<String, LabelId>,
}

impl Alphabet {
    fn intern(&mut self, name: &str, location: Location) -> Result<LabelId, ParseError> {
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = LabelId(
            u32::try_from(self.names.len())
                .map_err(|_| ParseError::at("Too many distinct labels.", location))?,
        );
        self.names.push(name.to_owned());
        self.ids.insert(name.to_owned(), id);
        Ok(id)
    }
}

pub(super) fn parse_constraint<L: Copy + Ord>(
    text: &str,
    field: Field,
    choices: &mut impl FnMut(&str, Location) -> Result<LabelSet<L>, ParseError>,
) -> Result<Constraint<L>, ParseError> {
    let mut configurations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let location = Location {
            field,
            line: Some(index + 1),
        };
        // Blank text has no configurations; () is one empty multiset (degree zero).
        let mut parts = Vec::new();
        let mut maximum_degree = 0u32;
        if line != "()" {
            for token in line.split_whitespace() {
                if token.contains('*') {
                    return Err(ParseError::at(
                        "Starred configurations are not supported.",
                        location,
                    ));
                }
                let (token, counts) = match token.split_once('^') {
                    None => (token, 1..=1),
                    Some((token, exponent)) => (token, parse_repetition(exponent, location)?),
                };
                let labels = choices(token, location)?;
                if labels.is_empty() {
                    return Err(ParseError::at("A part needs at least one label.", location));
                }
                maximum_degree = maximum_degree.checked_add(*counts.end()).ok_or_else(|| {
                    ParseError::at("Configuration degree is too large.", location)
                })?;
                parts.push((labels, counts));
            }
        }
        // Enumerate the Cartesian product with one counter per part. This needs
        // no product of range widths (which could overflow, especially on wasm).
        let mut counts: Vec<u32> = parts.iter().map(|(_, range)| *range.start()).collect();
        loop {
            let mut expanded = Vec::with_capacity(parts.len());
            for ((labels, _), &count) in parts.iter().zip(&counts) {
                // Zero occurrences contribute no part; stored parts stay positive.
                if count != 0 {
                    expanded.push(
                        Part::new(labels.clone(), count)
                            .map_err(|e| ParseError::at(e, location))?,
                    );
                }
            }
            configurations.push(
                CondensedConfiguration::new(expanded).map_err(|e| ParseError::at(e, location))?,
            );
            // Advance the first counter that has not reached its endpoint,
            // resetting earlier counters as we carry. An empty row runs once.
            let mut has_next = false;
            for ((_, range), count) in parts.iter().zip(&mut counts) {
                if *count < *range.end() {
                    *count += 1;
                    has_next = true;
                    break;
                }
                *count = *range.start();
            }
            if !has_next {
                break;
            }
        }
    }
    Ok(Constraint::new(configurations))
}

fn parse_repetition(text: &str, location: Location) -> Result<RangeInclusive<u32>, ParseError> {
    let integer = |digits: &str| {
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ParseError::at(
                "Expected a nonnegative integer or an inclusive range such as '5..8' after '^'.",
                location,
            ));
        }
        digits
            .parse::<u32>()
            .map_err(|_| ParseError::at("Repetition is too large.", location))
    };
    let (start, end) = match text.split_once("..") {
        Some((start, end)) => (integer(start)?, integer(end)?),
        None => {
            let count = integer(text)?;
            (count, count)
        }
    };
    if start > end {
        return Err(ParseError::at(
            "The start of an exponent range must not exceed its end.",
            location,
        ));
    }
    Ok(start..=end)
}

pub(super) fn parse_labels(
    text: &str,
    alphabet: &mut Alphabet,
    location: Location,
) -> Result<LabelSet, ParseError> {
    let mut chars = text.chars();
    let mut labels = Vec::new();
    while let Some(character) = chars.next() {
        if character == '(' {
            let mut name = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == ')' {
                    closed = true;
                    break;
                }
                if matches!(next, '(' | '^' | '*') || next.is_whitespace() {
                    return Err(ParseError::at(
                        "Invalid character inside a parenthesized label.",
                        location,
                    ));
                }
                name.push(next);
            }
            if !closed || name.is_empty() {
                return Err(ParseError::at(
                    "A parenthesized label must be nonempty and closed.",
                    location,
                ));
            }
            labels.push(alphabet.intern(&name, location)?);
        } else {
            if matches!(character, ')' | '^' | '*') || character.is_whitespace() {
                return Err(ParseError::at(
                    "Unexpected character in label choices.",
                    location,
                ));
            }
            labels.push(alphabet.intern(&character.to_string(), location)?);
        }
    }
    Ok(LabelSet::new(labels))
}

pub(super) fn parse_pairs(
    mut text: &str,
    inputs: &mut Alphabet,
    outputs: &mut Alphabet,
    location: Location,
) -> Result<LabelSet<LabelPair>, ParseError> {
    let mut pairs = Vec::new();
    while !text.is_empty() {
        let rest = text
            .strip_prefix('(')
            .ok_or_else(|| ParseError::at("Write pairs as (input,output).", location))?;
        let (pair, rest) = rest
            .split_once(')')
            .ok_or_else(|| ParseError::at("An input/output pair must be closed.", location))?;
        let (input, output) = pair.split_once(',').ok_or_else(|| {
            ParseError::at(
                "A pair needs an input and an output separated by a comma.",
                location,
            )
        })?;
        for name in [input, output] {
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
        pairs.push(LabelPair {
            input: inputs.intern(input, location)?,
            output: outputs.intern(output, location)?,
        });
        text = rest;
    }
    Ok(LabelSet::new(pairs))
}
