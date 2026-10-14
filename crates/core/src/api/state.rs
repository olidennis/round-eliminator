//! LLM: Client-owned computational state for one immutable problem snapshot.
//! LLM: This is sent with requests; neither transport retains a backend session.

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

/// LLM: A client-owned immutable problem snapshot and its reusable analysis artifacts.
/// LLM: Requests send this object to the stateless backend; a transformed problem starts with
/// LLM: an empty analysis cache.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct ProblemState {
    pub problem: Problem,
    #[serde(default)]
    pub analyses: AnalysisCache,
}

impl From<Problem> for ProblemState {
    /// LLM: Attach an empty analysis cache to a new problem snapshot. Used for fresh
    /// LLM: parsing/transformation results so proofs from another problem are never inherited.
    fn from(problem: Problem) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            problem,
            analyses: AnalysisCache::default(),
        }
    }
}

/// LLM: Completed analyses and proved palette intervals belonging to one unchanged problem.
/// LLM: IDs refer to that snapshot. Cache selection must respect the scope of each proof;
/// LLM: interrupted queries and partial maxima are not certificates.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(default)]
pub struct AnalysisCache {
    pub maximization: Option<MaximizationResult>,
    pub zero_round: Option<ZeroRoundResult>,
    pub coloring: Vec<CachedColoring>,
    pub diagrams: Vec<DiagramResult>,
    pub coloring_bounds: Vec<CachedColoringBounds>,
}

/// LLM: A completed coloring result paired with its coloring kind and query. The keys
/// LLM: distinguish numerical palette decisions, maximum searches, and incompatible kinds of
/// LLM: coloring.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct CachedColoring {
    pub kind: ColoringKind,
    pub search: PaletteSearch,
    pub result: ColoringResult,
}

/// LLM: The proved numerical interval for one coloring kind, with a witness for its lower
/// LLM: bound. It belongs to the same problem snapshot; `testing` is cleared because an
/// LLM: interrupted query proves no upper bound.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct CachedColoringBounds {
    pub kind: ColoringKind,
    pub bounds: ColoringBounds,
}

impl AnalysisCache {
    /// LLM: Find a cached relation equivalent to the effective requested options, allowing a
    /// LLM: complete relation to satisfy a weaker request. Used by the stateless diagram
    /// LLM: operation before new comparison work.
    pub fn diagram(&self, options: DiagramOptions) -> Option<DiagramResult> {
        // LLM: Normalize the method through a compatible source maximum before searching
        // LLM: equivalent completed relations.
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
