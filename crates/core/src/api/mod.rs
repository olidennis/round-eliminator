//! Transport-independent API. The server and wasm adapter use this same dispatch.

mod protocol;
mod state;

pub use protocol::{ApiError, ComputationEvent, Operation, Request, Response, ResponsePacket};
pub use state::{AnalysisCache, CachedColoring, CachedColoringBounds, ProblemState};

use crate::{
    algorithms::{
        diagram, fixed_point, maximize, round_elimination,
        solvability::{coloring, zero_round},
    },
    computation::{Cancellation, Computation, Phase},
    parser::parse_problem_with_cancellation,
};

pub fn execute(request: Request) -> ResponsePacket {
    execute_with(
        request,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

pub fn execute_with(request: Request, computation: &mut Computation<'_>) -> ResponsePacket {
    execute_reporting(request, computation, &mut |_| {}, &mut |_| {})
}

fn execute_reporting(
    request: Request,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
    cache_update: &mut dyn FnMut(AnalysisCache),
) -> ResponsePacket {
    let mut cache = AnalysisCache::default();
    let result = match request {
        Request::ParseProblem(text) => (|| {
            computation.report(Phase::Parsing, 0)?;
            Ok(Response::Problem(parse_problem_with_cancellation(
                text,
                &computation.cancellation,
            )?))
        })(),
        Request::Compute { state, operation } => execute_operation(
            &state,
            operation,
            computation,
            partial,
            &mut cache,
            cache_update,
        ),
    };
    let response = if computation.cancellation.is_cancelled() {
        Response::Cancelled
    } else {
        match result {
            Ok(response) => response,
            Err(error) => Response::Error(ApiError {
                location: error
                    .downcast_ref::<crate::parser::ParseError>()
                    .and_then(|error| error.location.clone()),
                message: error.to_string(),
            }),
        }
    };
    ResponsePacket { response, cache }
}

/// Algorithms consume only mathematically applicable artifacts. The client
/// sends one snapshot and has no operation-specific cache-selection policy.
fn execute_operation(
    state: &ProblemState,
    operation: Operation,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
    updates: &mut AnalysisCache,
    cache_update: &mut dyn FnMut(AnalysisCache),
) -> anyhow::Result<Response> {
    let problem = &state.problem;
    let cached = &state.analyses;
    computation.cancellation.check()?;
    match operation {
        Operation::BasicFixedPoint => Ok(Response::Problem(fixed_point::basic_with(
            problem,
            computation,
        )?)),
        Operation::Maximize | Operation::ApplyRe => {
            let computed;
            let maximum = if let Some(maximum) = &cached.maximization {
                maximum
            } else {
                computed = maximize::compute_with(problem, computation)?;
                updates.maximization = Some(computed);
                cache_update(updates.clone());
                updates.maximization.as_ref().expect("completed maximum")
            };
            if matches!(operation, Operation::ApplyRe) {
                Ok(Response::Problem(round_elimination::apply_with(
                    problem,
                    maximum,
                    computation,
                )?))
            } else {
                Ok(Response::Maximization(maximum.clone()))
            }
        }
        Operation::ComputeDiagram { options } => {
            if let Some(result) = cached.diagram(options) {
                return Ok(Response::Diagram(result));
            }
            let result = diagram::compute_with_maximum(
                problem,
                options,
                cached.maximization.as_ref(),
                computation,
            )?;
            updates.diagrams.push(result.clone());
            cache_update(updates.clone());
            Ok(Response::Diagram(result))
        }
        Operation::CheckZeroRound { passive_mode } => {
            if let Some(result) = &cached.zero_round {
                return Ok(Response::ZeroRound(result.clone()));
            }
            let result = if cached
                .coloring
                .iter()
                .any(|entry| coloring::excludes_zero_round(&entry.result))
                || cached
                    .coloring_bounds
                    .iter()
                    .any(|entry| entry.bounds.upper.is_some())
            {
                zero_round::ZeroRoundResult::Unsolvable
            } else {
                zero_round::check_with_maximum(
                    problem,
                    passive_mode,
                    cached.maximization.as_ref(),
                    computation,
                )?
            };
            updates.zero_round = Some(result.clone());
            cache_update(updates.clone());
            Ok(Response::ZeroRound(result))
        }
        Operation::CheckColoring {
            kind,
            search,
            passive_mode,
        } => {
            let minimum = coloring::minimum(problem, kind)?;
            if let coloring::PaletteSearch::Fixed { colors } = search {
                anyhow::ensure!(
                    colors >= minimum,
                    "This coloring requires at least {minimum} colors locally."
                );
            }
            // Exact queries first, then valid palette restriction/bound proofs.
            let previous = cached.coloring.iter().filter(|entry| entry.kind == kind);
            let previous = previous
                .clone()
                .filter(|entry| entry.search == search)
                .chain(previous.filter(|entry| entry.search != search));
            for entry in previous {
                if let Some(result) =
                    coloring::reuse_result(search, &entry.result, &computation.cancellation)?
                {
                    publish_coloring_bounds(&result, minimum, partial);
                    if entry.search != search {
                        updates.coloring.push(CachedColoring {
                            kind,
                            search,
                            result: result.clone(),
                        });
                        cache_update(updates.clone());
                    }
                    return Ok(Response::Coloring(result));
                }
            }
            let mut bounds = coloring::bounds_from_results(
                minimum,
                cached
                    .coloring
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .map(|entry| &entry.result),
                &computation.cancellation,
            )?;
            let saved_bounds = cached
                .coloring_bounds
                .iter()
                .find(|entry| entry.kind == kind)
                .map(|entry| &entry.bounds);
            if let Some(saved) = saved_bounds {
                coloring::merge_bounds(&mut bounds, saved);
            }
            if search == coloring::PaletteSearch::Maximum
                && let Some(result) = coloring::maximum_from_bounds(&bounds)
            {
                publish_coloring_bounds(&result, minimum, partial);
                updates.coloring.push(CachedColoring {
                    kind,
                    search,
                    result: result.clone(),
                });
                cache_update(updates.clone());
                return Ok(Response::Coloring(result));
            }
            // This preflight is already part of ordinary maximum search. Retain
            // its completed proof, including when the subsequent search is cancelled.
            if kind != coloring::ColoringKind::DistanceTwo
                && search == coloring::PaletteSearch::Maximum
                && cached.zero_round.is_none()
                && bounds.upper.is_none()
            {
                updates.zero_round = Some(
                    if cached
                        .coloring
                        .iter()
                        .any(|entry| coloring::excludes_zero_round(&entry.result))
                    {
                        zero_round::ZeroRoundResult::Unsolvable
                    } else {
                        zero_round::check_with_maximum(
                            problem,
                            passive_mode,
                            cached.maximization.as_ref(),
                            computation,
                        )?
                    },
                );
                cache_update(updates.clone());
            }
            let preflight = updates.zero_round.clone();
            let result = coloring::check_with_cached(
                problem,
                kind,
                search,
                passive_mode,
                coloring::ColoringCache {
                    passive_maximum: cached.maximization.as_ref(),
                    zero_round: cached.zero_round.as_ref().or(preflight.as_ref()),
                    bounds: saved_bounds,
                    results: cached
                        .coloring
                        .iter()
                        .filter(|entry| entry.kind == kind)
                        .map(|entry| &entry.result)
                        .collect(),
                },
                computation,
                &mut |bounds| {
                    let previous = updates
                        .coloring_bounds
                        .iter()
                        .find(|entry| entry.kind == kind)
                        .map(|entry| &entry.bounds)
                        .or(saved_bounds);
                    if (bounds.lower.is_some() || bounds.upper.is_some())
                        && previous.is_none_or(|previous| {
                            previous.lower != bounds.lower || previous.upper != bounds.upper
                        })
                    {
                        let mut proved = bounds.clone();
                        proved.testing = None;
                        updates.coloring_bounds.retain(|entry| entry.kind != kind);
                        updates.coloring_bounds.push(CachedColoringBounds {
                            kind,
                            bounds: proved,
                        });
                        cache_update(updates.clone());
                    }
                    partial(Response::ColoringBounds(bounds));
                },
            )?;
            publish_coloring_bounds(&result, minimum, partial);
            if cached.zero_round.is_none()
                && updates.zero_round.is_none()
                && coloring::excludes_zero_round(&result)
            {
                updates.zero_round = Some(zero_round::ZeroRoundResult::Unsolvable);
            }
            if matches!(
                result,
                coloring::ColoringResult::Maximum { .. }
                    | coloring::ColoringResult::Unbounded { .. }
                    | coloring::ColoringResult::NoPalette
            ) {
                updates.coloring_bounds.retain(|entry| entry.kind != kind);
            }
            updates.coloring.push(CachedColoring {
                kind,
                search,
                result: result.clone(),
            });
            cache_update(updates.clone());
            Ok(Response::Coloring(result))
        }
    }
}

fn publish_coloring_bounds(
    result: &coloring::ColoringResult,
    minimum: u32,
    partial: &mut dyn FnMut(Response),
) {
    use coloring::ColoringResult;
    let (lower, upper, witness) = match result {
        ColoringResult::Maximum { colors, witness } => {
            (Some(*colors), Some(*colors), Some(witness.clone()))
        }
        ColoringResult::NoPalette => (None, Some(minimum - 1), None),
        _ => return,
    };
    partial(Response::ColoringBounds(coloring::ColoringBounds {
        minimum,
        testing: None,
        lower,
        upper,
        witness,
    }));
}

/// Parse the transport envelope and run the same dispatch as direct Rust calls.
/// Adapters supply cancellation and a progress consumer, without request maps.
pub fn execute_text(request: &str, computation: &mut Computation<'_>) -> ResponsePacket {
    match serde_json::from_str::<Request>(request) {
        Ok(request) => execute_with(request, computation),
        Err(error) => Response::Error(ApiError {
            message: format!("Invalid request: {error}"),
            location: None,
        })
        .into(),
    }
}

/// Stream generic progress and partial results through either transport.
/// The final response is returned separately so adapters retain stream ownership.
pub fn execute_text_with_events(
    request: &str,
    cancellation: Cancellation,
    on_event: &mut dyn FnMut(ComputationEvent),
) -> ResponsePacket {
    let request = match serde_json::from_str::<Request>(request) {
        Ok(request) => request,
        Err(error) => {
            return Response::Error(ApiError {
                message: format!("Invalid request: {error}"),
                location: None,
            })
            .into();
        }
    };
    // Both callbacks synchronously borrow the same event consumer. No callback
    // retains the borrow while computation or another callback is running.
    let events = std::cell::RefCell::new(on_event);
    let mut progress = |progress| (events.borrow_mut())(ComputationEvent::Progress(progress));
    let mut partial =
        |response| (events.borrow_mut())(ComputationEvent::PartialResult(Box::new(response)));
    let mut cache = |cache| (events.borrow_mut())(ComputationEvent::CacheUpdate(Box::new(cache)));
    execute_reporting(
        request,
        &mut Computation::new(cancellation, &mut progress),
        &mut partial,
        &mut cache,
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

#[cfg(test)]
mod state_tests;
