//! Visit auxiliary color assignments without constructing a colored input problem.

use super::ColoringKind;
use crate::algorithms::inputs::LocalInput;
use crate::computation::Cancellation;
use crate::model::{CondensedConfiguration, Input, LabelId, LabelSet, Part, Problem};

pub(super) type ColoredInput = (Option<LabelId>, u32);

/// Expand occurrences only where port positions and their colors matter.
pub(super) fn port_inputs(input: &LocalInput) -> Vec<Option<LabelId>> {
    match &input.labels {
        Some(labels) => labels.iter().copied().map(Some).collect(),
        None => vec![None; input.degree as usize],
    }
}

/// Original input IDs index the behavior table; trivial input has one row.
pub(super) fn input_index(input: Option<LabelId>) -> usize {
    input.map_or(0, |label| label.0 as usize)
}

pub(super) fn input_labels(problem: &Problem) -> Vec<Option<LabelId>> {
    match problem.input() {
        Input::Trivial => vec![None],
        Input::Explicit(input) => (0..input.labels().len())
            .map(|id| Some(LabelId(id as u32)))
            .collect(),
    }
}

pub(super) fn minimum(problem: &Problem, kind: ColoringKind) -> anyhow::Result<u32> {
    let graph = problem.graph_class();
    let active = graph.active().iter().copied().max().unwrap_or(0);
    let passive = graph.passive().iter().copied().max().unwrap_or(0);
    if matches!(kind, ColoringKind::GraphEdges | ColoringKind::DistanceTwo)
        && graph.passive().iter().any(|&degree| degree != 2)
    {
        if kind == ColoringKind::DistanceTwo {
            anyhow::bail!("Distance-two coloring requires degree-two passive nodes.");
        }
        anyhow::bail!(
            "Graph edge coloring requires degree-two passive nodes; use incidence edge coloring for other graph classes."
        );
    }
    Ok(match kind {
        ColoringKind::Vertex => passive,
        ColoringKind::GraphEdges => active,
        ColoringKind::IncidenceEdges => active.max(passive),
        ColoringKind::DistanceTwo => active
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Distance-two palette size exceeds u32."))?,
    }
    .max(1))
}

/// Visit injective color assignments. Interchangeable adjacent input slots use
/// increasing colors, retaining every distinct joint input/color multiset.
/// Return false when the consumer stops the traversal.
pub(super) fn distinct_colors<I: Eq>(
    inputs: &[I],
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    distinct_colors_except(inputs, colors, None, cancellation, visitor)
}

/// As above, excluding the center color for a distance-two colored neighborhood.
pub(super) fn distinct_colors_except<I: Eq>(
    inputs: &[I],
    colors: u32,
    excluded: Option<u32>,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    fn visit<I: Eq>(
        inputs: &[I],
        colors: u32,
        excluded: Option<u32>,
        prefix: &mut Vec<u32>,
        cancellation: &Cancellation,
        visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        cancellation.check()?;
        let position = prefix.len();
        if position == inputs.len() {
            return visitor(prefix);
        }
        let first = if position > 0 && inputs[position] == inputs[position - 1] {
            prefix[position - 1] + 1
        } else {
            0
        };
        for color in first..colors {
            cancellation.check()?;
            if Some(color) == excluded || prefix.contains(&color) {
                continue;
            }
            prefix.push(color);
            let keep_going = visit(inputs, colors, excluded, prefix, cancellation, visitor)?;
            prefix.pop();
            if !keep_going {
                return Ok(false);
            }
        }
        Ok(true)
    }
    visit(
        inputs,
        colors,
        excluded,
        &mut Vec::new(),
        cancellation,
        visitor,
    )
}

pub(super) fn active_colors(
    inputs: &[Option<LabelId>],
    kind: ColoringKind,
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    if kind != ColoringKind::Vertex {
        return distinct_colors(inputs, colors, cancellation, visitor);
    }
    if inputs.is_empty() {
        return visitor(&[]);
    }
    for color in 0..colors {
        cancellation.check()?;
        if !visitor(&vec![color; inputs.len()])? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Keep original passive input choices condensed. Only color placement is
/// visited: input-label alternatives are never expanded here.
pub(super) fn passive_families(
    inputs: &[CondensedConfiguration<Option<LabelId>>],
    kind: ColoringKind,
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&CondensedConfiguration<ColoredInput>) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    for input in inputs {
        cancellation.check()?;
        if kind == ColoringKind::GraphEdges {
            for color in 0..colors {
                let family = CondensedConfiguration::new(
                    input
                        .parts()
                        .iter()
                        .map(|part| {
                            Part::new(
                                LabelSet::new(
                                    part.labels().iter().map(|label| (label, color)).collect(),
                                ),
                                part.multiplicity(),
                            )
                            .expect("nonempty inputs")
                        })
                        .collect(),
                )
                .expect("degree preserved");
                if !visitor(&family)? {
                    return Ok(false);
                }
            }
        } else {
            let slots: Vec<_> = input
                .parts()
                .iter()
                .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
                .collect();
            if !distinct_colors(&slots, colors, cancellation, &mut |palette| {
                let family = CondensedConfiguration::new(
                    slots
                        .iter()
                        .zip(palette)
                        .map(|(labels, &color)| {
                            Part::new(
                                LabelSet::new(labels.iter().map(|label| (label, color)).collect()),
                                1,
                            )
                            .expect("nonempty inputs")
                        })
                        .collect(),
                )
                .expect("degree preserved");
                visitor(&family)
            })? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
