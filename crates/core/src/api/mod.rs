//! LLM: Transport-independent API. The server and wasm adapter use this same dispatch.

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

/// LLM: Execute a structured request without progress/event consumers. Used by direct Rust
/// LLM: callers that need only the final response and cache delta.
pub fn execute(request: Request) -> ResponsePacket {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    execute_with(
        request,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

/// LLM: Execute a structured request with progress and cancellation but no partial-event
/// LLM: consumer. Used by direct callers and the text-envelope wrapper.
pub fn execute_with(request: Request, computation: &mut Computation<'_>) -> ResponsePacket {
    // LLM: Send the request through the shared dispatch while retaining proof/cache updates.
    execute_reporting(request, computation, &mut |_| {}, &mut |_| {})
}

/// LLM: Execute parsing or one computation and classify completion, errors, or cancellation
/// LLM: while retaining new source cache entries. Used by structured and streaming API
/// LLM: wrappers.
fn execute_reporting(
    request: Request,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
    cache_update: &mut dyn FnMut(AnalysisCache),
) -> ResponsePacket {
    // LLM: Retain reusable artifacts or dominators so proven work need not be repeated.
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
    // LLM: Keep the final packet separate from coalesced intermediate updates.
    let response = if computation.cancellation.is_cancelled() {
        Response::Cancelled
    } else {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
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
    // LLM: Update the current working state before continuing this operation.
    ResponsePacket { response, cache }
}

/// LLM: Select applicable cached proofs and dispatch one requested analysis or transformation.
/// LLM: Shared by server and wasm; new source artifacts are published even if later work is
/// LLM: interrupted.
fn execute_operation(
    state: &ProblemState,
    operation: Operation,
    computation: &mut Computation<'_>,
    partial: &mut dyn FnMut(Response),
    updates: &mut AnalysisCache,
    cache_update: &mut dyn FnMut(AnalysisCache),
) -> anyhow::Result<Response> {
    // LLM: Borrow the submitted snapshot and its source cache; all artifact IDs belong to this
    // LLM: problem.
    let problem = &state.problem;
    let cached = &state.analyses;
    computation.cancellation.check()?;
    // LLM: Combine completed same-kind proofs before starting another numerical search.
    match operation {
        Operation::BasicFixedPoint => Ok(Response::Problem(fixed_point::basic_with(
            problem,
            computation,
        )?)),
        Operation::Maximize | Operation::ApplyRe => {
            // LLM: Reuse the complete source maximum or compute and publish one before any
            // LLM: further RE transformation.
            let computed;
            let maximum = if let Some(maximum) = &cached.maximization {
                maximum
            } else {
                computed = maximize::compute_with(problem, computation)?;
                updates.maximization = Some(computed);
                cache_update(updates.clone());
                updates.maximization.as_ref().expect("completed maximum")
            };
            // LLM: Apply RE only for the transformation request; otherwise return the source
            // LLM: maximum itself.
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
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if let Some(result) = cached.diagram(options) {
                return Ok(Response::Diagram(result));
            }
            // LLM: Collect the computed values, preserving their association with the
            // LLM: requested work items.
            let result = diagram::compute_with_maximum(
                problem,
                options,
                cached.maximization.as_ref(),
                computation,
            )?;
            updates.diagrams.push(result.clone());
            cache_update(updates.clone());
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(Response::Diagram(result))
        }
        Operation::CheckZeroRound { passive_mode } => {
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if let Some(result) = &cached.zero_round {
                return Ok(Response::ZeroRound(result.clone()));
            }
            // LLM: Collect the computed values, preserving their association with the
            // LLM: requested work items.
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
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                zero_round::check_with_maximum(
                    problem,
                    passive_mode,
                    cached.maximization.as_ref(),
                    computation,
                )?
            };
            // LLM: Update the current working state before continuing this operation.
            updates.zero_round = Some(result.clone());
            cache_update(updates.clone());
            Ok(Response::ZeroRound(result))
        }
        Operation::CheckColoring {
            kind,
            search,
            passive_mode,
        } => {
            // LLM: Establish the feasible bound for this local search before exploring or
            // LLM: allocating alternatives.
            let minimum = coloring::minimum(problem, kind)?;
            if let coloring::PaletteSearch::Fixed { colors } = search {
                anyhow::ensure!(
                    colors >= minimum,
                    "This coloring requires at least {minimum} colors locally."
                );
            }
            // LLM: Exact queries first, then valid palette restriction/bound proofs.

            let previous = cached.coloring.iter().filter(|entry| entry.kind == kind);
            let previous = previous
                .clone()
                .filter(|entry| entry.search == search)
                .chain(previous.filter(|entry| entry.search != search));
            // LLM: Try deriving this query from a previous completed same-kind proof.
            for entry in previous {
                // LLM: Try deriving this query from a previous completed same-kind proof.
                if let Some(result) =
                    coloring::reuse_result(search, &entry.result, &computation.cancellation)?
                {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    publish_coloring_bounds(&result, minimum, partial);
                    if entry.search != search {
                        // LLM: Retain the newly constructed choices in the result for this
                        // LLM: stage.
                        updates.coloring.push(CachedColoring {
                            kind,
                            search,
                            result: result.clone(),
                        });
                        cache_update(updates.clone());
                    }
                    // LLM: Return the result of this completed search or conversion to its
                    // LLM: caller.
                    return Ok(Response::Coloring(result));
                }
            }
            // LLM: Combine completed same-kind proofs before starting another numerical
            // LLM: search.
            let mut bounds = coloring::bounds_from_results(
                minimum,
                cached
                    .coloring
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .map(|entry| &entry.result),
                &computation.cancellation,
            )?;
            // LLM: Select proved bounds only for this coloring kind; other kinds cannot
            // LLM: initialize its numerical interval.
            let saved_bounds = cached
                .coloring_bounds
                .iter()
                .find(|entry| entry.kind == kind)
                .map(|entry| &entry.bounds);
            if let Some(saved) = saved_bounds {
                coloring::merge_bounds(&mut bounds, saved);
            }
            // LLM: Return early only when the proved interval already determines the answer.
            if search == coloring::PaletteSearch::Maximum
                && let Some(result) = coloring::maximum_from_bounds(&bounds)
            {
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                publish_coloring_bounds(&result, minimum, partial);
                updates.coloring.push(CachedColoring {
                    kind,
                    search,
                    result: result.clone(),
                });
                cache_update(updates.clone());
                return Ok(Response::Coloring(result));
            }
            // LLM: This preflight is already part of ordinary maximum search. Retain
            // LLM: its completed proof, including when the subsequent search is cancelled.

            if kind != coloring::ColoringKind::DistanceTwo
                && search == coloring::PaletteSearch::Maximum
                && cached.zero_round.is_none()
                && bounds.upper.is_none()
            {
                // LLM: Update the current working state before continuing this operation.
                updates.zero_round = Some(
                    if cached
                        .coloring
                        .iter()
                        .any(|entry| coloring::excludes_zero_round(&entry.result))
                    {
                        zero_round::ZeroRoundResult::Unsolvable
                    } else {
                        // LLM: Delegate to the common implementation so this entry point
                        // LLM: shares its semantics and checks.
                        zero_round::check_with_maximum(
                            problem,
                            passive_mode,
                            cached.maximization.as_ref(),
                            computation,
                        )?
                    },
                );
                // LLM: Delegate to the common implementation so this entry point shares its
                // LLM: semantics and checks.
                cache_update(updates.clone());
            }
            // LLM: Prepare `preflight` for the following computation; it stores the preflight
            // LLM: for this stage.
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
                    // LLM: Record prior work so the next pass need only visit newly relevant
                    // LLM: parent pairs.
                    let previous = updates
                        .coloring_bounds
                        .iter()
                        .find(|entry| entry.kind == kind)
                        .map(|entry| &entry.bounds)
                        .or(saved_bounds);
                    // LLM: Handle this condition before continuing with the remaining search
                    // LLM: alternatives.
                    if (bounds.lower.is_some() || bounds.upper.is_some())
                        && previous.is_none_or(|previous| {
                            previous.lower != bounds.lower || previous.upper != bounds.upper
                        })
                    {
                        // LLM: Publish mathematical bounds with no in-flight palette, so
                        // LLM: interruption can safely preserve them.
                        let mut proved = bounds.clone();
                        proved.testing = None;
                        updates.coloring_bounds.retain(|entry| entry.kind != kind);
                        updates.coloring_bounds.push(CachedColoringBounds {
                            kind,
                            bounds: proved,
                        });
                        cache_update(updates.clone());
                    }
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
                    partial(Response::ColoringBounds(bounds));
                },
            )?;
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            publish_coloring_bounds(&result, minimum, partial);
            if cached.zero_round.is_none()
                && updates.zero_round.is_none()
                && coloring::excludes_zero_round(&result)
            {
                updates.zero_round = Some(zero_round::ZeroRoundResult::Unsolvable);
            }
            // LLM: Remove redundant partial intervals when a complete maximum, unbounded
            // LLM: result, or no-palette proof supersedes them.
            if matches!(
                result,
                coloring::ColoringResult::Maximum { .. }
                    | coloring::ColoringResult::Unbounded { .. }
                    | coloring::ColoringResult::NoPalette
            ) {
                updates.coloring_bounds.retain(|entry| entry.kind != kind);
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
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

/// LLM: Publish settled maximum/no-palette bounds from a completed result. Used by the API
/// LLM: when answering either from caches or a new coloring computation.
fn publish_coloring_bounds(
    result: &coloring::ColoringResult,
    minimum: u32,
    partial: &mut dyn FnMut(Response),
) {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use coloring::ColoringResult;
    let (lower, upper, witness) = match result {
        ColoringResult::Maximum { colors, witness } => {
            (Some(*colors), Some(*colors), Some(witness.clone()))
        }
        ColoringResult::NoPalette => (None, Some(minimum - 1), None),
        _ => return,
    };
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    partial(Response::ColoringBounds(coloring::ColoringBounds {
        minimum,
        testing: None,
        lower,
        upper,
        witness,
    }));
}

/// LLM: Deserialize the shared request envelope and run the structured dispatch with the
/// LLM: supplied computation context. Used by adapters or callers that do not stream partial
/// LLM: proofs.
pub fn execute_text(request: &str, computation: &mut Computation<'_>) -> ResponsePacket {
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    match serde_json::from_str::<Request>(request) {
        Ok(request) => execute_with(request, computation),
        Err(error) => Response::Error(ApiError {
            message: format!("Invalid request: {error}"),
            location: None,
        })
        .into(),
    }
}

/// LLM: Deserialize a request and connect progress, cumulative proofs, and cache callbacks to
/// LLM: one event consumer. Used by both transport adapters; returns the final packet
/// LLM: separately.
pub fn execute_text_with_events(
    request: &str,
    cancellation: Cancellation,
    on_event: &mut dyn FnMut(ComputationEvent),
) -> ResponsePacket {
    // LLM: Prepare `request` for the following computation; it stores the request for this
    // LLM: stage.
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
    // LLM: Both callbacks synchronously borrow the same event consumer. No callback
    // LLM: retains the borrow while computation or another callback is running.

    let events = std::cell::RefCell::new(on_event);
    let mut progress = |progress| (events.borrow_mut())(ComputationEvent::Progress(progress));
    let mut partial =
        |response| (events.borrow_mut())(ComputationEvent::PartialResult(Box::new(response)));
    let mut cache = |cache| (events.borrow_mut())(ComputationEvent::CacheUpdate(Box::new(cache)));
    // LLM: Send the request through the shared dispatch while retaining proof/cache updates.
    execute_reporting(
        request,
        &mut Computation::new(cancellation, &mut progress),
        &mut partial,
        &mut cache,
    )
}

/// LLM: Execute a JSON request and serialize its final packet. Used by the nonstreaming
/// LLM: transport boundary and simple direct clients.
pub fn execute_json(request: &str) -> String {
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
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
