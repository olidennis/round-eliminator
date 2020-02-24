
function request_server(req, onresult, oncomplete, worker) {
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

let use_wasm = !window.location.href.includes("server");


if( use_wasm ){ 
    $.getScript("wasm.js",function(){
        wasm_bindgen.init("wasm_bg.wasm").then(function(x){
        });
    });
}

function request_wasm(req, onresult, oncomplete, worker) {
    let r = JSON.stringify(req);
    let t0 = performance.now();

    if( !worker ){
        let f = function(m){
            let o = JSON.parse(m);
            if( o == "Pong" ){
            }else if( o != "Done" ){
                onresult(o);
            } else {
                let t1 = performance.now();
                console.log("Computation took "+(t1-t0)+" ms.");
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
            let t1 = performance.now();
            console.log("Computation took "+(t1-t0)+" ms.");
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
    if( use_wasm ){
        return request_wasm(req,onresult,oncomplete,worker);
    } else {
        return request_server(req,onresult,oncomplete,worker);
    }
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

export function api_possible_addarrow(p, ready){
    return request({ PossibleAddarrow : p }, function(r){ready(r.S)} , function(){} , false);
}

export function api_simplify(p, s, ready){
    return request({ Simplify : [p,s] }, function(r){ready(r.P)} , function(){} , true);
}

export function api_simplify_s(p, s, ready){
    return request({ SimplifyS : [p,s] }, function(r){ready(r.P)} , function(){} , true);
}

export function api_addarrow(p, s, ready){
    return request({ Addarrow : [p,s] }, function(r){ready(r.P)} , function(){} , true);
}

export function api_harden(p, h, usepred, ready){
    return request({ Harden : [p,h,usepred] }, ready , function(){} , true);
}

export function api_rename(p, v, ready){
    return request({ Rename : [p,v] }, ready , function(){} , true);
}

export function api_autolb(p,label,iter, col, rcs, useunreach, usediag, useaddarrow, useindirect, result, end) {
    return request({ AutoLb : [p,label,iter,col,rcs, useunreach, usediag, useaddarrow, useindirect] }, result , end ,true );
}

export function api_autoub(p,label,iter,col, rcs, usepred, usedet, result, end) {
    return request({ AutoUb : [p,label,iter,col, rcs, usepred,usedet] }, result , end ,true);
}

