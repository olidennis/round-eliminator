//! Transport-independent API. The server and wasm adapter use this same dispatch.

mod protocol;

pub use protocol::{ApiError, ComputationEvent, Request, Response};

use crate::{
    algorithms::solvability::{coloring, zero_round},
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
    execute_with_partial(request, computation, &mut |_| {})
}

fn execute_with_partial(
    request: Request,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
) -> Response {
    let response = execute_inner(request, computation, partial);
    // Cancellation is an execution state, never a proof of unsolvability or a
    // parse error, including when it arrives during range expansion.
    if computation.cancellation.is_cancelled() {
        Response::Cancelled
    } else {
        response
    }
}

/// Here we map a request to the actual code that computes the response.
fn execute_inner(
    request: Request,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
) -> Response {
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
        Request::CheckColoring {
            problem,
            kind,
            search,
            passive_mode,
        } => {
            match coloring::check_with(
                &problem,
                kind,
                search,
                passive_mode,
                computation,
                &mut |bounds| partial(Response::ColoringBounds(bounds)),
            ) {
                Ok(result) => Response::Coloring(result),
                Err(error) => Response::Error(ApiError {
                    message: error.to_string(),
                    location: None,
                }),
            }
        }
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

/// Stream generic progress and partial results through either transport.
/// The final response is returned separately so adapters retain stream ownership.
pub fn execute_text_with_events(
    request: &str,
    cancellation: Cancellation,
    on_event: &mut dyn FnMut(ComputationEvent),
) -> Response {
    let request = match serde_json::from_str::<Request>(request) {
        Ok(request) => request,
        Err(error) => {
            return Response::Error(ApiError {
                message: format!("Invalid request: {error}"),
                location: None,
            });
        }
    };
    // Both callbacks synchronously borrow the same event consumer. No callback
    // retains the borrow while computation or another callback is running.
    let events = std::cell::RefCell::new(on_event);
    let mut progress = |progress| (events.borrow_mut())(ComputationEvent::Progress(progress));
    let mut partial =
        |response| (events.borrow_mut())(ComputationEvent::PartialResult(Box::new(response)));
    execute_with_partial(
        request,
        &mut Computation::new(cancellation, &mut progress),
        &mut partial,
    )
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
