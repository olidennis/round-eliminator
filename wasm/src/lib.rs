mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn request_json(req : &str, f: &js_sys::Function ){
    simulation::request_json(req,|s|{
        let this = JsValue::NULL;
        let s = JsValue::from(s);
        let _ = f.call1(&this, &s);
    });
}