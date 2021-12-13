    

let dontgc = [];

function request_wasm(req, onresult, oncomplete) {
    let t0 = performance.now();

    var w = new Worker("worker.js");
    w.onerror = function() {
        console.log('There is an error with the worker!');
    }

    w.onmessage = function (s){
        let r = s.data;
        let o = JSON.parse(r);

        if( o == "WASM_READY"){
            let r = JSON.stringify(req);
            w.postMessage(r);
        } else if( o == "Pong" ){

        } else if( o != "Done" ){
            onresult(o);
        } else {
            let t1 = performance.now();
            console.log("Computation took "+(t1-t0)+" ms.");
            oncomplete();
            w.terminate();
            dontgc.splice(dontgc.indexOf(w),1);
        }
    }
    let terminate = function(){
        console.log("terminating worker!");
        w.terminate();
        dontgc.splice(dontgc.indexOf(w),1);
    }

    dontgc.push(w);

    return terminate;
}