//! Passive emission families, exact checks, and SAT blocking clauses.

use std::collections::BTreeMap;

use crate::computation::{Cancellation, Cancelled};
use rustsat::types::Lit;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How to verify passive output families for a candidate strategy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PassiveMode {
    /// Exact for arbitrary constraints, including unions of condensed configurations.
    #[default]
    General,
    /// Requires every maximal passive configuration to be listed, up to permutation.
    /// The caller guarantees this; choosing this mode does not perform maximization.
    Maximized,
}

use crate::algorithms::configurations::{
    accepts_with, contains_condensed_with, visit_multisets_with,
};
use crate::model::LabelId;
use crate::model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part};
use crate::{algorithms::maximize::MaximizationResult, model::InputOutputLabel};

// The search attaches a SAT literal as provenance. Independent witness
// verification uses (), sharing the exact check without constructing literals.
#[derive(Clone, Copy)]
pub(crate) struct Emission<T = Lit, I = Option<LabelId>> {
    pub input: I,
    pub output: LabelId,
    pub source: T,
}

// Limit eager *encoding work*, not the problem or palette. Larger cases keep
// exact lazy refinement. This conservative scheduling heuristic is not a
// benchmarked crossover; count possible origin tuples, not resulting clauses.
pub(crate) const EAGER_ENCODING_BUDGET: usize = 100_000;

/// Upper bound on origin tuples across all color placements of this input row.
/// Repeated parts can avoid permutations, so actual encoding may do less work.
pub(crate) fn eager_encoding_work<I: Copy + Ord>(
    input: &CondensedConfiguration<I>,
    placements: usize,
    mut choices: impl FnMut(I) -> usize,
    cancellation: &Cancellation,
) -> Result<usize, Cancelled> {
    cancellation.check()?;
    if input.degree() > 2 {
        return Ok(usize::MAX);
    }
    if placements == 0 {
        return Ok(0); // No locally valid color placement, hence no obligation.
    }
    let mut work = placements;
    for part in input.parts() {
        let mut alternatives = 0usize;
        for label in part.labels().iter() {
            cancellation.check()?;
            alternatives = alternatives.saturating_add(choices(label));
        }
        work = work.saturating_mul(alternatives.saturating_pow(part.multiplicity()));
    }
    Ok(work)
}

/// Return emission origins whose simultaneous availability permits a forbidden
/// passive output. Keeping all these origins enabled would preserve the violation.
pub(crate) fn forbidden_combination<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    inputs: &[CondensedConfiguration<I>],
    emissions: &[Emission<T, I>],
    mode: PassiveMode,
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    for input in inputs {
        cancellation.check()?;
        let Some(parts) = output_parts(input, emissions, cancellation)? else {
            continue; // No compatible active neighbors can realize this family.
        };
        let family = output_family(&parts);
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
        if let Some(sources) = forbidden_output(constraint, &parts, cancellation)? {
            return Ok(Some(sources));
        }
    }
    Ok(None)
}

/// Use a cached maximum as a containment certificate. General search retains
/// concrete counterexample clauses: blocking an entire failed family can cause
/// substantially more SAT work. Colors are projected away only for the proof;
/// original emission sources still justify every learned counterexample.
pub(crate) fn forbidden_with_maximum<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    inputs: &[CondensedConfiguration<I>],
    emissions: &[Emission<T, I>],
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    original_input: impl Fn(I) -> Option<LabelId>,
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    match maximum {
        None => forbidden_combination(constraint, inputs, emissions, mode, cancellation),
        Some(MaximizationResult::OutputOnly(maximum)) => {
            forbidden_combination(maximum, inputs, emissions, mode, cancellation)
        }
        Some(MaximizationResult::InputOutput(maximum)) => {
            for input in inputs {
                let mut parts = Vec::new();
                let mut possible = true;
                for part in input.parts() {
                    let mut pairs = Vec::new();
                    for emission in emissions {
                        cancellation.check()?;
                        if part.labels().contains(emission.input) {
                            pairs.push(InputOutputLabel {
                                input: original_input(emission.input),
                                output: emission.output,
                            });
                        }
                    }
                    if pairs.is_empty() {
                        possible = false;
                        break;
                    }
                    parts.push(
                        Part::new(LabelSet::new(pairs), part.multiplicity())
                            .expect("nonempty pairs"),
                    );
                }
                if !possible {
                    continue;
                }
                let family = CondensedConfiguration::new(parts).expect("degree preserved");
                let mut covered = false;
                for target in maximum.configurations_at_degree(family.degree()) {
                    if contains_condensed_with(target, &family, cancellation)? {
                        covered = true;
                        break;
                    }
                }
                if covered {
                    continue;
                }
                // This family lies within the original input promise, so failed
                // joint containment guarantees a concrete invalid output. Find
                // its actual origins rather than blocking every enabled origin.
                if let Some(parts) = output_parts(input, emissions, cancellation)?
                    && let Some(sources) = forbidden_output(constraint, &parts, cancellation)?
                {
                    return Ok(Some(sources));
                }
            }
            Ok(None)
        }
    }
}

/// This function is used to encode the passive constraint in the case of degree <=2
pub(crate) fn visit_forbidden<I: Copy + Ord>(
    constraint: &Constraint,
    input: &CondensedConfiguration<I>,
    emissions: &[Emission<Lit, I>],
    cancellation: &Cancellation,
    mut visitor: impl FnMut(Vec<Lit>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Eager clauses must cover every emission origin, even when several input
    // labels can emit the same output. Callers estimate the work first when
    // degree <= 2 still has many possible origins.
    let mut choices = Vec::new();
    let mut equal_previous = Vec::new();
    for part in input.parts() {
        let mut compatible = Vec::new();
        for emission in emissions {
            cancellation.check()?;
            if part.labels().contains(emission.input) {
                compatible.push(emission);
            }
        }
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
struct OutputPart<'a, T, I> {
    choices: Vec<&'a Emission<T, I>>,
    multiplicity: u32,
}

/// One origin per output suffices for a fixed SAT candidate. Keep ALL origins
/// separately in visit_forbidden: eager clauses must cover every future candidate.
fn output_parts<'a, T: Copy + Ord, I: Copy + Ord>(
    input: &CondensedConfiguration<I>,
    emissions: &'a [Emission<T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<OutputPart<'a, T, I>>>, Cancelled> {
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
            .and_modify(|(_, count): &mut (Vec<&Emission<T, I>>, u32)| {
                *count += part.multiplicity()
            })
            .or_insert((choices, part.multiplicity()));
    }
    // Merge identical temporary output parts, including their source choices.
    // This prevents `ab ab ...` from enumerating permutations of the same outputs.
    // Source equality preserves the justification of a returned blocking clause.
    let parts: Vec<_> = grouped
        .into_values()
        .map(|(choices, count)| OutputPart {
            choices,
            multiplicity: count,
        })
        .collect();
    Ok(Some(parts))
}

/// Enumerate output count distributions lazily; retain the first bad multiset's
/// emission origins so the caller can add a sound SAT blocking clause.
fn forbidden_output<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    parts: &[OutputPart<'_, T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    let mut grouped = BTreeMap::<Vec<LabelId>, Vec<&OutputPart<'_, T, I>>>::new();
    for part in parts.iter() {
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
    // Group only the semantic choices. Provenance stays attached to every
    // original part; equal output domains may have distinct input/color origins.
    let groups: Vec<_> = grouped.into_values().collect();
    let family = output_family(parts);
    let mut forbidden = None;
    visit_multisets_with(&family, cancellation, |output, counts| {
        if accepts_with(constraint, output, cancellation)? {
            return Ok(true);
        }
        let mut sources = Vec::new();
        for (origins, counts) in groups.iter().zip(counts) {
            let mut remaining = counts.clone();
            // All these original parts have the same output domain. Allocate
            // this group's counts to their slots, preserving an actual source
            // for every output used in the counterexample. Blocking those
            // sources stays sound even if future candidates enable others.
            for origin in origins {
                cancellation.check()?;
                let mut slots = origin.multiplicity;
                for (emission, count) in origin.choices.iter().zip(&mut remaining) {
                    cancellation.check()?;
                    let used = slots.min(*count);
                    if used != 0 {
                        sources.push(emission.source);
                    }
                    slots -= used;
                    *count -= used;
                }
                debug_assert_eq!(slots, 0);
            }
            debug_assert!(remaining.iter().all(|count| *count == 0));
        }
        sources.sort_unstable();
        sources.dedup();
        forbidden = Some(sources);
        Ok(false)
    })?;
    Ok(forbidden)
}

fn output_family<T, I>(parts: &[OutputPart<'_, T, I>]) -> CondensedConfiguration {
    // Equal output sets are interchangeable for checking output validity.
    // Original input/color sources remain separate in OutputPart.
    let normalized = Configuration::new(
        parts
            .iter()
            .map(|part| {
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

    #[test]
    fn eager_work_estimate_handles_condensed_inputs_overflow_and_no_placements() {
        let input =
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![0u8, 1]), 2).unwrap()])
                .unwrap();
        let cancellation = Cancellation::default();
        assert_eq!(
            eager_encoding_work(&input, 6, |i| usize::from(i) + 2, &cancellation).unwrap(),
            6 * 25
        );
        assert_eq!(
            eager_encoding_work(&input, 6, |_| usize::MAX, &cancellation).unwrap(),
            usize::MAX
        );
        assert_eq!(
            eager_encoding_work(&input, 0, |_| panic!("no color placement"), &cancellation)
                .unwrap(),
            0
        );
        let high =
            CondensedConfiguration::new(vec![Part::new(LabelSet::new(vec![0u8]), 3).unwrap()])
                .unwrap();
        assert_eq!(
            eager_encoding_work(&high, 1, |_| 1, &cancellation).unwrap(),
            usize::MAX
        );
        cancellation.cancel();
        assert!(eager_encoding_work(&input, 0, |_| 0, &cancellation).is_err());
    }

    #[test]
    fn cached_joint_counterexamples_preserve_colored_input_origins_for_all_future_supersets() {
        let problem = crate::parse_problem(serde_json::from_value(serde_json::json!({
            "kind":"mapped", "input":{"active":"ab^3","passive":"ab^3"},
            "output":{"active":"AB^3","passive":"A^3\nA^2 B\nB^3"}, "mapping":"a -> AB\nb -> AB",
        })).unwrap()).unwrap();
        let maximum = crate::algorithms::maximize::compute(&problem).unwrap();
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
        let safe = |mask: u16| {
            let mut counts = vec![0];
            for (color, slots) in [(0, 1), (1, 2)] {
                let a = mask & (0b0101 << (4 * color)) != 0;
                let b = mask & (0b1010 << (4 * color)) != 0;
                if !a && !b {
                    return true;
                }
                counts = counts
                    .into_iter()
                    .flat_map(|old| {
                        (0..=slots)
                            .filter(move |count| (*count == 0 || a) && (*count == slots || b))
                            .map(move |count| old + count)
                    })
                    .collect();
            }
            !counts.contains(&1)
        };
        for mask in 0u16..256 {
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
            assert_eq!(forbidden.is_none(), safe(mask));
            if let Some(sources) = forbidden {
                let required = sources
                    .into_iter()
                    .fold(0u16, |bits, source| bits | (1 << source));
                assert_eq!(required & mask, required);
                for future in 0u16..256 {
                    if future & required == required {
                        assert!(!safe(future));
                    }
                }
            }
        }
    }

    // Independent oracle: allowed iff the number of A outputs is not three.
    fn safe(mask: u8) -> bool {
        let counts = [1, 2, 4];
        let mut totals = vec![0];
        for (input, count) in counts.into_iter().enumerate() {
            let a = mask & (1 << (2 * input)) != 0;
            let b = mask & (1 << (2 * input + 1)) != 0;
            if !a && !b {
                return true;
            } // this emission family cannot occur
            totals = totals
                .into_iter()
                .flat_map(|total| {
                    (0..=count)
                        .filter(move |n| (*n == 0 || a) && (*n == count || b))
                        .map(move |n| total + n)
                })
                .collect();
        }
        !totals.contains(&3)
    }

    #[test]
    fn every_grouped_counterexample_clause_preserves_input_origins() {
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
        let input = CondensedConfiguration::new(
            [1, 2, 4]
                .into_iter()
                .enumerate()
                .map(|(input, count)| Part::new(LabelSet::new(vec![input as u8]), count).unwrap())
                .collect(),
        )
        .unwrap();
        for mask in 0u8..64 {
            let emissions: Vec<_> = (0u8..6)
                .filter(|source| mask & (1 << source) != 0)
                .map(|source| Emission {
                    input: source / 2,
                    output: LabelId(u32::from(source % 2)),
                    source,
                })
                .collect();
            let forbidden = forbidden_combination(
                &target,
                std::slice::from_ref(&input),
                &emissions,
                PassiveMode::General,
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(forbidden.is_none(), safe(mask), "mask {mask}");
            if let Some(sources) = forbidden {
                assert!(!sources.is_empty());
                let required = sources
                    .into_iter()
                    .fold(0, |bits, source| bits | (1 << source));
                assert_eq!(mask & required, required);
                for future in 0u8..64 {
                    if future & required == required {
                        assert!(!safe(future), "clause admitted a valid future assignment");
                    }
                }
            }
        }
    }
}
