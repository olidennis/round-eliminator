    

let dontgc = [];

function request_wasm(req, onresult, oncomplete) {
    let t0 = performance.now();

    var w = new Worker("worker.js"/*,{type : "module"}*/);
    w.onerror = function(e) {
        console.log('There is an error with the worker!');
        console.log(e);
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
    let terminated = false;
    let opened = false;

    let ping = setInterval(function(){
        if( terminated ){
            clearInterval(ping);
            return;
        }
        if( a.readyState == WebSocket.OPEN ){
            a.send( JSON.stringify( "Ping" ) );
        } else if( a.readyState != WebSocket.CONNECTING ){
            clearInterval(ping);
        }
    },5000);

    let t0;
    a.onopen = function() {
        opened = true;
        a.send(r);
        t0 = performance.now();
    }
    a.onerror = function(e) {
        //onclose will be called even if onerror is called
        //alert("Something bad happened.");
        //oncomplete();
    }
    a.onclose = function(e){
        if( !e.wasClean && !terminated )alert("Something bad happened.");
        oncomplete();
    }
    a.onmessage = function(s){
        if( terminated )return;
        let m = s.data;
        let o = JSON.parse(m);
        //console.log(o);
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

    let terminate = function(){
        console.log("terminating server thread!");
        if(opened)a.send(JSON.stringify("STOP"));
        a.close();
        terminated = true;
    }
    return terminate;
}


let use_wasm = !window.location.href.includes("server");

export function request(req, onresult, oncomplete) {
    if( use_wasm ){
        //console.log("wasm request");
        return request_wasm(req, onresult, oncomplete);
    } else {
        //console.log("server request");
        return request_server(req, onresult, oncomplete);
    }
}