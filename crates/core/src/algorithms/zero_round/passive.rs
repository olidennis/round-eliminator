//! Passive emission families, exact checks, and SAT blocking clauses.

use std::collections::{BTreeMap, BTreeSet};

use crate::computation::{Cancellation, Cancelled};
use rustsat::types::Lit;

use super::PassiveMode;
use crate::algorithms::configurations::{accepts_with, contains_condensed_with};
use crate::model::LabelId;
use crate::model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part};

// The search attaches a SAT literal as provenance. Independent witness
// verification uses (), sharing the exact check without constructing literals.
#[derive(Clone, Copy)]
pub(super) struct Emission<T = Lit> {
    pub input: Option<LabelId>,
    pub output: LabelId,
    pub source: T,
}

/// We check if the found zero round algorithm is valid.
pub(super) fn forbidden_combination<T: Copy + Ord>(
    constraint: &Constraint,
    inputs: &[CondensedConfiguration<Option<LabelId>>],
    emissions: &[Emission<T>],
    mode: PassiveMode,
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    for input in inputs {
        cancellation.check()?;
        let Some(mut parts) = output_parts(input, emissions, cancellation)? else {
            continue; // No compatible active neighbors can realize this family.
        };
        let family = CondensedConfiguration::new(
            parts
                .iter()
                .map(|part| {
                    Part::new(
                        LabelSet::new(
                            part.choices
                                .iter()
                                .map(|emission| emission.output)
                                .collect(),
                        ),
                        part.counts.iter().sum(),
                    )
                    .expect("input multiplicity fits u32")
                })
                .collect(),
        )
        .expect("transformation preserves degree");
        let mut covered = false;
        for target in constraint.configurations_at_degree(family.degree()) {
            if contains_condensed_with(target, &family, cancellation)? {
                covered = true;
                break;
            }
        }
        if covered {
            continue;
        }
        if mode == PassiveMode::Maximized {
            // Keeping all these emissions keeps the same forbidden family.
            // The clause blocks that combination without choosing a concrete output.
            let mut sources = Vec::new();
            for emission in emissions {
                cancellation.check()?;
                if input
                    .parts()
                    .iter()
                    .any(|part| part.labels().contains(emission.input))
                {
                    sources.push(emission.source);
                }
            }
            sources.sort_unstable();
            sources.dedup();
            return Ok(Some(sources));
        }
        // If the constraint is maximized, this is skipped
        if let Some(sources) = forbidden_output(constraint, &mut parts, cancellation)? {
            return Ok(Some(sources));
        }
    }
    Ok(None)
}


/// This function is used to encode the passive constraint in the case of degree <=2
pub(super) fn visit_forbidden(
    constraint: &Constraint,
    input: &CondensedConfiguration<Option<LabelId>>,
    emissions: &[Emission],
    cancellation: &Cancellation,
    mut visitor: impl FnMut(Vec<Lit>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Eager clauses must cover every emission origin, even when several input
    // labels can emit the same output. Degree <= 2 keeps this traversal small.
    let mut choices = Vec::new();
    let mut equal_previous = Vec::new();
    for part in input.parts() {
        let compatible: Vec<_> = emissions
            .iter()
            .filter(|emission| part.labels().contains(emission.input))
            .collect();
        for occurrence in 0..part.multiplicity() {
            choices.push(compatible.clone());
            equal_previous.push(occurrence != 0);
        }
    }
    if choices.iter().any(Vec::is_empty) {
        // No compatible active neighbor can realize this passive configuration.
        return Ok(());
    }
    let mut indices = vec![0; choices.len()];
    loop {
        cancellation.check()?;
        let selected: Vec<_> = choices
            .iter()
            .zip(&indices)
            .map(|(choices, &index)| choices[index])
            .collect();
        let output: Vec<_> = selected.iter().map(|emission| emission.output).collect();
        if !accepts_with(
            constraint,
            &Configuration::from_occurrences(output).expect("rule degree fits u32"),
            cancellation,
        )? {
            let mut clause: Vec<_> = selected.iter().map(|emission| emission.source).collect();
            clause.sort_unstable();
            clause.dedup();
            visitor(clause)?;
        }
        let Some(position) = indices
            .iter()
            .enumerate()
            .rposition(|(position, &index)| index + 1 < choices[position].len())
        else {
            break;
        };
        indices[position] += 1;
        // Inputs arrive sorted. For repeated inputs, permutations of the
        // outputs describe the same pair multiset, so keep indices sorted
        // within each equal-input block instead of revisiting permutations.
        for next in position + 1..indices.len() {
            indices[next] = if equal_previous[next] {
                indices[next - 1]
            } else {
                0
            };
        }
    }
    Ok(())
}

/// One transformed input part. Outputs are unique and ordered; a representative
/// emission per output suffices to justify one counterexample for a fixed candidate.
struct OutputPart<'a, T> {
    choices: Vec<&'a Emission<T>>,
    counts: Vec<u32>,
}

/// One origin per output suffices for a fixed SAT candidate. Keep ALL origins
/// separately in visit_forbidden: eager clauses must cover every future candidate.
fn output_parts<'a, T: Copy + Ord>(
    input: &CondensedConfiguration<Option<LabelId>>,
    emissions: &'a [Emission<T>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<OutputPart<'a, T>>>, Cancelled> {
    let mut grouped = BTreeMap::new();
    for part in input.parts() {
        cancellation.check()?;
        let mut outputs = BTreeMap::new();
        for emission in emissions {
            cancellation.check()?;
            if part.labels().contains(emission.input) {
                outputs.entry(emission.output).or_insert(emission);
            }
        }
        if outputs.is_empty() {
            // No active emissions can fill this part, so it creates no obligation.
            return Ok(None);
        }
        let choices: Vec<_> = outputs.into_values().collect();
        let key: Vec<_> = choices
            .iter()
            .map(|emission| (emission.output, emission.source))
            .collect();
        grouped
            .entry(key)
            .and_modify(|(_, count): &mut (Vec<&Emission<T>>, u32)| *count += part.multiplicity())
            .or_insert((choices, part.multiplicity()));
    }
    // Merge identical temporary output parts, including their source choices.
    // This prevents `ab ab ...` from enumerating permutations of the same outputs.
    // Source equality preserves the justification of a returned blocking clause.
    let parts: Vec<_> = grouped
        .into_values()
        .map(|(choices, count)| {
            let mut counts = vec![0; choices.len()];
            counts[0] = count;
            OutputPart { choices, counts }
        })
        .collect();
    Ok(Some(parts))
}

/// Enumerate output count distributions lazily; retain the first bad multiset's
/// emission origins so the caller can add a sound SAT blocking clause.
fn forbidden_output<T: Copy + Ord>(
    constraint: &Constraint,
    parts: &mut [OutputPart<'_, T>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    // Different distributions between overlapping parts can yield the same
    // output multiset. Check it once; retain only outputs actually visited.
    let mut visited = BTreeSet::new();
    loop {
        cancellation.check()?;
        let output = Configuration::new(
            parts
                .iter()
                .flat_map(|part| {
                    part.choices
                        .iter()
                        .zip(&part.counts)
                        .map(|(emission, &count)| (emission.output, count))
                })
                .collect(),
        )
        .expect("degree preserved");
        if visited.insert(output.clone()) && !accepts_with(constraint, &output, cancellation)? {
            let mut clause: Vec<_> = parts
                .iter()
                .flat_map(|part| {
                    part.choices
                        .iter()
                        .zip(&part.counts)
                        .filter(|(_, count)| **count != 0)
                        .map(|(emission, _)| emission.source)
                })
                .collect();
            clause.sort_unstable();
            clause.dedup();
            return Ok(Some(clause));
        }
        let Some(position) = (0..parts.len())
            .rev()
            .find(|&index| advance_counts(&mut parts[index].counts))
        else {
            break;
        };
        for part in &mut parts[position + 1..] {
            let count = part.counts.iter().sum();
            part.counts.fill(0);
            part.counts[0] = count;
        }
    }
    Ok(None)
}

/// Advance to the next count distribution, preserving its total multiplicity.
fn advance_counts(counts: &mut [u32]) -> bool {
    let Some(position) = counts[..counts.len() - 1]
        .iter()
        .rposition(|&count| count != 0)
    else {
        return false;
    };
    counts[position] -= 1;
    let remainder = counts[position + 1..].iter().sum::<u32>() + 1;
    counts[position + 1..].fill(0);
    counts[position + 1] = remainder;
    true
}
