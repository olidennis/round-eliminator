use wasm_bindgen::prelude::wasm_bindgen;

/// Keep the wasm boundary as thin as the server boundary.
#[wasm_bindgen]
pub fn execute_json(request: &str) -> String {
    round_eliminator_3::execute_json(request)
}

/// Progress callbacks run synchronously inside the worker. The browser cancels
/// the whole computation by terminating that worker, including wasm execution.
#[wasm_bindgen]
pub fn execute_with_progress(request: &str, on_progress: &js_sys::Function) -> String {
    use round_eliminator_3::{
        api::execute_text,
        computation::{Cancellation, Computation},
    };
    let mut report = |progress| {
        let json = serde_json::to_string(&progress).expect("progress must serialize");
        let _ = on_progress.call1(&wasm_bindgen::JsValue::NULL, &json.into());
    };
    let response = execute_text(
        request,
        &mut Computation::new(Cancellation::default(), &mut report),
    );
    serde_json::to_string(&response).expect("response must serialize")
}

/// Deliver the same generic event envelope used by the native SSE transport,
/// including proved partial results while the search continues.
#[wasm_bindgen]
pub fn execute_with_events(request: &str, on_event: &js_sys::Function) -> String {
    let mut report = |event| {
        let json = serde_json::to_string(&event).expect("event must serialize");
        let _ = on_event.call1(&wasm_bindgen::JsValue::NULL, &json.into());
    };
    let response = round_eliminator_3::api::execute_text_with_events(
        request,
        round_eliminator_3::computation::Cancellation::default(),
        &mut report,
    );
    serde_json::to_string(&response).expect("response must serialize")
}
