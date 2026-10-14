//! LLM: Restrict already proved coloring strategies to requested palettes.
//! LLM: Numerical witnesses retain the actual input table and, for distance two,
//! LLM: both color indices. Order certificates expand using endpoint comparisons.

use super::{ColoringResult, ColoringWitness, NeighborColorBehavior, PaletteSearch};
use crate::computation::{Cancellation, Cancelled};

/// LLM: Derive a requested same-kind coloring decision from a completed palette proof or
/// LLM: restrict its witness to a smaller palette. Used by stateless cache selection before
/// LLM: running SAT.
pub fn reuse_result(
    search: PaletteSearch,
    result: &ColoringResult,
    cancellation: &Cancellation,
) -> Result<Option<ColoringResult>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    if search == PaletteSearch::Maximum {
        // LLM: Return the result of this completed search or conversion to its caller.
        return Ok(match result {
            ColoringResult::Maximum { .. }
            | ColoringResult::NoPalette
            | ColoringResult::Unbounded { .. } => Some(result.clone()),
            _ => None,
        });
    }
    // LLM: Prepare `PaletteSearch` for the following computation; it stores the palette search
    // LLM: for this stage.
    let PaletteSearch::Fixed { colors } = search else {
        unreachable!()
    };
    // LLM: Construct a checkable strategy from the current output choices.
    let witness = match result {
        ColoringResult::NoPalette => return Ok(Some(ColoringResult::Unsolvable { colors })),
        ColoringResult::Maximum {
            colors: maximum, ..
        } if colors > *maximum => return Ok(Some(ColoringResult::Unsolvable { colors })),
        ColoringResult::Unsolvable { colors: failed } if colors >= *failed => {
            return Ok(Some(ColoringResult::Unsolvable { colors }));
        }
        ColoringResult::Solvable {
            colors: bound,
            witness,
        }
        | ColoringResult::Maximum {
            colors: bound,
            witness,
        } if colors <= *bound => witness,
        ColoringResult::Unbounded { witness, .. } => witness,
        _ => return Ok(None),
    };
    // LLM: Observe cooperative interruption before performing the next unit of CPU work.
    let restricted = match witness {
        ColoringWitness::LocalColors { behaviors } => {
            // LLM: Prepare `repeated` for the following computation; it stores the repeated
            // LLM: for this stage.
            let repeated = matches!(result, ColoringResult::Unbounded { .. });
            let mut rows = Vec::new();
            for color in 0..colors as usize {
                cancellation.check()?;
                rows.push(behaviors[if repeated { 0 } else { color }].clone());
            }
            ColoringWitness::LocalColors { behaviors: rows }
        }
        ColoringWitness::NeighborColors { behaviors } => {
            // LLM: Prepare counted rows or remaining row capacities for the next exact
            // LLM: comparison.
            let mut rows = Vec::new();
            for row in &behaviors[..colors as usize] {
                cancellation.check()?;
                rows.push(NeighborColorBehavior {
                    outputs_by_neighbor: row.outputs_by_neighbor[..colors as usize].to_vec(),
                });
            }
            ColoringWitness::NeighborColors { behaviors: rows }
        }
        ColoringWitness::ColorOrder { smaller, larger } => {
            // LLM: Prepare counted rows or remaining row capacities for the next exact
            // LLM: comparison.
            let mut rows = Vec::new();
            for own in 0..colors {
                // LLM: Restrict both own-color rows and neighbor-color columns when reusing a
                // LLM: numerical distance-two witness.
                let mut neighbors = Vec::new();
                for neighbor in 0..colors {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    neighbors.push(if neighbor < own {
                        smaller.clone()
                    } else if neighbor > own {
                        larger.clone()
                    } else {
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        super::ColorBehavior {
                            outputs_by_input: vec![
                                Default::default();
                                smaller.outputs_by_input.len()
                            ],
                        }
                    });
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                rows.push(NeighborColorBehavior {
                    outputs_by_neighbor: neighbors,
                });
            }
            // LLM: Update the current working state before continuing this operation.
            ColoringWitness::NeighborColors { behaviors: rows }
        }
    };
    // LLM: Return the complete strategy certificate after all required contexts have been
    // LLM: covered.
    Ok(Some(ColoringResult::Solvable {
        colors,
        witness: restricted,
    }))
}

/// LLM: Convert a color-independent zero-round strategy to an unbounded coloring certificate
/// LLM: that retains all original inputs. Used by maximum preflight and cached proof reuse.
pub(super) fn from_zero_round(
    problem: &crate::model::Problem,
    kind: super::ColoringKind,
    search: PaletteSearch,
    result: &super::ZeroRoundResult,
    cancellation: &Cancellation,
) -> anyhow::Result<Option<ColoringResult>> {
    // LLM: Prepare `super` for the following computation; it stores the super for this stage.
    let super::ZeroRoundResult::Solvable { strategy } = result else {
        return Ok(None);
    };
    let mut outputs = vec![Vec::new(); super::prepare::input_labels(problem).len()];
    // LLM: Process the relevant label choices while preserving their table identities.
    for rule in strategy {
        for (port, &output) in rule.output.iter().enumerate() {
            cancellation.check()?;
            let input = rule.input.as_ref().map(|labels| labels[port]);
            outputs[super::prepare::input_index(input)].push(output);
        }
    }
    // LLM: Prepare `behavior` for the following computation; it stores the behavior for this
    // LLM: stage.
    let behavior = super::ColorBehavior {
        outputs_by_input: outputs
            .into_iter()
            .map(crate::model::LabelSet::new)
            .collect(),
    };
    // LLM: Construct a checkable strategy from the current output choices.
    let witness = if kind == super::ColoringKind::DistanceTwo {
        ColoringWitness::ColorOrder {
            smaller: behavior.clone(),
            larger: behavior,
        }
    } else {
        ColoringWitness::LocalColors {
            behaviors: vec![behavior],
        }
    };
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let result = ColoringResult::Unbounded {
        minimum: super::minimum(problem, kind)?,
        witness,
    };
    Ok(reuse_result(search, &result, cancellation)?)
}

/// LLM: Recognize completed coloring results that rule out a color-independent strategy. Used
/// LLM: by the API to infer a negative zero-round result without another solve.
pub fn excludes_zero_round(result: &ColoringResult) -> bool {
    // LLM: Update the current working state before continuing this operation.
    matches!(
        result,
        ColoringResult::Unsolvable { .. }
            | ColoringResult::Maximum { .. }
            | ColoringResult::NoPalette
    )
}

/// LLM: Combine completed same-kind numerical proofs into the tightest known palette interval
/// LLM: and its best lower witness. Used to initialize or resume maximum search.
pub fn bounds_from_results<'a>(
    minimum: u32,
    results: impl IntoIterator<Item = &'a ColoringResult>,
    cancellation: &Cancellation,
) -> Result<super::ColoringBounds, Cancelled> {
    // LLM: Prepare `lower` for the following computation; it stores the lower for this stage.
    let mut lower = None;
    let mut upper = None;
    // LLM: Visit the color placements required by the auxiliary promise while retaining
    // LLM: original input context.
    for result in results {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let mut tighten = |bound| upper = Some(upper.map_or(bound, |old: u32| old.min(bound)));
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match result {
            ColoringResult::Solvable { colors, witness }
            | ColoringResult::Maximum { colors, witness } => {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if lower.is_none_or(|(old, _)| *colors > old) {
                    lower = Some((*colors, witness));
                }
                if matches!(result, ColoringResult::Maximum { .. }) {
                    tighten(*colors);
                }
            }
            ColoringResult::Unsolvable { colors } => tighten(colors.saturating_sub(1)),
            ColoringResult::NoPalette => tighten(minimum - 1),
            ColoringResult::Unbounded { .. } => {}
        }
    }
    // LLM: Return the complete strategy certificate after all required contexts have been
    // LLM: covered.
    Ok(super::ColoringBounds {
        minimum,
        testing: None,
        lower: lower.map(|(n, _)| n),
        upper,
        witness: lower.map(|(_, w)| w.clone()),
    })
}

/// LLM: Return a completed maximum or no-palette result only when proved bounds settle the
/// LLM: interval. Used by cache reuse; an upper bound alone is not a maximum.
pub fn maximum_from_bounds(bounds: &super::ColoringBounds) -> Option<ColoringResult> {
    // LLM: An upper bound below the local minimum proves that no feasible palette works.
    if bounds.upper.is_some_and(|upper| upper < bounds.minimum) {
        return Some(ColoringResult::NoPalette);
    }
    // LLM: Equal proved endpoints settle the maximum only when the lower endpoint has its
    // LLM: strategy witness.
    if bounds.lower.is_some() && bounds.lower == bounds.upper {
        return Some(ColoringResult::Maximum {
            colors: bounds.lower.expect("proved palette"),
            witness: bounds.witness.clone().expect("proved lower bound"),
        });
    }
    None
}

/// LLM: Merge proved bounds for the same problem and coloring kind, retaining the stronger
/// LLM: lower witness and clearing the interrupted query. Used by resumption from client-owned
/// LLM: state.
pub fn merge_bounds(target: &mut super::ColoringBounds, saved: &super::ColoringBounds) {
    // LLM: Keep the stronger lower proof and its associated witness, rather than the transient
    // LLM: tested palette.
    if let Some(lower) = saved.lower
        && target.lower.is_none_or(|old| lower > old)
    {
        target.lower = Some(lower);
        target.witness = saved.witness.clone();
    }
    // LLM: Intersect the proved upper bounds; an interrupted query never supplies one.
    if let Some(upper) = saved.upper {
        target.upper = Some(target.upper.map_or(upper, |old| old.min(upper)));
    }
    target.testing = None;
}
