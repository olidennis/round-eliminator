//! One request and response schema for native server and wasm worker.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    parse_problem,
    parser::{Location, ParseError, ProblemText},
    problem::Problem,
};

/// These are the possible requests that we can receive from the UI
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Request {
    ParseProblem(ProblemText),
}

/// These are the possible answers that we can send to the UI
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Response {
    Problem(Problem),
    Error(ApiError),
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


/// Here we map a request to the actual code that computes the response.
pub fn execute(request: Request) -> Response {
    match request {
        Request::ParseProblem(text) => match parse_problem(text) {
            Ok(problem) => Response::Problem(problem),
            Err(error) => Response::Error(error.into()),
        },
    }
}

/// Requests and responses use json. This is the main entry point.
/// We receive a string with some json, we parse, we call the handler,
/// we create a json output from the response.
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
