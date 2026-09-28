//! One request and response schema for native server and wasm worker.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{parse_problem, problem::PlainProblem};

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Request {
    ParseProblem { active: String, passive: String },
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Response {
    Problem(PlainProblem),
    Error(ApiError),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct ApiError {
    pub message: String,
    pub location: Option<Location>,
}

impl ApiError {
    pub(crate) fn parse(message: &str, side: Side, line: usize) -> Self {
        Self {
            message: message.to_owned(),
            location: Some(Location { side, line }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
pub struct Location {
    pub side: Side,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Active,
    Passive,
}

pub fn execute(request: Request) -> Response {
    match request {
        Request::ParseProblem { active, passive } => match parse_problem(&active, &passive) {
            Ok(problem) => Response::Problem(problem),
            Err(error) => Response::Error(error),
        },
    }
}

/// The identical JSON entry point used by both transport adapters.
pub fn execute_json(request: &str) -> String {
    let response = match serde_json::from_str::<Request>(request) {
        Ok(request) => execute(request),
        Err(error) => Response::Error(ApiError {
            message: format!("Invalid request: {error}"),
            location: None,
        }),
    };
    serde_json::to_string(&response).expect("protocol responses must serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_protocol_returns_problem_and_structured_error() {
        let request = r#"{"type":"parse_problem","data":{"active":"A B","passive":"AB"}}"#;
        let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
        assert_eq!(result["type"], "problem");
        assert_eq!(result["data"]["labels"], serde_json::json!(["A", "B"]));

        let request = r#"{"type":"parse_problem","data":{"active":"A","passive":"B^0"}}"#;
        let result: serde_json::Value = serde_json::from_str(&execute_json(request)).unwrap();
        assert_eq!(result["type"], "error");
        assert_eq!(result["data"]["location"]["side"], "passive");
        assert_eq!(result["data"]["location"]["line"], 1);

        let result: serde_json::Value = serde_json::from_str(&execute_json("not json")).unwrap();
        assert_eq!(result["type"], "error");
        assert!(result["data"]["location"].is_null());
    }
}
