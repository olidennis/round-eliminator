//! Transport-independent API. The server and wasm adapter use this same dispatch.

mod protocol;

pub use protocol::{ApiError, ComputationEvent, Request, Response};

use crate::{
    algorithms::zero_round,
    computation::{Cancellation, Computation, Phase},
    parser::parse_problem_with_cancellation,
};

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
        Request::CheckZeroRound {
            problem,
            passive_mode,
        } => match zero_round::check_with(&problem, passive_mode, computation) {
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
mod tests;
