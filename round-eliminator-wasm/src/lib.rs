use log::Level;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn request_json(req: &str, f: &js_sys::Function) {
    round_eliminator_lib::serial::request_json(req, |s| {
        let this = JsValue::NULL;
        let s = JsValue::from(s);
        let _ = f.call1(&this, &s);
    });
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Trace).expect("error initializing log");
    Ok(())
}
