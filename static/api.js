
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

    a.onopen = function() {
        a.send(r);
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
            a.close();
        }
    }

}

$.getScript("wasm.js",function(){
    wasm_bindgen.init("wasm_bg.wasm").then(function(x){
    });
});

function request_wasm(req, onresult, oncomplete, worker) {
    let r = JSON.stringify(req);

    if( !worker ){
        let f = function(m){
            let o = JSON.parse(m);
            if( o == "Pong" ){
            }else if( o != "Done" ){
                onresult(o);
            } else {
                oncomplete();
            }
        }
        wasm_bindgen.request_json(r,f);
        return function(){};
    }

    var w = new Worker("worker.js");
    w.onerror = function() {
        console.log('There is an error with the worker!');
      }

    w.postMessage(r);

    w.onmessage = function (s){
        let r = s.data;
        let o = JSON.parse(r);
        if( o == "Pong" ){
        }else if( o != "Done" ){
            onresult(o);
        } else {
            oncomplete();
            w.terminate();
        }
    }
    let terminate = function(){
        console.log("terminating worker!");
        w.terminate();
    }
    return terminate;
}

function request(req, onresult, oncomplete, worker) {
    return request_wasm(req,onresult,oncomplete,worker);
}


export function api_new_problem(s1,s2, ready) {
    return request({ NewProblem : [s1,s2] }, ready , function(){} , true);
}

export function api_speedup(p, ready){
    return request({ Speedup : p }, ready , function(){} , true);
}

export function api_possible_simplifications(p, ready){
    return request({ PossibleSimplifications : p }, function(r){ready(r.S)} , function(){} , false);
}

export function api_simplify(p, s, ready){
    return request({ Simplify : [p,s] }, function(r){ready(r.P)} , function(){} , true);
}

export function api_harden(p, h, ready){
    return request({ Harden : [p,h] }, ready , function(){} , true);
}

export function api_rename(p, v, ready){
    return request({ Rename : [p,v] }, ready , function(){} , true);
}

export function api_autolb(p,label,iter, result, end) {
    return request({ AutoLb : [p,label,iter] }, result , end ,true );
}

export function api_autoub(p,label,iter, result, end) {
    return request({ AutoUb : [p,label,iter] }, result , end ,true);
}

