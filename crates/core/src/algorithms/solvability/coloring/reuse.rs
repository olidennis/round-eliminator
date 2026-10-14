//! Restrict already proved coloring strategies to requested palettes.
//! Numerical witnesses retain the actual input table and, for distance two,
//! both color indices. Order certificates expand using endpoint comparisons.

use super::{ColoringResult, ColoringWitness, NeighborColorBehavior, PaletteSearch};
use crate::computation::{Cancellation, Cancelled};

pub fn reuse_result(
    search: PaletteSearch,
    result: &ColoringResult,
    cancellation: &Cancellation,
) -> Result<Option<ColoringResult>, Cancelled> {
    cancellation.check()?;
    if search == PaletteSearch::Maximum {
        return Ok(match result {
            ColoringResult::Maximum { .. }
            | ColoringResult::NoPalette
            | ColoringResult::Unbounded { .. } => Some(result.clone()),
            _ => None,
        });
    }
    let PaletteSearch::Fixed { colors } = search else {
        unreachable!()
    };
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
    let restricted = match witness {
        ColoringWitness::LocalColors { behaviors } => {
            let repeated = matches!(result, ColoringResult::Unbounded { .. });
            let mut rows = Vec::new();
            for color in 0..colors as usize {
                cancellation.check()?;
                rows.push(behaviors[if repeated { 0 } else { color }].clone());
            }
            ColoringWitness::LocalColors { behaviors: rows }
        }
        ColoringWitness::NeighborColors { behaviors } => {
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
            let mut rows = Vec::new();
            for own in 0..colors {
                let mut neighbors = Vec::new();
                for neighbor in 0..colors {
                    cancellation.check()?;
                    neighbors.push(if neighbor < own {
                        smaller.clone()
                    } else if neighbor > own {
                        larger.clone()
                    } else {
                        super::ColorBehavior {
                            outputs_by_input: vec![
                                Default::default();
                                smaller.outputs_by_input.len()
                            ],
                        }
                    });
                }
                rows.push(NeighborColorBehavior {
                    outputs_by_neighbor: neighbors,
                });
            }
            ColoringWitness::NeighborColors { behaviors: rows }
        }
    };
    Ok(Some(ColoringResult::Solvable {
        colors,
        witness: restricted,
    }))
}

/// A zero-round strategy can ignore every kind of auxiliary coloring. For
/// distance two, equal smaller/larger behaviors are an order certificate.
pub(super) fn from_zero_round(
    problem: &crate::model::Problem,
    kind: super::ColoringKind,
    search: PaletteSearch,
    result: &super::ZeroRoundResult,
    cancellation: &Cancellation,
) -> anyhow::Result<Option<ColoringResult>> {
    let super::ZeroRoundResult::Solvable { strategy } = result else {
        return Ok(None);
    };
    let mut outputs = vec![Vec::new(); super::prepare::input_labels(problem).len()];
    for rule in strategy {
        for (port, &output) in rule.output.iter().enumerate() {
            cancellation.check()?;
            let input = rule.input.as_ref().map(|labels| labels[port]);
            outputs[super::prepare::input_index(input)].push(output);
        }
    }
    let behavior = super::ColorBehavior {
        outputs_by_input: outputs
            .into_iter()
            .map(crate::model::LabelSet::new)
            .collect(),
    };
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
    let result = ColoringResult::Unbounded {
        minimum: super::minimum(problem, kind)?,
        witness,
    };
    Ok(reuse_result(search, &result, cancellation)?)
}

/// Completed negative coloring decisions rule out a color-independent strategy.
pub fn excludes_zero_round(result: &ColoringResult) -> bool {
    matches!(
        result,
        ColoringResult::Unsolvable { .. }
            | ColoringResult::Maximum { .. }
            | ColoringResult::NoPalette
    )
}

/// Combine same-kind numerical proofs. Take the tightest upper bound and the
/// largest certified palette, cloning only that palette's witness once.
pub fn bounds_from_results<'a>(
    minimum: u32,
    results: impl IntoIterator<Item = &'a ColoringResult>,
    cancellation: &Cancellation,
) -> Result<super::ColoringBounds, Cancelled> {
    let mut lower = None;
    let mut upper = None;
    for result in results {
        cancellation.check()?;
        let mut tighten = |bound| upper = Some(upper.map_or(bound, |old: u32| old.min(bound)));
        match result {
            ColoringResult::Solvable { colors, witness }
            | ColoringResult::Maximum { colors, witness } => {
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
    Ok(super::ColoringBounds {
        minimum,
        testing: None,
        lower: lower.map(|(n, _)| n),
        upper,
        witness: lower.map(|(_, w)| w.clone()),
    })
}

pub fn maximum_from_bounds(bounds: &super::ColoringBounds) -> Option<ColoringResult> {
    if bounds.upper.is_some_and(|upper| upper < bounds.minimum) {
        return Some(ColoringResult::NoPalette);
    }
    if bounds.lower.is_some() && bounds.lower == bounds.upper {
        return Some(ColoringResult::Maximum {
            colors: bounds.lower.expect("proved palette"),
            witness: bounds.witness.clone().expect("proved lower bound"),
        });
    }
    None
}

/// Combine proofs belonging to the same problem and coloring kind. Keep the
/// witness of the stronger lower bound; never retain the interrupted query.
pub fn merge_bounds(target: &mut super::ColoringBounds, saved: &super::ColoringBounds) {
    if let Some(lower) = saved.lower
        && target.lower.is_none_or(|old| lower > old)
    {
        target.lower = Some(lower);
        target.witness = saved.witness.clone();
    }
    if let Some(upper) = saved.upper {
        target.upper = Some(target.upper.map_or(upper, |old| old.min(upper)));
    }
    target.testing = None;
}
