//! One request and response schema for native server and wasm worker.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    algorithms::zero_round::{self, ZeroRoundResult},
    computation::{Cancellation, Computation, Phase, Progress},
    parser::{Location, ParseError, ProblemText, parse_problem_with_cancellation},
    problem::Problem,
};

/// These are the possible requests that we can receive from the UI
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Request {
    ParseProblem(ProblemText),
    CheckZeroRound(Problem),
}

/// These are the possible answers that we can send to the UI
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Response {
    Problem(Problem),
    ZeroRound(ZeroRoundResult),
    Error(ApiError),
    Cancelled,
}

/// Computation could just proceed, and we send the Progress status,
/// or it ends, and we send the result as Response.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum ComputationEvent {
    Progress(Progress),
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

pub fn execute(request: Request) -> Response {
    let mut report = |_| {};
    execute_with(
        request,
        &mut Computation::new(Cancellation::default(), &mut report),
    )
}

pub fn execute_with(request: Request, computation: &mut Computation<'_>) -> Response {
    let response = execute_inner(request, computation);
    // Cancellation is an execution state, never a proof of unsolvability or a
    // parse error, including when it arrives during range expansion.
    if computation.cancellation.is_cancelled() {
        Response::Cancelled
    } else {
        response
    }
}

/// Here we map a request to the actual code that computes the response.
fn execute_inner(request: Request, computation: &mut Computation<'_>) -> Response {
    match request {
        Request::ParseProblem(text) => {
            if computation.report(Phase::Parsing, 0).is_err() {
                return Response::Cancelled;
            }
            match parse_problem_with_cancellation(text, &computation.cancellation) {
                Ok(problem) => Response::Problem(problem),
                Err(error) => Response::Error(error.into()),
            }
        }
        Request::CheckZeroRound(problem) => match zero_round::check_with(&problem, computation) {
            Ok(result) => Response::ZeroRound(result),
            Err(error) => Response::Error(ApiError {
                message: error.to_string(),
                location: None,
            }),
        },
    }
}

/// Parse the transport envelope and run the same dispatch as direct Rust calls.
/// Adapters supply cancellation and a progress consumer, without request maps.
pub fn execute_text(request: &str, computation: &mut Computation<'_>) -> Response {
    match serde_json::from_str::<Request>(request) {
        Ok(request) => execute_with(request, computation),
        Err(error) => Response::Error(ApiError {
            message: format!("Invalid request: {error}"),
            location: None,
        }),
    }
}

/// Convenience entry point for callers that only need the final JSON response.
pub fn execute_json(request: &str) -> String {
    let mut report = |_| {};
    let response = execute_text(
        request,
        &mut Computation::new(Cancellation::default(), &mut report),
    );
    serde_json::to_string(&response).expect("protocol responses must serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis_request(active: &str, passive: &str) -> serde_json::Value {
        let text = serde_json::from_value(serde_json::json!({
            "kind":"plain", "constraints":{"active":active, "passive":passive}
        }))
        .unwrap();
        let problem = crate::parser::parse_problem(text).unwrap();
        serde_json::json!({"type":"check_zero_round", "data":problem})
    }

    #[test]
    fn cancellation_at_each_phase_is_never_reported_as_unsolvability() {
        for phase in [
            Phase::Enumerating,
            Phase::Encoding,
            Phase::Solving,
            Phase::Verifying,
        ] {
            let cancellation = Cancellation::default();
            let mut seen = Vec::new();
            let mut report = |progress: Progress| {
                seen.push(progress.phase);
                if progress.phase == phase {
                    cancellation.cancel();
                }
            };
            let request = analysis_request("A B", "AB^3").to_string();
            let response = execute_text(
                &request,
                &mut Computation::new(cancellation.clone(), &mut report),
            );
            assert!(matches!(response, Response::Cancelled));
            assert_eq!(seen.last(), Some(&phase));
        }
        let cancellation = Cancellation::default();
        let mut report = |progress: Progress| {
            if progress.phase == Phase::Parsing {
                cancellation.cancel();
            }
        };
        let response = execute_text(
            r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A","passive":"A"}}}"#,
            &mut Computation::new(cancellation.clone(), &mut report),
        );
        assert!(matches!(response, Response::Cancelled));
    }

    #[test]
    fn analysis_reuses_the_start_result_without_a_parsing_phase() {
        let parsed = execute_json(
            r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"AB^0..2","passive":"AB^2"}}}"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert_eq!(parsed["type"], "problem");
        let request = serde_json::json!({"type":"check_zero_round", "data":parsed["data"]});
        let mut phases = Vec::new();
        let mut report = |progress: Progress| phases.push(progress.phase);
        let response = execute_text(
            &request.to_string(),
            &mut Computation::new(Cancellation::default(), &mut report),
        );
        let response = serde_json::to_value(response).unwrap();
        assert_eq!(response["type"], "zero_round");
        assert_eq!(response["data"]["status"], "solvable");
        assert!(response["data"].get("problem").is_none());
        assert!(response["data"].get("result").is_none());
        assert_eq!(phases.first(), Some(&Phase::Enumerating));
        assert!(!phases.contains(&Phase::Parsing));
    }

    #[test]
    fn zero_round_protocol_returns_results_for_structured_problems() {
        for (active, passive, status) in [("A B", "AB^3", "solvable"), ("A B", "A B", "unsolvable")]
        {
            let request = analysis_request(active, passive);
            let result: serde_json::Value =
                serde_json::from_str(&execute_json(&request.to_string())).unwrap();
            assert_eq!(result["type"], "zero_round");
            assert_eq!(result["data"]["status"], status);
            if status == "solvable" {
                assert_eq!(
                    result["data"]["strategy"][0]["output"]
                        .as_array()
                        .unwrap()
                        .len(),
                    2
                );
            } else {
                assert_eq!(
                    result,
                    serde_json::json!({"type":"zero_round", "data":{"status":"unsolvable"}})
                );
            }
        }
    }

    #[test]
    fn json_protocol_returns_problem_and_structured_error() {
        let request = r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A B","passive":"AB"}}}"#;
        let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
        assert_eq!(result["type"], "problem");
        assert_eq!(
            result["data"]["data"]["output"]["labels"],
            serde_json::json!(["A", "B"])
        );

        let request = r#"{"type":"parse_problem","data":{"kind":"plain","constraints":{"active":"A","passive":"B^2..1"}}}"#;
        let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
        assert_eq!(result["type"], "error");
        assert_eq!(result["data"]["location"]["field"], "passive");
        assert_eq!(result["data"]["location"]["line"], 1);

        let result: serde_json::Value = serde_json::from_str(&execute_json("not json")).unwrap();
        assert_eq!(result["type"], "error");
        assert!(result["data"]["location"].is_null());
    }
}
