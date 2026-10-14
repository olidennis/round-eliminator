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
use crate::{algorithms::maximize::MaximizationResult, model::InputOutputLabel};

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

// LLM: Limit eager *encoding work*, not the problem or palette. Larger cases keep
// LLM: exact lazy refinement. This conservative scheduling heuristic is not a
// LLM: benchmarked crossover; count possible origin tuples, not resulting clauses.

pub(crate) const EAGER_ENCODING_BUDGET: usize = 100_000;

/// LLM: Estimate possible emission-origin tuples for one low-degree passive input row across
/// LLM: its color placements. Used to choose eager clauses versus exact refinement; the
/// LLM: estimate is not a problem-size limit.
pub(crate) fn eager_encoding_work<I: Copy + Ord>(
    input: &CondensedConfiguration<I>,
    placements: usize,
    mut choices: impl FnMut(I) -> usize,
    cancellation: &Cancellation,
) -> Result<usize, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    if input.degree() > 2 {
        return Ok(usize::MAX);
    }
    if placements == 0 {
        return Ok(0); // LLM: No locally valid color placement, hence no obligation.
    }
    let mut work = placements;
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for part in input.parts() {
        // LLM: Prepare `alternatives` for the following computation; it stores the
        // LLM: alternatives for this stage.
        let mut alternatives = 0usize;
        for label in part.labels().iter() {
            cancellation.check()?;
            alternatives = alternatives.saturating_add(choices(label));
        }
        work = work.saturating_mul(alternatives.saturating_pow(part.multiplicity()));
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(work)
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
        let Some(parts) = output_parts(input, emissions, cancellation)? else {
            continue; // LLM: No compatible active neighbors can realize this family.
        };
        let family = output_family(&parts);
        let mut covered = false;
        // LLM: Try a single-row containment proof using counted subset-compatible matching.
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

        if let Some(sources) = forbidden_output(constraint, &parts, cancellation)? {
            return Ok(Some(sources));
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(None)
}

/// LLM: Check passive validity using a compatible cached maximum when available, otherwise the
/// LLM: original exact verifier. Used by solvability searches; source origins are retained for
/// LLM: a sound refinement clause.
pub(crate) fn forbidden_with_maximum<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    inputs: &[CondensedConfiguration<I>],
    emissions: &[Emission<T, I>],
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    original_input: impl Fn(I) -> Option<LabelId>,
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    match maximum {
        None => forbidden_combination(constraint, inputs, emissions, mode, cancellation),
        Some(MaximizationResult::OutputOnly(maximum)) => {
            forbidden_combination(maximum, inputs, emissions, mode, cancellation)
        }
        Some(MaximizationResult::InputOutput(maximum)) => {
            // LLM: Look for one rejected output multiset while preserving the source choices
            // LLM: that explain it.
            for input in inputs {
                // LLM: Prepare `parts` for the following computation; it stores the parts for
                // LLM: this stage.
                let mut parts = Vec::new();
                let mut possible = true;
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for part in input.parts() {
                    // LLM: Queue the parent pairs for this bounded generation batch.
                    let mut pairs = Vec::new();
                    for emission in emissions {
                        // LLM: Observe cooperative interruption before continuing this unit of
                        // LLM: work.
                        cancellation.check()?;
                        if part.labels().contains(emission.input) {
                            pairs.push(InputOutputLabel {
                                input: original_input(emission.input),
                                output: emission.output,
                            });
                        }
                    }
                    // LLM: Handle the empty case explicitly before the general traversal or
                    // LLM: allocation path.
                    if pairs.is_empty() {
                        possible = false;
                        break;
                    }
                    parts.push(
                        Part::new(LabelSet::new(pairs), part.multiplicity())
                            .expect("nonempty pairs"),
                    );
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if !possible {
                    continue;
                }
                let family = CondensedConfiguration::new(parts).expect("degree preserved");
                let mut covered = false;
                // LLM: Try a single-row containment proof using counted subset-compatible
                // LLM: matching.
                for target in maximum.configurations_at_degree(family.degree()) {
                    if contains_condensed_with(target, &family, cancellation)? {
                        covered = true;
                        break;
                    }
                }
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if covered {
                    continue;
                }
                // LLM: This family lies within the original input promise, so failed
                // LLM: joint containment guarantees a concrete invalid output. Find
                // LLM: its actual origins rather than blocking every enabled origin.

                if let Some(parts) = output_parts(input, emissions, cancellation)?
                    && let Some(sources) = forbidden_output(constraint, &parts, cancellation)?
                {
                    return Ok(Some(sources));
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(None)
        }
    }
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
    // LLM: labels can emit the same output. Callers estimate the work first when
    // LLM: degree <= 2 still has many possible origins.

    let mut choices = Vec::new();
    let mut equal_previous = Vec::new();
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for part in input.parts() {
        // LLM: Record the admissible pairings used to restrict exact matching or support
        // LLM: search.
        let mut compatible = Vec::new();
        for emission in emissions {
            cancellation.check()?;
            if part.labels().contains(emission.input) {
                compatible.push(emission);
            }
        }
        // LLM: Process each condensed part without expanding its repeated occurrences.
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
    // LLM: Group temporary parts only when output choices and their representative sources
    // LLM: coincide.
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
        // LLM: Include source identity in the grouping key so a returned counterexample
        // LLM: remains justified by its actual origins.
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
    parts: &[OutputPart<'_, T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    // LLM: Prepare `grouped` for the following computation; it stores the grouped for this
    // LLM: stage.
    let mut grouped = BTreeMap::<Vec<LabelId>, Vec<&OutputPart<'_, T, I>>>::new();
    for part in parts.iter() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        grouped
            .entry(
                part.choices
                    .iter()
                    .map(|emission| emission.output)
                    .collect(),
            )
            .or_default()
            .push(part);
    }
    // LLM: Group only the semantic choices. Provenance stays attached to every
    // LLM: original part; equal output domains may have distinct input/color origins.

    let groups: Vec<_> = grouped.into_values().collect();
    let family = output_family(parts);
    let mut forbidden = None;
    // LLM: Observe cooperative interruption before continuing this unit of work.
    visit_multisets_with(&family, cancellation, |output, counts| {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if accepts_with(constraint, output, cancellation)? {
            return Ok(true);
        }
        let mut sources = Vec::new();
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        for (origins, counts) in groups.iter().zip(counts) {
            // LLM: Distribute this semantic output count across its original parts so every
            // LLM: selected output retains a valid source.
            let mut remaining = counts.clone();
            // LLM: All these original parts have the same output domain. Allocate
            // LLM: this group's counts to their slots, preserving an actual source
            // LLM: for every output used in the counterexample. Blocking those
            // LLM: sources stays sound even if future candidates enable others.

            for origin in origins {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                let mut slots = origin.multiplicity;
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for (emission, count) in origin.choices.iter().zip(&mut remaining) {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    let used = slots.min(*count);
                    if used != 0 {
                        sources.push(emission.source);
                    }
                    slots -= used;
                    *count -= used;
                }
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                debug_assert_eq!(slots, 0);
            }
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            debug_assert!(remaining.iter().all(|count| *count == 0));
        }
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

/// LLM: Build the condensed family induced by available passive emissions. Used by passive
/// LLM: coverage checks; equal output domains can merge while counted provenance remains
/// LLM: recoverable.
fn output_family<T, I>(parts: &[OutputPart<'_, T, I>]) -> CondensedConfiguration {
    // LLM: Equal output sets are interchangeable for checking output validity.
    // LLM: Original input/color sources remain separate in OutputPart.

    let normalized = Configuration::new(
        parts
            .iter()
            .map(|part| {
                // LLM: Update the current working state before continuing this operation.
                (
                    LabelSet::new(
                        part.choices
                            .iter()
                            .map(|emission| emission.output)
                            .collect(),
                    ),
                    part.multiplicity,
                )
            })
            .collect(),
    )
    .expect("same total degree");
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    CondensedConfiguration::new(
        normalized
            .multiplicities()
            .iter()
            .map(|(labels, count)| Part::new(labels.clone(), *count).expect("nonempty choices"))
            .collect(),
    )
    .expect("same degree")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LLM: Regression test: verify that eager work estimate handles condensed inputs overflow
    /// LLM: and no placements. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn eager_work_estimate_handles_condensed_inputs_overflow_and_no_placements() {
        // LLM: Retain or prepare the original promised input semantics for this stage.
        let input =
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![0u8, 1]), 2).unwrap()])
                .unwrap();
        let cancellation = Cancellation::default();
        assert_eq!(
            eager_encoding_work(&input, 6, |i| usize::from(i) + 2, &cancellation).unwrap(),
            6 * 25
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert_eq!(
            eager_encoding_work(&input, 6, |_| usize::MAX, &cancellation).unwrap(),
            usize::MAX
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert_eq!(
            eager_encoding_work(&input, 0, |_| panic!("no color placement"), &cancellation)
                .unwrap(),
            0
        );
        let high =
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![0u8]), 3).unwrap()])
                .unwrap();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert_eq!(
            eager_encoding_work(&high, 1, |_| 1, &cancellation).unwrap(),
            usize::MAX
        );
        cancellation.cancel();
        assert!(eager_encoding_work(&input, 0, |_| 0, &cancellation).is_err());
    }

    /// LLM: Regression test: verify that cached joint counterexamples preserve colored input
    /// LLM: origins for all future supersets. Used by the test suite to protect this behavior
    /// LLM: while implementations change.
    #[test]
    fn cached_joint_counterexamples_preserve_colored_input_origins_for_all_future_supersets() {
        // LLM: Prepare `problem` for the following computation; it stores the problem for this
        // LLM: stage.
        let problem = crate::parse_problem(serde_json::from_value(serde_json::json!({
            "kind":"mapped", "input":{"active":"ab^3","passive":"ab^3"},
            "output":{"active":"AB^3","passive":"A^3\nA^2 B\nB^3"}, "mapping":"a -> AB\nb -> AB",
        })).unwrap()).unwrap();
        let maximum = crate::algorithms::maximize::compute(&problem).unwrap();
        // LLM: Retain or prepare the original promised input semantics for this stage.
        let input = CondensedConfiguration::new(vec![
            Part::new(
                LabelSet::new(vec![(Some(LabelId(0)), 0u32), (Some(LabelId(1)), 0)]),
                1,
            )
            .unwrap(),
            Part::new(
                LabelSet::new(vec![(Some(LabelId(0)), 1u32), (Some(LabelId(1)), 1)]),
                2,
            )
            .unwrap(),
        ])
        .unwrap();
        // LLM: Prepare `safe` for the following computation; it stores the safe for this
        // LLM: stage.
        let safe = |mask: u16| {
            // LLM: Keep repetitions as count distributions instead of expanding permutations
            // LLM: of occurrences.
            let mut counts = vec![0];
            for (color, slots) in [(0, 1), (1, 2)] {
                // LLM: Prepare `a` for the following computation; it stores the a for this
                // LLM: stage.
                let a = mask & (0b0101 << (4 * color)) != 0;
                let b = mask & (0b1010 << (4 * color)) != 0;
                if !a && !b {
                    return true;
                }
                // LLM: Update the current working state before continuing this operation.
                counts = counts
                    .into_iter()
                    .flat_map(|old| {
                        (0..=slots)
                            .filter(move |count| (*count == 0 || a) && (*count == slots || b))
                            .map(move |count| old + count)
                    })
                    .collect();
            }
            // LLM: Update the current working state before continuing this operation.
            !counts.contains(&1)
        };
        // LLM: Verify the candidate's universal passive outputs, reusing only a compatible
        // LLM: maximum certificate.
        for mask in 0u16..256 {
            // LLM: Collect available outputs with their input/color descriptors and source
            // LLM: provenance.
            let emissions: Vec<_> = (0u8..8)
                .filter(|source| mask & (1 << source) != 0)
                .map(|source| Emission {
                    input: (
                        Some(LabelId(u32::from((source % 4) / 2))),
                        u32::from(source / 4),
                    ),
                    output: LabelId(u32::from(source % 2)),
                    source,
                })
                .collect();
            // LLM: Verify the candidate's universal passive outputs, reusing only a compatible
            // LLM: maximum certificate.
            let forbidden = forbidden_with_maximum(
                problem.output().passive(),
                std::slice::from_ref(&input),
                &emissions,
                PassiveMode::General,
                Some(&maximum),
                |input| input.0,
                &Cancellation::default(),
            )
            .unwrap();
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(forbidden.is_none(), safe(mask));
            if let Some(sources) = forbidden {
                // LLM: Prepare `required` for the following computation; it stores the
                // LLM: required for this stage.
                let required = sources
                    .into_iter()
                    .fold(0u16, |bits, source| bits | (1 << source));
                assert_eq!(required & mask, required);
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for future in 0u16..256 {
                    if future & required == required {
                        assert!(!safe(future));
                    }
                }
            }
        }
    }

    // LLM: Independent oracle: allowed iff the number of A outputs is not three.
    /// LLM: Check a small candidate against the independent fixture relation. Used to validate
    /// LLM: counterexample soundness in the passive tests.
    fn safe(mask: u8) -> bool {
        // LLM: Keep repetitions as count distributions instead of expanding permutations of
        // LLM: occurrences.
        let counts = [1, 2, 4];
        let mut totals = vec![0];
        // LLM: Explore the next counted alternative while preserving the required total
        // LLM: capacities.
        for (input, count) in counts.into_iter().enumerate() {
            // LLM: Prepare `a` for the following computation; it stores the a for this stage.
            let a = mask & (1 << (2 * input)) != 0;
            let b = mask & (1 << (2 * input + 1)) != 0;
            if !a && !b {
                return true;
            } // LLM: this emission family cannot occur
            totals = totals
                .into_iter()
                .flat_map(|total| {
                    (0..=count)
                        .filter(move |n| (*n == 0 || a) && (*n == count || b))
                        .map(move |n| total + n)
                })
                .collect();
        }
        // LLM: Update the current working state before continuing this operation.
        !totals.contains(&3)
    }

    /// LLM: Regression test: verify that every grouped counterexample clause preserves input
    /// LLM: origins. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn every_grouped_counterexample_clause_preserves_input_origins() {
        // LLM: Prepare `target` for the following computation; it stores the target for this
        // LLM: stage.
        let target = Constraint::new(
            (0..=7)
                .filter(|count| *count != 3)
                .map(|count| {
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![LabelId(0)]), count).unwrap(),
                        Part::new(LabelSet::new(vec![LabelId(1)]), 7 - count).unwrap(),
                    ])
                    .unwrap()
                })
                .collect(),
        );
        // LLM: Retain or prepare the original promised input semantics for this stage.
        let input = CondensedConfiguration::new(
            [1, 2, 4]
                .into_iter()
                .enumerate()
                .map(|(input, count)| Part::new(LabelSet::new(vec![input as u8]), count).unwrap())
                .collect(),
        )
        .unwrap();
        // LLM: Find the emission origins of a passive violation so refinement blocks that
        // LLM: simultaneous choice.
        for mask in 0u8..64 {
            // LLM: Collect available outputs with their input/color descriptors and source
            // LLM: provenance.
            let emissions: Vec<_> = (0u8..6)
                .filter(|source| mask & (1 << source) != 0)
                .map(|source| Emission {
                    input: source / 2,
                    output: LabelId(u32::from(source % 2)),
                    source,
                })
                .collect();
            // LLM: Find the emission origins of a passive violation so refinement blocks that
            // LLM: simultaneous choice.
            let forbidden = forbidden_combination(
                &target,
                std::slice::from_ref(&input),
                &emissions,
                PassiveMode::General,
                &Cancellation::default(),
            )
            .unwrap();
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(forbidden.is_none(), safe(mask), "mask {mask}");
            if let Some(sources) = forbidden {
                // LLM: Check the expected result and invariants for this fixture, including
                // LLM: the relevant edge case.
                assert!(!sources.is_empty());
                let required = sources
                    .into_iter()
                    .fold(0, |bits, source| bits | (1 << source));
                assert_eq!(mask & required, required);
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for future in 0u8..64 {
                    if future & required == required {
                        assert!(!safe(future), "clause admitted a valid future assignment");
                    }
                }
            }
        }
    }
}
