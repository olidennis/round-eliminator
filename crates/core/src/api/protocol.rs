//! LLM: Shared request, response, progress-event, and error schema for both transports.

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

/// LLM: The shared server/wasm request envelope: parse a text definition, or perform one
/// LLM: operation on supplied client-owned state. Serde tags and generated TypeScript keep the
/// LLM: transport schema aligned.
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

/// LLM: One requested computation on a problem snapshot. Options select semantics; Rust
/// LLM: chooses applicable cached artifacts instead of requiring the UI to assemble
/// LLM: algorithm-specific inputs.
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

/// LLM: The requested mathematical result, a structured error, or cancellation. Coloring-bound
/// LLM: variants can also carry a partial proof while computation continues.
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

/// LLM: A final response plus the new source-problem cache entries produced by that request.
/// LLM: It does not echo the submitted problem or pre-existing analyses; the client merges
/// LLM: these entries into its state.
#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ResponsePacket {
    pub response: Response,
    pub cache: AnalysisCache,
}

impl From<Response> for ResponsePacket {
    /// LLM: Wrap a response with no newly computed cache entries. Used for immediate replies
    /// LLM: and envelope errors.
    fn from(response: Response) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            response,
            cache: AnalysisCache::default(),
        }
    }
}

/// LLM: The streamed event envelope shared by native SSE and wasm callbacks. Progress,
/// LLM: cumulative cache updates, cumulative partial proofs, and the final response have
/// LLM: distinct meanings and delivery slots.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum ComputationEvent {
    Progress(Progress),
    /// LLM: Cumulative completed entries produced by this request, for the source snapshot.
    CacheUpdate(Box<AnalysisCache>),
    /// LLM: A self-contained proof snapshot, including earlier bounds and witnesses.
    /// LLM: Computation continues afterward; a later snapshot supersedes this one.
    PartialResult(Box<Response>),
    Response(Box<ResponsePacket>),
}

/// LLM: A transport-visible error explanation and optional text-entry location. Algorithm
/// LLM: errors generally have no location; parse errors preserve the field and line that
/// LLM: caused them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct ApiError {
    pub message: String,
    pub location: Option<Location>,
}

impl From<ParseError> for ApiError {
    /// LLM: Preserve the parser's explanation and optional editor location in the transport
    /// LLM: error schema. Used at the text-entry API boundary.
    fn from(error: ParseError) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            message: error.message,
            location: error.location,
        }
    }
}
