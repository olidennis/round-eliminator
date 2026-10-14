//! LLM: Visit auxiliary color assignments without constructing a colored input problem.

use super::ColoringKind;
use crate::algorithms::inputs::LocalInput;
use crate::computation::Cancellation;
use crate::model::{CondensedConfiguration, Input, LabelId, LabelSet, Part, Problem};

/// LLM: An original input descriptor augmented with the auxiliary color information needed by
/// LLM: this coloring encoding. The tuple shape distinguishes local-color from
/// LLM: own/neighbor-color incidences.
pub(super) type ColoredInput = (Option<LabelId>, u32);

/// LLM: Expand a prepared local input descriptor into one input ID or implicit `None` per
/// LLM: port. Used when matching full active contexts to membership choices.
pub(super) fn port_inputs(input: &LocalInput) -> Vec<Option<LabelId>> {
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    match &input.labels {
        Some(labels) => labels.iter().copied().map(Some).collect(),
        None => vec![None; input.degree as usize],
    }
}

/// LLM: Convert an original input descriptor to its output-table index, with zero for implicit
/// LLM: trivial input. Used by both coloring encodings and witness verification.
pub(super) fn input_index(input: Option<LabelId>) -> usize {
    // LLM: Update the current working state before continuing this operation.
    input.map_or(0, |label| label.0 as usize)
}

/// LLM: List the original input descriptors, or the one implicit trivial input. Used to size
/// LLM: and index coloring behavior tables without inventing a dummy alphabet.
pub(super) fn input_labels(problem: &Problem) -> Vec<Option<LabelId>> {
    // LLM: Select the representation path that preserves the supplied input promise and
    // LLM: incidence permissions.
    match problem.input() {
        Input::Trivial => vec![None],
        Input::Explicit(input) => (0..input.labels().len())
            .map(|id| Some(LabelId(id as u32)))
            .collect(),
    }
}

/// LLM: Compute the smallest palette locally compatible with the graph degrees and coloring
/// LLM: kind. Used to validate fixed queries and initialize consecutive-palette search.
pub fn minimum(problem: &Problem, kind: ColoringKind) -> anyhow::Result<u32> {
    // LLM: Prepare `graph` for the following computation; it stores the graph for this stage.
    let graph = problem.graph_class();
    let active = graph.active().iter().copied().max().unwrap_or(0);
    let passive = graph.passive().iter().copied().max().unwrap_or(0);
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if matches!(kind, ColoringKind::GraphEdges | ColoringKind::DistanceTwo)
        && graph.passive().iter().any(|&degree| degree != 2)
    {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if kind == ColoringKind::DistanceTwo {
            anyhow::bail!("Distance-two coloring requires degree-two passive nodes.");
        }
        anyhow::bail!(
            "Graph edge coloring requires degree-two passive nodes; use incidence edge coloring for other graph classes."
        );
    }
    // LLM: Reject degree/count overflow rather than silently wrapping a model invariant.
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

/// LLM: Visit color placements that are distinct except where identical incidence descriptors
/// LLM: can share a placement symmetry. Used to generate auxiliary coloring promises without
/// LLM: duplicate permutations.
pub(super) fn distinct_colors<I: Eq>(
    inputs: &[I],
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    distinct_colors_except(inputs, colors, None, cancellation, visitor)
}

/// LLM: Visit valid distinct-color placements while excluding one optional own color. Used by
/// LLM: distance-two neighbor contexts and the ordinary distinct-color traversal.
pub(super) fn distinct_colors_except<I: Eq>(
    inputs: &[I],
    colors: u32,
    excluded: Option<u32>,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    /// LLM: Extend the current enumeration prefix, visit complete choices, and restore it when
    /// LLM: backtracking. Used by the enclosing exact traversal or independent fixture oracle.
    fn visit<I: Eq>(
        inputs: &[I],
        colors: u32,
        excluded: Option<u32>,
        prefix: &mut Vec<u32>,
        cancellation: &Cancellation,
        visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let position = prefix.len();
        if position == inputs.len() {
            return visitor(prefix);
        }
        // LLM: Use increasing colors within adjacent equal-input slots to avoid equivalent
        // LLM: permutations, while preserving distinct colors.
        let first = if position > 0 && inputs[position] == inputs[position - 1] {
            prefix[position - 1] + 1
        } else {
            0
        };
        // LLM: Visit the color placements required by the auxiliary promise while retaining
        // LLM: original input context.
        for color in first..colors {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if Some(color) == excluded || prefix.contains(&color) {
                continue;
            }
            prefix.push(color);
            let keep_going = visit(inputs, colors, excluded, prefix, cancellation, visitor)?;
            prefix.pop();
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if !keep_going {
                return Ok(false);
            }
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(true)
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    visit(
        inputs,
        colors,
        excluded,
        &mut Vec::new(),
        cancellation,
        visitor,
    )
}

/// LLM: Visit the active color placements required by the selected coloring promise. Used by
/// LLM: membership refinement and independent witness verification.
pub(super) fn active_colors(
    inputs: &[Option<LabelId>],
    kind: ColoringKind,
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&[u32]) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    // LLM: Edge/incidence promises need distinct active colors; vertex coloring supplies one
    // LLM: shared color to all ports.
    if kind != ColoringKind::Vertex {
        return distinct_colors(inputs, colors, cancellation, visitor);
    }
    if inputs.is_empty() {
        return visitor(&[]);
    }
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for color in 0..colors {
        cancellation.check()?;
        if !visitor(&vec![color; inputs.len()])? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// LLM: Visit auxiliary color placements over condensed passive input families. Used by
/// LLM: ordinary coloring encoding and verification; graph-edge colors match across the edge
/// LLM: and other kinds require distinct passive colors.
pub(super) fn passive_families(
    inputs: &[CondensedConfiguration<Option<LabelId>>],
    kind: ColoringKind,
    colors: u32,
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(&CondensedConfiguration<ColoredInput>) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for input in inputs {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if kind == ColoringKind::GraphEdges {
            // LLM: Visit the color placements required by the auxiliary promise while
            // LLM: retaining original input context.
            for color in 0..colors {
                // LLM: Prepare `family` for the following computation; it stores the family
                // LLM: for this stage.
                let family = CondensedConfiguration::new(
                    input
                        .parts()
                        .iter()
                        .map(|part| {
                            // LLM: Delegate to the common implementation so this entry point
                            // LLM: shares its semantics and checks.
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
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if !visitor(&family)? {
                    return Ok(false);
                }
            }
        } else {
            // LLM: Prepare `slots` for the following computation; it stores the slots for this
            // LLM: stage.
            let slots: Vec<_> = input
                .parts()
                .iter()
                .flat_map(|part| std::iter::repeat_n(part.labels(), part.multiplicity() as usize))
                .collect();
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if !distinct_colors(&slots, colors, cancellation, &mut |palette| {
                // LLM: Prepare `family` for the following computation; it stores the family
                // LLM: for this stage.
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
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                visitor(&family)
            })? {
                return Ok(false);
            }
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(true)
}
