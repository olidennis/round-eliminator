//! LLM: Passive emission families, exact checks, and SAT blocking clauses.

use std::collections::BTreeMap;

use crate::computation::{Cancellation, Cancelled};
use rustsat::types::Lit;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// LLM: The guarantee used to check passive output families. General mode verifies the whole
/// LLM: constraint exactly; maximized mode may reject on failed single-row containment only
/// LLM: when maximality is guaranteed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PassiveMode {
    /// LLM: Exact for arbitrary constraints, including unions of condensed configurations.

    #[default]
    General,
    /// LLM: Requires every maximal passive configuration to be listed, up to permutation.
    /// LLM: The caller guarantees this; choosing this mode does not perform maximization.

    Maximized,
}

use crate::algorithms::configurations::{
    accepts_with, contains_condensed_with, visit_multisets_with,
};
use crate::model::LabelId;
use crate::model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part};

// LLM: The search attaches a SAT literal as provenance. Independent witness
// LLM: verification uses (), sharing the exact check without constructing literals.
/// LLM: A possible output paired with its input descriptor and source identifier. Sources
/// LLM: preserve provenance for counterexamples, so a SAT caller can exclude simultaneous
/// LLM: enabling of the emissions causing a violation.
#[derive(Clone, Copy)]
pub(crate) struct Emission<T = Lit, I = Option<LabelId>> {
    pub input: I,
    pub output: LabelId,
    pub source: T,
}

/// LLM: Find source emissions whose simultaneous availability permits a forbidden passive
/// LLM: output. Used by SAT refinement and independent verification; no violation returns no
/// LLM: blocking sources.
pub(crate) fn forbidden_combination<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    inputs: &[CondensedConfiguration<I>],
    emissions: &[Emission<T, I>],
    mode: PassiveMode,
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    // LLM: Look for one rejected output multiset while preserving the source choices that
    // LLM: explain it.
    for input in inputs {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let Some(mut parts) = output_parts(input, emissions, cancellation)? else {
            continue;  // LLM: No compatible active neighbors can realize this family.
        };
        // LLM: Prepare `family` for the following computation; it stores the family for this
        // LLM: stage.
        let family = CondensedConfiguration::new(
            parts
                .iter()
                .map(|part| {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    Part::new(
                        LabelSet::new(
                            part.choices
                                .iter()
                                .map(|emission| emission.output)
                                .collect(),
                        ),
                        part.multiplicity,
                    )
                    .expect("input multiplicity fits u32")
                })
                .collect(),
        )
        .expect("transformation preserves degree");
        // LLM: Prepare `covered` for the following computation; it stores the covered for this
        // LLM: stage.
        let mut covered = false;
        for target in constraint.configurations_at_degree(family.degree()) {
            if contains_condensed_with(target, &family, cancellation)? {
                covered = true;
                break;
            }
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if covered {
            continue;
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if mode == PassiveMode::Maximized {
            // LLM: Keeping all these emissions keeps the same forbidden family.
            // LLM: The clause blocks that combination without choosing a concrete output.

            let mut sources = Vec::new();
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for emission in emissions {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                if input
                    .parts()
                    .iter()
                    .any(|part| part.labels().contains(emission.input))
                {
                    sources.push(emission.source);
                }
            }
            // LLM: Restore deterministic canonical ordering before the value is stored or
            // LLM: returned.
            sources.sort_unstable();
            sources.dedup();
            return Ok(Some(sources));
        }
        // LLM: If the constraint is maximized, this is skipped

        if let Some(sources) = forbidden_output(constraint, &mut parts, cancellation)? {
            return Ok(Some(sources));
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(None)
}

/// LLM: Visit forbidden low-degree emission-origin combinations to encode passive clauses
/// LLM: eagerly. Used by zero-round and coloring SAT, retaining all origins needed for future
/// LLM: candidate assignments.
pub(crate) fn visit_forbidden<I: Copy + Ord>(
    constraint: &Constraint,
    input: &CondensedConfiguration<I>,
    emissions: &[Emission<Lit, I>],
    cancellation: &Cancellation,
    mut visitor: impl FnMut(Vec<Lit>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // LLM: Eager clauses must cover every emission origin, even when several input
    // LLM: labels can emit the same output. Degree <= 2 keeps this traversal small.

    let mut choices = Vec::new();
    let mut equal_previous = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for part in input.parts() {
        // LLM: Record the admissible pairings used to restrict exact matching or support
        // LLM: search.
        let compatible: Vec<_> = emissions
            .iter()
            .filter(|emission| part.labels().contains(emission.input))
            .collect();
        for occurrence in 0..part.multiplicity() {
            choices.push(compatible.clone());
            equal_previous.push(occurrence != 0);
        }
    }
    // LLM: Handle the empty case explicitly before the general traversal or allocation path.
    if choices.iter().any(Vec::is_empty) {
        // LLM: No compatible active neighbor can realize this passive configuration.

        return Ok(());
    }
    let mut indices = vec![0; choices.len()];
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    loop {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let selected: Vec<_> = choices
            .iter()
            .zip(&indices)
            .map(|(choices, &index)| choices[index])
            .collect();
        let output: Vec<_> = selected.iter().map(|emission| emission.output).collect();
        // LLM: Handle this condition before continuing with the remaining search alternatives.
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
        // LLM: Stop this alternative when the required choice is absent, leaving the enclosing
        // LLM: traversal to backtrack or continue.
        let Some(position) = indices
            .iter()
            .enumerate()
            .rposition(|(position, &index)| index + 1 < choices[position].len())
        else {
            break;
        };
        indices[position] += 1;
        // LLM: Inputs arrive sorted. For repeated inputs, permutations of the
        // LLM: outputs describe the same pair multiset, so keep indices sorted
        // LLM: within each equal-input block instead of revisiting permutations.

        for next in position + 1..indices.len() {
            indices[next] = if equal_previous[next] {
                indices[next - 1]
            } else {
                0
            };
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(())
}

/// LLM: One substituted passive input part with counted output choices and representative
/// LLM: source provenance. Equal semantic domains can merge during verification while their
/// LLM: original origins remain available for a blocking clause.
struct OutputPart<'a, T, I> {
    choices: Vec<&'a Emission<T, I>>,
    multiplicity: u32,
}

/// LLM: Substitute available emissions into condensed passive input parts and retain
/// LLM: representative sources for each output. Used to verify one candidate, rather than
/// LLM: encode all future assignments.
fn output_parts<'a, T: Copy + Ord, I: Copy + Ord>(
    input: &CondensedConfiguration<I>,
    emissions: &'a [Emission<T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<OutputPart<'a, T, I>>>, Cancelled> {
    // LLM: Prepare `grouped` for the following computation; it stores the grouped for this
    // LLM: stage.
    let mut grouped = BTreeMap::new();
    for part in input.parts() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let mut outputs = BTreeMap::new();
        for emission in emissions {
            cancellation.check()?;
            if part.labels().contains(emission.input) {
                outputs.entry(emission.output).or_insert(emission);
            }
        }
        // LLM: Handle the empty case explicitly before the general traversal or allocation
        // LLM: path.
        if outputs.is_empty() {
            // LLM: No active emissions can fill this part, so it creates no obligation.

            return Ok(None);
        }
        let choices: Vec<_> = outputs.into_values().collect();
        // LLM: Encode the complete counted row for exact history lookup.
        let key: Vec<_> = choices
            .iter()
            .map(|emission| (emission.output, emission.source))
            .collect();
        // LLM: Update the current working state before continuing this operation.
        grouped
            .entry(key)
            .and_modify(|(_, count): &mut (Vec<&Emission<T, I>>, u32)| {
                *count += part.multiplicity()
            })
            .or_insert((choices, part.multiplicity()));
    }
    // LLM: Merge identical temporary output parts, including their source choices.
    // LLM: This prevents `ab ab ...` from enumerating permutations of the same outputs.
    // LLM: Source equality preserves the justification of a returned blocking clause.

    let parts: Vec<_> = grouped
        .into_values()
        .map(|(choices, count)| OutputPart {
            choices,
            multiplicity: count,
        })
        .collect();
    Ok(Some(parts))
}

/// LLM: Search counted output distributions lazily for a forbidden multiset and recover its
/// LLM: emission origins. Used by passive strategy refinement after single-row containment
/// LLM: cannot establish validity.
fn forbidden_output<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    parts: &mut [OutputPart<'_, T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    // LLM: Prepare `family` for the following computation; it stores the family for this
    // LLM: stage.
    let family = CondensedConfiguration::new(
        parts
            .iter()
            .map(|part| {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                Part::new(
                    LabelSet::new(
                        part.choices
                            .iter()
                            .map(|emission| emission.output)
                            .collect(),
                    ),
                    part.multiplicity,
                )
                .expect("nonempty choices")
            })
            .collect(),
    )
    .expect("degree preserved");
    // LLM: Preserve the violation or conflict needed to reject an invalid search branch.
    let mut forbidden = None;
    visit_multisets_with(&family, cancellation, |output, counts| {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if accepts_with(constraint, output, cancellation)? {
            return Ok(true);
        }
        // LLM: Prepare `sources` for the following computation; it stores the sources for this
        // LLM: stage.
        let mut sources: Vec<_> = parts
            .iter()
            .zip(counts)
            .flat_map(|(part, counts)| {
                part.choices
                    .iter()
                    .zip(counts)
                    .filter(|(_, count)| **count != 0)
                    .map(|(emission, _)| emission.source)
            })
            .collect();
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        sources.sort_unstable();
        sources.dedup();
        forbidden = Some(sources);
        Ok(false)
    })?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(forbidden)
}
