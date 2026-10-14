use round_eliminator_3::{
    api::{ComputationEvent, execute_text_with_events},
    computation::{Cancellation, Progress},
};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    // LLM: Monotonic in browsers and Node; callbacks run inside a synchronous worker,
    // LLM: so a JavaScript timer cannot throttle while wasm is still computing.
    /// LLM: Read the monotonic JavaScript performance clock. Used inside synchronous wasm
    /// LLM: execution to throttle callbacks without depending on JavaScript timers.
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn now() -> f64;
}

/// LLM: Execute a JSON request and serialize its final packet. Used by the nonstreaming
/// LLM: transport boundary and simple direct clients.
#[wasm_bindgen]
pub fn execute_json(request: &str) -> String {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    round_eliminator_3::execute_json(request)
}

/// LLM: Execute a request with throttled synchronous JavaScript progress callbacks. Used by
/// LLM: wasm clients needing progress only; browser cancellation terminates the worker.
#[wasm_bindgen]
pub fn execute_with_progress(request: &str, on_progress: &js_sys::Function) -> String {
    // LLM: Send the request through the shared dispatch while retaining proof/cache updates.
    execute_reporting(request, &mut |event| {
        if let ComputationEvent::Progress(progress) = event {
            let json = serde_json::to_string(&progress).expect("progress must serialize");
            let _ = on_progress.call1(&wasm_bindgen::JsValue::NULL, &json.into());
        }
    })
}

/// LLM: Execute a request with the same throttled event envelope as the native stream. Used by
/// LLM: the wasm worker to deliver partial proofs and cache updates as well as progress.
#[wasm_bindgen]
pub fn execute_with_events(request: &str, on_event: &js_sys::Function) -> String {
    // LLM: Send the request through the shared dispatch while retaining proof/cache updates.
    execute_reporting(request, &mut |event| {
        let json = serde_json::to_string(&event).expect("event must serialize");
        let _ = on_event.call1(&wasm_bindgen::JsValue::NULL, &json.into());
    })
}

/// LLM: Run the shared JSON dispatch while coalescing worker events, flush the latest proofs,
/// LLM: and serialize the final packet. Used by both wasm callback entry points.
fn execute_reporting(request: &str, emit: &mut dyn FnMut(ComputationEvent)) -> String {
    // LLM: Coalesce worker updates before JSON serialization and JavaScript callbacks,
    // LLM: preserving independent proof/cache slots.
    let mut updates = Updates::default();
    let response = execute_text_with_events(request, Cancellation::default(), &mut |event| {
        updates.push(event, now(), emit);
    });
    // LLM: Preserve the last counter and proof before the separately returned result.

    updates.flush(emit);
    serde_json::to_string(&response).expect("response must serialize")
}

/// LLM: The wasm adapter's latest progress, cache, and partial-proof slots with their last
/// LLM: delivery time. Superseded updates are dropped before JSON/JavaScript work; final
/// LLM: flushing preserves the latest proofs.
#[derive(Default)]
struct Updates {
    last_sent: Option<f64>,
    last_progress: Option<Progress>,
    progress: Option<Progress>,
    partial: Option<ComputationEvent>,
    cache: Option<ComputationEvent>,
}

impl Updates {
    /// LLM: Replace the appropriate latest-event slot and flush only when the monotonic
    /// LLM: delivery interval has elapsed. Used by the wasm computation callback before
    /// LLM: serialization.
    fn push(&mut self, event: ComputationEvent, now: f64, emit: &mut dyn FnMut(ComputationEvent)) {
        // LLM: Replace progress, cache, or partial proof independently so a newer event of one
        // LLM: kind cannot erase another kind.
        match event {
            ComputationEvent::Progress(progress) => self.progress = Some(progress),
            event @ ComputationEvent::CacheUpdate(_) => self.cache = Some(event),
            other => self.partial = Some(other),
        }
        // LLM: Deliver retained updates before returning, preserving the latest proof even
        // LLM: after a burst.
        if self
            .last_sent
            .is_none_or(|previous| now - previous >= 100.0)
        {
            self.last_sent = Some(now);
            self.flush(emit);
        }
    }

    /// LLM: Deliver retained progress, cache, and partial-proof slots, suppressing unchanged
    /// LLM: progress. Used on timed wasm ticks and immediately before returning the final
    /// LLM: packet.
    fn flush(&mut self, emit: &mut dyn FnMut(ComputationEvent)) {
        // LLM: Suppress identical progress counters but still deliver retained cache and proof
        // LLM: snapshots.
        if let Some(progress) = self.progress.take()
            && self.last_progress != Some(progress)
        {
            self.last_progress = Some(progress);
            emit(ComputationEvent::Progress(progress));
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if let Some(cache) = self.cache.take() {
            emit(cache);
        }
        if let Some(partial) = self.partial.take() {
            emit(partial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use round_eliminator_3::{
        algorithms::solvability::coloring::ColoringBounds, computation::Phase,
    };

    /// LLM: Construct a progress event for a requested iteration count. Used by wasm transport
    /// LLM: coalescing fixtures.
    fn progress(iterations: u32) -> ComputationEvent {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        ComputationEvent::Progress(Progress {
            phase: Phase::Solving,
            iterations,
            work: None,
        })
    }

    /// LLM: Construct a partial lower-bound proof event. Used by transport coalescing fixtures
    /// LLM: to distinguish proofs from progress.
    fn proof(lower: u32) -> ComputationEvent {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        ComputationEvent::PartialResult(Box::new(
            round_eliminator_3::api::Response::ColoringBounds(ColoringBounds {
                minimum: 1,
                lower: Some(lower),
                upper: None,
                testing: None,
                witness: None,
            }),
        ))
    }

    /// LLM: Regression test: verify that burst is coalesced before callbacks and final flush
    /// LLM: retains latest proof. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn burst_is_coalesced_before_callbacks_and_final_flush_retains_latest_proof() {
        // LLM: Prepare `updates` for the following computation; it stores the updates for this
        // LLM: stage.
        let mut updates = Updates::default();
        let mut events = Vec::new();
        let mut emit = |event| events.push(event);
        for i in 0..100_000 {
            updates.push(progress(i), 0.0, &mut emit);
        }
        updates.push(proof(3), 0.0, &mut emit);
        updates.push(proof(4), 0.0, &mut emit);
        // LLM: Deliver retained updates before returning, preserving the latest proof even
        // LLM: after a burst.
        updates.flush(&mut emit);
        assert_eq!(events.len(), 3); // LLM: First update, last update, best proof.
        assert!(matches!(
            events[1],
            ComputationEvent::Progress(Progress {
                iterations: 99_999,
                ..
            })
        ));
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            matches!(&events[2], ComputationEvent::PartialResult(response)
            if matches!(**response, round_eliminator_3::api::Response::ColoringBounds(ColoringBounds { lower: Some(4), .. })))
        );
    }

    /// LLM: Regression test: verify that cache updates and partial proofs are coalesced
    /// LLM: independently. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn cache_updates_and_partial_proofs_are_coalesced_independently() {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use round_eliminator_3::{
            algorithms::solvability::zero_round::ZeroRoundResult, api::AnalysisCache,
        };
        let mut updates = Updates::default();
        let mut events = Vec::new();
        updates.push(progress(0), 0.0, &mut |event| events.push(event));
        let mut cache = AnalysisCache::default();
        cache.zero_round = Some(ZeroRoundResult::Unsolvable);
        // LLM: Retain the newly constructed choices in the result for this stage.
        updates.push(
            ComputationEvent::CacheUpdate(Box::new(cache.clone())),
            1.0,
            &mut |event| events.push(event),
        );
        updates.push(proof(3), 2.0, &mut |event| events.push(event));
        // LLM: Update the current working state before continuing this operation.
        cache.maximization = Some(
            round_eliminator_3::algorithms::maximize::MaximizationResult::OutputOnly(
                round_eliminator_3::model::Constraint::new(Vec::new()),
            ),
        );
        // LLM: Retain the newly constructed choices in the result for this stage.
        updates.push(
            ComputationEvent::CacheUpdate(Box::new(cache.clone())),
            3.0,
            &mut |event| events.push(event),
        );
        updates.push(proof(4), 4.0, &mut |event| events.push(event));
        updates.flush(&mut |event| events.push(event));
        assert_eq!(events.len(), 3);
        // LLM: Compare the implementation result with the independently prepared reference for
        // LLM: this fixture.
        assert!(
            matches!(&events[1], ComputationEvent::CacheUpdate(actual) if actual.as_ref() == &cache)
        );
        assert!(
            matches!(&events[2], ComputationEvent::PartialResult(response)
            if matches!(response.as_ref(), round_eliminator_3::api::Response::ColoringBounds(bounds) if bounds.lower == Some(4)))
        );
    }

    /// LLM: Regression test: verify that delivery is limited to ten ticks per second even
    /// LLM: across phase changes. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn delivery_is_limited_to_ten_ticks_per_second_even_across_phase_changes() {
        // LLM: Prepare `updates` for the following computation; it stores the updates for this
        // LLM: stage.
        let mut updates = Updates::default();
        let mut events = Vec::new();
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for i in 0..1000 {
            // LLM: Prepare `event` for the following computation; it stores the event for this
            // LLM: stage.
            let mut event = progress(i);
            if i % 2 == 0 {
                event = ComputationEvent::Progress(Progress {
                    phase: Phase::Verifying,
                    iterations: i,
                    work: None,
                });
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            updates.push(event, f64::from(i), &mut |event| events.push(event));
        }
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(events.len(), 10);
        updates.flush(&mut |event| events.push(event));
        assert_eq!(events.len(), 11); // LLM: Final flush is immediate, outside the rate limit.
    }
}
