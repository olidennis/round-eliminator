//! Shared request, response, progress-event, and error schema for both transports.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::state::{AnalysisCache, ProblemState};

use crate::{
    algorithms::diagram::{DiagramOptions, DiagramResult},
    algorithms::maximize::MaximizationResult,
    algorithms::solvability::{
        coloring::{ColoringBounds, ColoringKind, ColoringResult, PaletteSearch},
        passive::PassiveMode,
        zero_round::ZeroRoundResult,
    },
    computation::Progress,
    model::Problem,
    parser::{Location, ParseError, ProblemText},
};

/// These are the possible requests that we can receive from the UI
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Request {
    ParseProblem(ProblemText),
    Compute {
        state: ProblemState,
        operation: Operation,
    },
}

/// An action on a supplied snapshot. Cache selection belongs to Rust.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Operation {
    Maximize,
    ApplyRe,
    BasicFixedPoint,
    ComputeDiagram {
        #[serde(default)]
        options: DiagramOptions,
    },
    CheckZeroRound {
        #[serde(default)]
        passive_mode: PassiveMode,
    },
    CheckColoring {
        kind: ColoringKind,
        search: PaletteSearch,
        #[serde(default)]
        passive_mode: PassiveMode,
    },
}

/// These are the possible answers that we can send to the UI
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Response {
    Problem(Problem),
    Maximization(MaximizationResult),
    Diagram(DiagramResult),
    ZeroRound(ZeroRoundResult),
    Coloring(ColoringResult),
    ColoringBounds(ColoringBounds),
    Error(ApiError),
    Cancelled,
}

/// The requested result and completed source-cache entries produced by it.
/// The submitted problem and pre-existing entries are never echoed.
#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ResponsePacket {
    pub response: Response,
    pub cache: AnalysisCache,
}

impl From<Response> for ResponsePacket {
    fn from(response: Response) -> Self {
        Self {
            response,
            cache: AnalysisCache::default(),
        }
    }
}

/// Latest progress and cumulative proof snapshots precede a final response.
/// Transports may coalesce superseded updates; the final response is retained.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum ComputationEvent {
    Progress(Progress),
    /// Cumulative completed entries produced by this request, for the source snapshot.
    CacheUpdate(Box<AnalysisCache>),
    /// A self-contained proof snapshot, including earlier bounds and witnesses.
    /// Computation continues afterward; a later snapshot supersedes this one.
    PartialResult(Box<Response>),
    Response(Box<ResponsePacket>),
}

/// In case of error, we give a message and we say what it refers to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct ApiError {
    pub message: String,
    pub location: Option<Location>,
}

impl From<ParseError> for ApiError {
    fn from(error: ParseError) -> Self {
        Self {
            message: error.message,
            location: error.location,
        }
    }
}
