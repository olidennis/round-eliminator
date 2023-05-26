importScripts('./pkg/round_eliminator_wasm.js');


async function init_wasm_in_worker() {
    await wasm_bindgen('./pkg/round_eliminator_wasm_bg.wasm');

    self.onmessage = function(event) {
        let r = event.data;
        let f = function(x){
            self.postMessage(x);
        }

        wasm_bindgen.request_json(r,f);
       
    };

    self.postMessage(JSON.stringify("WASM_READY"));
};

init_wasm_in_worker();
/*
import init, {request_json, initThreadPool} from './pkg/round_eliminator_wasm.js';


async function init_wasm_in_worker() {
    await init();
    await initThreadPool(navigator.hardwareConcurrency);

    self.onmessage = function(event) {
        let r = event.data;
        let f = function(x){
            self.postMessage(x);
        }
        request_json(r,f);
       
    };

    self.postMessage(JSON.stringify("WASM_READY"));
};

init_wasm_in_worker();
*/