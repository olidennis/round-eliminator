//! Zero-round strategies using both a promised coloring and the original inputs.

mod distance_two;
mod encoding;
mod order;
mod prepare;
mod verify;
pub use verify::verify_witness;

use crate::{
    algorithms::{
        sat::backend,
        solvability::{
            passive::PassiveMode,
            zero_round::{self, ZeroRoundResult},
        },
    },
    computation::{Cancellation, Computation},
    model::{LabelSet, Problem},
};
use encoding::PaletteSolver;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Color placement and the information available to an active node.
/// Vertex coloring is strong: colors around every passive node are distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColoringKind {
    Vertex,
    GraphEdges,
    IncidenceEdges,
    /// Proper coloring of the original graph's square. Each active node knows
    /// its own color and all neighboring colors; passive nodes must have degree 2.
    DistanceTwo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaletteSearch {
    Fixed { colors: u32 },
    Maximum,
}

/// Output sets indexed by the original input label table; trivial input has one
/// implicit entry. A local rule may choose a different valid output for each
/// full local input/color context. Empty sets are allowed for unused inputs.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ColorBehavior {
    pub outputs_by_input: Vec<LabelSet>,
}

/// Behavior for one own color. Neighbor entries index the neighbor's color;
/// the diagonal entry has empty output sets because adjacent colors differ.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct NeighborColorBehavior {
    pub outputs_by_neighbor: Vec<ColorBehavior>,
}

/// These sets describe a strategy: choose a whole allowed active configuration
/// using the relevant sets. Passive validity holds for every such choice.
/// Behavior at index c belongs to given color c; for NeighborColors this is the
/// node's own color. The UI displays colors from 1.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColoringWitness {
    LocalColors {
        behaviors: Vec<ColorBehavior>,
    },
    NeighborColors {
        behaviors: Vec<NeighborColorBehavior>,
    },
    /// The neighbor's color is smaller or larger than the node's own color.
    /// Both sets retain original input dependence and certify every palette.
    ColorOrder {
        smaller: ColorBehavior,
        larger: ColorBehavior,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ColoringResult {
    Solvable {
        colors: u32,
        witness: ColoringWitness,
    },
    Unsolvable {
        colors: u32,
    },
    Maximum {
        colors: u32,
        witness: ColoringWitness,
    },
    NoPalette,
    /// Every locally feasible palette works, using a repeatable local behavior
    /// or the smaller/larger certificate for visible neighboring colors.
    Unbounded {
        minimum: u32,
        witness: ColoringWitness,
    },
}

/// Proved palette bounds, streamed after each completed SAT decision and before
/// the next query. `upper` is inclusive; neither cancellation nor a timeout
/// supplies an upper bound. The witness certifies `lower`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct ColoringBounds {
    pub minimum: u32,
    pub testing: Option<u32>,
    pub lower: Option<u32>,
    pub upper: Option<u32>,
    pub witness: Option<ColoringWitness>,
}

pub fn check(
    problem: &Problem,
    kind: ColoringKind,
    search: PaletteSearch,
    mode: PassiveMode,
) -> anyhow::Result<ColoringResult> {
    let mut progress = |_| {};
    check_with(
        problem,
        kind,
        search,
        mode,
        &mut Computation::new(Cancellation::default(), &mut progress),
        &mut |_| {},
    )
}

pub fn check_with(
    problem: &Problem,
    kind: ColoringKind,
    search: PaletteSearch,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
) -> anyhow::Result<ColoringResult> {
    let minimum = prepare::minimum(problem, kind)?;
    if let PaletteSearch::Fixed { colors } = search {
        anyhow::ensure!(
            colors >= minimum,
            "This coloring requires at least {minimum} colors locally."
        );
    }
    if kind == ColoringKind::DistanceTwo {
        if search == PaletteSearch::Maximum
            && let Some(witness) = order::check_with(problem, mode, computation)?
        {
            return Ok(ColoringResult::Unbounded { minimum, witness });
        }
        let mut solver = distance_two::PaletteSolver::new(
            problem,
            backend(&computation.cancellation),
            computation,
        )?;
        if let PaletteSearch::Fixed { colors } = search {
            return Ok(match solver.solve(colors, mode, computation)? {
                Some(witness) => ColoringResult::Solvable { colors, witness },
                None => ColoringResult::Unsolvable { colors },
            });
        }
        return search_maximum(
            minimum,
            computation,
            on_bounds,
            &mut |colors, computation| solver.solve(colors, mode, computation),
        );
    }
    if search == PaletteSearch::Maximum {
        // A color-independent strategy supplies a repeatable behavior. Conversely,
        // with finite labels, inputs and degrees, there are finitely many behavior
        // vectors. Arbitrarily large palettes contain enough equal vectors to
        // cover every active/passive degree and hence give a color-independent
        // strategy. This proves the remaining maximum search terminates; there
        // is no heuristic palette cutoff.
        if let ZeroRoundResult::Solvable { strategy } =
            zero_round::check_with(problem, mode, computation)?
        {
            let inputs = prepare::input_labels(problem);
            let mut outputs = vec![Vec::new(); inputs.len()];
            for rule in strategy {
                computation.cancellation.check()?;
                for (port, output) in rule.output.into_iter().enumerate() {
                    let input = rule.input.as_ref().map(|labels| labels[port]);
                    outputs[prepare::input_index(input)].push(output);
                }
            }
            let behavior = ColorBehavior {
                outputs_by_input: outputs.into_iter().map(LabelSet::new).collect(),
            };
            return Ok(ColoringResult::Unbounded {
                minimum,
                witness: ColoringWitness::LocalColors {
                    behaviors: vec![behavior],
                },
            });
        }
    }
    let mut solver = PaletteSolver::new(
        problem,
        kind,
        backend(&computation.cancellation),
        computation,
    )?;
    if let PaletteSearch::Fixed { colors } = search {
        return Ok(match solver.solve(colors, mode, computation)? {
            Some(witness) => ColoringResult::Solvable { colors, witness },
            None => ColoringResult::Unsolvable { colors },
        });
    }
    search_maximum(
        minimum,
        computation,
        on_bounds,
        &mut |colors, computation| solver.solve(colors, mode, computation),
    )
}

/// Solvability is downward closed: restrict a strategy to a smaller palette.
/// Call only after ruling out unbounded solvability with a complete certificate.
fn search_maximum(
    minimum: u32,
    computation: &mut Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
    solve: &mut impl FnMut(u32, &mut Computation<'_>) -> anyhow::Result<Option<ColoringWitness>>,
) -> anyhow::Result<ColoringResult> {
    let mut bounds = ColoringBounds {
        minimum,
        testing: None,
        lower: None,
        upper: None,
        witness: None,
    };
    let mut colors = minimum;
    loop {
        bounds.testing = Some(colors);
        publish(&bounds, computation, on_bounds)?;
        match solve(colors, computation)? {
            Some(witness) => {
                bounds.lower = Some(colors);
                bounds.witness = Some(witness);
            }
            None => bounds.upper = Some(colors - 1),
        }
        bounds.testing = None;
        publish(&bounds, computation, on_bounds)?;
        match (bounds.lower, bounds.upper) {
            (None, Some(_)) => return Ok(ColoringResult::NoPalette),
            (Some(lower), Some(upper)) if lower == upper => {
                return Ok(ColoringResult::Maximum {
                    colors: lower,
                    witness: bounds.witness.expect("proved lower bound"),
                });
            }
            (Some(lower), Some(upper)) => colors = lower + (upper - lower).div_ceil(2),
            (Some(lower), None) => {
                colors = lower.checked_mul(2).ok_or_else(|| {
                    anyhow::anyhow!("Palette size exceeds the u32 representation.")
                })?
            }
            (None, None) => unreachable!("a completed decision sets a bound"),
        }
    }
}

fn publish(
    bounds: &ColoringBounds,
    computation: &Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
) -> anyhow::Result<()> {
    computation.cancellation.check()?;
    on_bounds(bounds.clone());
    computation.cancellation.check()?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod distance_two_tests;
