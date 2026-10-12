//! Shared request, response, progress-event, and error schema for both transports.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    algorithms::diagram::{DiagramOptions, DiagramResult},
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
    ComputeDiagram {
        problem: Problem,
        #[serde(default)]
        options: DiagramOptions,
    },
    CheckZeroRound {
        problem: Problem,
        // Default to general mode when callers omit the field.
        #[serde(default)]
        passive_mode: PassiveMode,
    },
    CheckColoring {
        problem: Problem,
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
    Diagram(DiagramResult),
    ZeroRound(ZeroRoundResult),
    Coloring(ColoringResult),
    ColoringBounds(ColoringBounds),
    Error(ApiError),
    Cancelled,
}

/// Latest progress and cumulative proof snapshots precede a final response.
/// Transports may coalesce superseded updates; the final response is retained.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum ComputationEvent {
    Progress(Progress),
    /// A self-contained proof snapshot, including earlier bounds and witnesses.
    /// Computation continues afterward; a later snapshot supersedes this one.
    PartialResult(Box<Response>),
    Response(Box<Response>),
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
