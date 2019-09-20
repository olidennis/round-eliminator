
onmessage = function(e) {
    console.log("Received message.");
    self.importScripts("wasm.js");
    console.log("Loaded wasm.");

    let r = e.data;
    let f = function(x){
        postMessage(x);
    }
    wasm_bindgen.init("wasm_bg.wasm").then(function(api){
        console.log("really loaded wasm");
        wasm_bindgen.request_json(r,f);
    });
};
