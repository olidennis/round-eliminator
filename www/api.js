    

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



function request_server(req, onresult, oncomplete) {
    let a = new WebSocket("ws://" + location.host + "/api");
    let r = JSON.stringify(req);

    let ping = setInterval(function(){
        if( a.readyState == WebSocket.OPEN ){
            a.send( JSON.stringify( "Ping" ) );
        } else if( a.readyState != WebSocket.CONNECTING ){
            clearInterval(ping);
        }
    },5000);

    let t0;
    a.onopen = function() {
        a.send(r);
        t0 = performance.now();
    }
    a.onerror = function(e) {
        //onclose will be called even if onerror is called
        //alert("Something bad happened.");
        //oncomplete();
    }
    a.onclose = function(e){
        if( !e.wasClean )alert("Something bad happened.");
        oncomplete();
    }
    a.onmessage = function(s){
        let m = s.data;
        let o = JSON.parse(m);
        if( o == "Pong" ){
        }else if( o != "Done" ){
            onresult(o);
        } else {
            //oncomplete();
            let t1 = performance.now();
            console.log("Computation took "+(t1-t0)+" ms.");
            a.close();
        }
    }
    return function(){
        console.log("not implemented!");
    }
}