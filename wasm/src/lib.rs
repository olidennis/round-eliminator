mod utils;

use wasm_bindgen::prelude::*;
use log::Level;
use console_log;

#[wasm_bindgen]
pub fn request_json(req : &str, f: &js_sys::Function ){
    simulation::request_json(req,|s|{
        let this = JsValue::NULL;
        let s = JsValue::from(s);
        let _ = f.call1(&this, &s);
    });
}




#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    utils::set_panic_hook();
    console_log::init_with_level(Level::Trace).expect("error initializing log");
    Ok(())
}