use wasm_bindgen::prelude::wasm_bindgen;

/// Keep the wasm boundary as thin as the server boundary.
#[wasm_bindgen]
pub fn execute_json(request: &str) -> String {
    round_eliminator_3::execute_json(request)
}
