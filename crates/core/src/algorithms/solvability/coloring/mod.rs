//! LLM: Zero-round strategies using both a promised coloring and the original inputs.

mod distance_two;
mod encoding;
mod order;
mod prepare;
mod reuse;
mod verify;
pub use prepare::minimum;
pub use reuse::{
    bounds_from_results, excludes_zero_round, maximum_from_bounds, merge_bounds, reuse_result,
};
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

/// LLM: The promise supplied by the auxiliary coloring: vertex/strong hypergraph, graph-edge,
/// LLM: incidence-edge, or distance-two coloring. Each kind determines which local color
/// LLM: placements must be supported.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColoringKind {
    Vertex,
    GraphEdges,
    IncidenceEdges,
    /// LLM: Proper coloring of the original graph's square. Each active node knows
    /// LLM: its own color and all neighboring colors; passive nodes must have degree 2.
    DistanceTwo,
}

/// LLM: A request to decide one fixed palette or find the maximum supported palette. Maximum
/// LLM: search can also return an unbounded certificate or report that no locally feasible
/// LLM: palette works.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaletteSearch {
    Fixed { colors: u32 },
    Maximum,
}

/// LLM: The output choices available to one local color, indexed by original input. These sets
/// LLM: permit a whole valid active configuration to be selected after seeing the full local
/// LLM: context.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ColorBehavior {
    pub outputs_by_input: Vec<LabelSet>,
}

/// LLM: The output choices for one own-color row, indexed by neighbor color and original
/// LLM: input. Endpoint reversal selects the corresponding opposite-direction table on a
/// LLM: passive degree-two node.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct NeighborColorBehavior {
    pub outputs_by_neighbor: Vec<ColorBehavior>,
}

/// LLM: A checkable strategy certificate: local-color output tables, own/neighbor-color
/// LLM: tables, or smaller/larger order tables for an unbounded distance-two strategy. All
/// LLM: tables retain original input permissions.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColoringWitness {
    LocalColors {
        behaviors: Vec<ColorBehavior>,
    },
    NeighborColors {
        behaviors: Vec<NeighborColorBehavior>,
    },
    /// LLM: The neighbor's color is smaller or larger than the node's own color.
    /// LLM: Both sets retain original input dependence and certify every palette.
    ColorOrder {
        smaller: ColorBehavior,
        larger: ColorBehavior,
    },
}

/// LLM: A fixed-palette decision, a certified finite maximum, an unbounded strategy, or
/// LLM: absence of any feasible palette. Positive decisions carry independently checkable
/// LLM: strategy tables.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
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
    /// LLM: Every locally feasible palette works, using a repeatable local behavior
    /// LLM: or the smaller/larger certificate for visible neighboring colors.
    Unbounded {
        minimum: u32,
        witness: ColoringWitness,
    },
}

/// LLM: The local minimum, current tested palette, proved lower/upper bounds, and lower-bound
/// LLM: witness. The upper bound is inclusive for the maximum; cancellation adds no negative
/// LLM: proof.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ColoringBounds {
    pub minimum: u32,
    pub testing: Option<u32>,
    pub lower: Option<u32>,
    pub upper: Option<u32>,
    pub witness: Option<ColoringWitness>,
}

/// LLM: Borrowed applicable proof artifacts for one coloring request: a passive maximum,
/// LLM: zero-round result, completed same-kind results, and saved numerical bounds. It is a
/// LLM: query view, not a second serialized problem.
#[derive(Default)]
pub struct ColoringCache<'a> {
    pub passive_maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
    pub zero_round: Option<&'a ZeroRoundResult>,
    pub results: Vec<&'a ColoringResult>,
    pub bounds: Option<&'a ColoringBounds>,
}

/// LLM: Decide the requested coloring palette or maximum while retaining original inputs and
/// LLM: mapping restrictions. Used by the shared API and direct callers; applicable cached
/// LLM: proofs initialize the search rather than changing the promise.
pub fn check(
    problem: &Problem,
    kind: ColoringKind,
    search: PaletteSearch,
    mode: PassiveMode,
) -> anyhow::Result<ColoringResult> {
    // LLM: Prepare `progress` for the following computation; it stores the progress for this
    // LLM: stage.
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

/// LLM: Decide the requested coloring palette or maximum while retaining original inputs and
/// LLM: mapping restrictions. Used by the shared API and direct callers; applicable cached
/// LLM: proofs initialize the search rather than changing the promise.
pub fn check_with(
    problem: &Problem,
    kind: ColoringKind,
    search: PaletteSearch,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
) -> anyhow::Result<ColoringResult> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    check_with_cached(
        problem,
        kind,
        search,
        mode,
        ColoringCache::default(),
        computation,
        on_bounds,
    )
}

/// LLM: Decide the requested coloring palette or maximum while retaining original inputs and
/// LLM: mapping restrictions. Used by the shared API and direct callers; applicable cached
/// LLM: proofs initialize the search rather than changing the promise.
pub fn check_with_cached(
    problem: &Problem,
    kind: ColoringKind,
    search: PaletteSearch,
    mode: PassiveMode,
    cache: ColoringCache<'_>,
    computation: &mut Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
) -> anyhow::Result<ColoringResult> {
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let maximum = cache.passive_maximum;
    let zero_round = cache.zero_round;
    let minimum = prepare::minimum(problem, kind)?;
    // LLM: Validate the fixed palette against the coloring promise before applying any cached
    // LLM: decision.
    if let PaletteSearch::Fixed { colors } = search {
        anyhow::ensure!(
            colors >= minimum,
            "This coloring requires at least {minimum} colors locally."
        );
    }
    // LLM: Combine completed same-kind proofs before starting another numerical search.
    for result in &cache.results {
        // LLM: Combine completed same-kind proofs before starting another numerical search.
        if let Some(reused) = reuse_result(search, result, &computation.cancellation)? {
            // LLM: Combine completed same-kind proofs before starting another numerical
            // LLM: search.
            if search == PaletteSearch::Maximum {
                // LLM: Combine completed same-kind proofs before starting another numerical
                // LLM: search.
                let bounds = bounds_from_results(
                    minimum,
                    std::iter::once(*result),
                    &computation.cancellation,
                )?;
                if bounds.lower.is_some() || bounds.upper.is_some() {
                    publish(&bounds, computation, on_bounds)?;
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            return Ok(reused);
        }
    }
    // LLM: Combine completed same-kind proofs before starting another numerical search.
    let mut bounds = bounds_from_results(
        minimum,
        cache.results.iter().copied(),
        &computation.cancellation,
    )?;
    if let Some(saved) = cache.bounds {
        merge_bounds(&mut bounds, saved);
    }
    // LLM: Validate the fixed palette against the coloring promise before applying any cached
    // LLM: decision.
    if let PaletteSearch::Fixed { colors } = search {
        // LLM: A proved inclusive maximum upper bound rules out this larger palette without
        // LLM: another SAT query.
        if bounds.upper.is_some_and(|upper| colors > upper) {
            return Ok(ColoringResult::Unsolvable { colors });
        }
        // LLM: Try deriving this query from a previous completed same-kind proof.
        if let (Some(lower), Some(witness)) = (bounds.lower, &bounds.witness)
            && colors <= lower
        {
            // LLM: Try deriving this query from a previous completed same-kind proof.
            return Ok(reuse_result(
                search,
                &ColoringResult::Solvable {
                    colors: lower,
                    witness: witness.clone(),
                },
                &computation.cancellation,
            )?
            .expect("restriction of a proved palette"));
        }
    }
    // LLM: Return early only when the proved interval already determines the answer.
    if search == PaletteSearch::Maximum
        && let Some(result) = maximum_from_bounds(&bounds)
    {
        publish(&bounds, computation, on_bounds)?;
        return Ok(result);
    }
    // LLM: Turn the color-independent strategy into a certificate that ignores auxiliary
    // LLM: colors.
    if let Some(zero_round) = zero_round
        && let Some(result) =
            reuse::from_zero_round(problem, kind, search, zero_round, &computation.cancellation)?
    {
        return Ok(result);
    }
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    if kind == ColoringKind::DistanceTwo {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if search == PaletteSearch::Maximum
            && bounds.upper.is_none()
            && let Some(witness) = order::check_with(problem, mode, computation)?
        {
            return Ok(ColoringResult::Unbounded { minimum, witness });
        }
        // LLM: Prepare the numerical own/neighbor solver only after the order-based unbounded
        // LLM: preflight has failed.
        let mut solver = distance_two::PaletteSolver::new_with_maximum(
            problem,
            backend(&computation.cancellation),
            maximum,
            computation,
        )?;
        // LLM: Validate the fixed palette against the coloring promise before applying any
        // LLM: cached decision.
        if let PaletteSearch::Fixed { colors } = search {
            return Ok(match solver.solve(colors, mode, computation)? {
                Some(witness) => ColoringResult::Solvable { colors, witness },
                None => ColoringResult::Unsolvable { colors },
            });
        }
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        return search_maximum(
            bounds,
            computation,
            on_bounds,
            &mut |colors, computation| solver.solve(colors, mode, computation),
        );
    }
    // LLM: Turn the color-independent strategy into a certificate that ignores auxiliary
    // LLM: colors.
    if search == PaletteSearch::Maximum && bounds.upper.is_none() {
        // LLM: A color-independent strategy supplies a repeatable behavior. Conversely,
        // LLM: with finite labels, inputs and degrees, there are finitely many behavior
        // LLM: vectors. Arbitrarily large palettes contain enough equal vectors to
        // LLM: cover every active/passive degree and hence give a color-independent
        // LLM: strategy. This proves the remaining maximum search terminates; there
        // LLM: is no heuristic palette cutoff.

        let computed;
        let zero_round = match zero_round {
            Some(result) => result,
            None => {
                computed = zero_round::check_with_maximum(problem, mode, maximum, computation)?;
                &computed
            }
        };
        // LLM: Turn the color-independent strategy into a certificate that ignores auxiliary
        // LLM: colors.
        if let Some(result) =
            reuse::from_zero_round(problem, kind, search, zero_round, &computation.cancellation)?
        {
            return Ok(result);
        }
    }
    // LLM: Keep one local-color incremental solver for all unproved numerical palettes in this
    // LLM: request.
    let mut solver = PaletteSolver::new_with_maximum(
        problem,
        kind,
        backend(&computation.cancellation),
        maximum,
        computation,
    )?;
    // LLM: Validate the fixed palette against the coloring promise before applying any cached
    // LLM: decision.
    if let PaletteSearch::Fixed { colors } = search {
        return Ok(match solver.solve(colors, mode, computation)? {
            Some(witness) => ColoringResult::Solvable { colors, witness },
            None => ColoringResult::Unsolvable { colors },
        });
    }
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    search_maximum(
        bounds,
        computation,
        on_bounds,
        &mut |colors, computation| solver.solve(colors, mode, computation),
    )
}

/// LLM: Try consecutive unproved palettes above the saved lower bound until UNSAT closes the
/// LLM: maximum interval. Shared by all coloring kinds; publish proved bounds and preserve
/// LLM: their witnesses on cancellation.
fn search_maximum(
    mut bounds: ColoringBounds,
    computation: &mut Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
    solve: &mut impl FnMut(u32, &mut Computation<'_>) -> anyhow::Result<Option<ColoringWitness>>,
) -> anyhow::Result<ColoringResult> {
    // LLM: Establish the feasible bound for this local search before exploring or allocating
    // LLM: alternatives.
    let minimum = bounds.minimum;
    if bounds.lower.is_some() || bounds.upper.is_some() {
        publish(&bounds, computation, on_bounds)?;
    }
    // LLM: Reject degree/count overflow rather than silently wrapping a model invariant.
    let mut colors = match bounds.lower {
        Some(lower) => lower
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Palette size exceeds the u32 representation."))?,
        None => minimum,
    };
    // LLM: Reject degree/count overflow rather than silently wrapping a model invariant.
    loop {
        // LLM: Announce the next unproved palette separately from the already proved interval.
        bounds.testing = Some(colors);
        publish(&bounds, computation, on_bounds)?;
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        match solve(colors, computation)? {
            Some(witness) => {
                bounds.lower = Some(colors);
                bounds.witness = Some(witness);
            }
            None => bounds.upper = Some(colors - 1),
        }
        bounds.testing = None;
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        publish(&bounds, computation, on_bounds)?;
        if let Some(result) = maximum_from_bounds(&bounds) {
            return Ok(result);
        }
        colors = colors
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Palette size exceeds the u32 representation."))?;
    }
}

/// LLM: Report a numerical coloring-bound snapshot and propagate cancellation after the
/// LLM: consumer callback. Used by maximum search to distinguish partial proofs from execution
/// LLM: progress.
fn publish(
    bounds: &ColoringBounds,
    computation: &Computation<'_>,
    on_bounds: &mut dyn FnMut(ColoringBounds),
) -> anyhow::Result<()> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    computation.cancellation.check()?;
    on_bounds(bounds.clone());
    computation.cancellation.check()?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod distance_two_tests;
