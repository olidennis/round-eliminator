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
        computation::{Cancellation, Computation},
        protocol::execute_text,
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
