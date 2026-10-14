//! Client-owned computational state for one immutable problem snapshot.
//! This is sent with requests; neither transport retains a backend session.

use crate::{
    algorithms::{
        diagram::{DiagramOptions, DiagramResult},
        maximize::MaximizationResult,
        solvability::{
            coloring::{ColoringBounds, ColoringKind, ColoringResult, PaletteSearch},
            zero_round::ZeroRoundResult,
        },
    },
    model::Problem,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ProblemState {
    pub problem: Problem,
    #[serde(default)]
    pub analyses: AnalysisCache,
}

impl From<Problem> for ProblemState {
    fn from(problem: Problem) -> Self {
        Self {
            problem,
            analyses: AnalysisCache::default(),
        }
    }
}

/// Completed results and proved palette bounds only. In-flight queries and
/// failed decisions are not proofs. Label IDs always refer to the enclosing problem snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(default)]
pub struct AnalysisCache {
    pub maximization: Option<MaximizationResult>,
    pub zero_round: Option<ZeroRoundResult>,
    pub coloring: Vec<CachedColoring>,
    pub diagrams: Vec<DiagramResult>,
    pub coloring_bounds: Vec<CachedColoringBounds>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct CachedColoring {
    pub kind: ColoringKind,
    pub search: PaletteSearch,
    pub result: ColoringResult,
}

/// The best proved interval for one coloring kind, including the lower witness.
/// Cancellation does not establish an upper bound. `testing` is always omitted
/// from this artifact by setting it to None before publication.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct CachedColoringBounds {
    pub kind: ColoringKind,
    pub bounds: ColoringBounds,
}

impl AnalysisCache {
    pub fn diagram(&self, options: DiagramOptions) -> Option<DiagramResult> {
        let options = options.with_maximum(self.maximization.as_ref());
        self.diagrams
            .iter()
            .find(|cached| {
                cached.options.same_computation(options)
                    || (cached.options.kind == options.kind && cached.diagram.complete)
            })
            .map(|cached| DiagramResult {
                options,
                diagram: cached.diagram.clone(),
            })
    }
}
