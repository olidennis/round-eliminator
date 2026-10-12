use round_eliminator_3::{
    api::{ComputationEvent, execute_text_with_events},
    computation::{Cancellation, Progress},
};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    // Monotonic in browsers and Node; callbacks run inside a synchronous worker,
    // so a JavaScript timer cannot throttle while wasm is still computing.
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn now() -> f64;
}

/// Keep the wasm boundary as thin as the server boundary.
#[wasm_bindgen]
pub fn execute_json(request: &str) -> String {
    round_eliminator_3::execute_json(request)
}

/// Progress callbacks run synchronously inside the worker. The browser cancels
/// the whole computation by terminating that worker, including wasm execution.
#[wasm_bindgen]
pub fn execute_with_progress(request: &str, on_progress: &js_sys::Function) -> String {
    execute_reporting(request, &mut |event| {
        if let ComputationEvent::Progress(progress) = event {
            let json = serde_json::to_string(&progress).expect("progress must serialize");
            let _ = on_progress.call1(&wasm_bindgen::JsValue::NULL, &json.into());
        }
    })
}

/// Deliver the same generic event envelope used by the native SSE transport,
/// including cumulative proof snapshots while the search continues.
#[wasm_bindgen]
pub fn execute_with_events(request: &str, on_event: &js_sys::Function) -> String {
    execute_reporting(request, &mut |event| {
        let json = serde_json::to_string(&event).expect("event must serialize");
        let _ = on_event.call1(&wasm_bindgen::JsValue::NULL, &json.into());
    })
}

fn execute_reporting(request: &str, emit: &mut dyn FnMut(ComputationEvent)) -> String {
    let mut updates = Updates::default();
    let response = execute_text_with_events(request, Cancellation::default(), &mut |event| {
        updates.push(event, now(), emit);
    });
    // Preserve the last counter and proof before the separately returned result.
    updates.flush(emit);
    serde_json::to_string(&response).expect("response must serialize")
}

/// Retain at most one progress update and one cumulative proof snapshot. Apply
/// the rate limit before JSON serialization and JS callbacks, not only in the UI.
#[derive(Default)]
struct Updates {
    last_sent: Option<f64>,
    last_progress: Option<Progress>,
    progress: Option<Progress>,
    partial: Option<ComputationEvent>,
}

impl Updates {
    fn push(&mut self, event: ComputationEvent, now: f64, emit: &mut dyn FnMut(ComputationEvent)) {
        match event {
            ComputationEvent::Progress(progress) => self.progress = Some(progress),
            other => self.partial = Some(other),
        }
        if self
            .last_sent
            .is_none_or(|previous| now - previous >= 100.0)
        {
            self.last_sent = Some(now);
            self.flush(emit);
        }
    }

    fn flush(&mut self, emit: &mut dyn FnMut(ComputationEvent)) {
        if let Some(progress) = self.progress.take()
            && self.last_progress != Some(progress)
        {
            self.last_progress = Some(progress);
            emit(ComputationEvent::Progress(progress));
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

    fn progress(iterations: u32) -> ComputationEvent {
        ComputationEvent::Progress(Progress {
            phase: Phase::Solving,
            iterations,
        })
    }

    fn proof(lower: u32) -> ComputationEvent {
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

    #[test]
    fn burst_is_coalesced_before_callbacks_and_final_flush_retains_latest_proof() {
        let mut updates = Updates::default();
        let mut events = Vec::new();
        let mut emit = |event| events.push(event);
        for i in 0..100_000 {
            updates.push(progress(i), 0.0, &mut emit);
        }
        updates.push(proof(3), 0.0, &mut emit);
        updates.push(proof(4), 0.0, &mut emit);
        updates.flush(&mut emit);
        assert_eq!(events.len(), 3); // First update, last update, best proof.
        assert!(matches!(
            events[1],
            ComputationEvent::Progress(Progress {
                iterations: 99_999,
                ..
            })
        ));
        assert!(
            matches!(&events[2], ComputationEvent::PartialResult(response)
            if matches!(**response, round_eliminator_3::api::Response::ColoringBounds(ColoringBounds { lower: Some(4), .. })))
        );
    }

    #[test]
    fn delivery_is_limited_to_ten_ticks_per_second_even_across_phase_changes() {
        let mut updates = Updates::default();
        let mut events = Vec::new();
        for i in 0..1000 {
            let mut event = progress(i);
            if i % 2 == 0 {
                event = ComputationEvent::Progress(Progress {
                    phase: Phase::Verifying,
                    iterations: i,
                });
            }
            updates.push(event, f64::from(i), &mut |event| events.push(event));
        }
        assert_eq!(events.len(), 10);
        updates.flush(&mut |event| events.push(event));
        assert_eq!(events.len(), 11); // Final flush is immediate, outside the rate limit.
    }
}
