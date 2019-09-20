
onmessage = function(e) {
    self.importScripts("wasm.js");

    let r = e.data;
    let f = function(x){
        postMessage(x);
    }
    wasm_bindgen.init("wasm_bg.wasm").then(function(api){
        wasm_bindgen.request_json(r,f);
    });
};
