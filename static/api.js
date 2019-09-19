function request(req, onresult, oncomplete) {
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
        a.send( JSON.stringify( "Ping" ) );
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
            //console.log("received pong!");
        }else if( o != "Done" ){
            onresult(o);
        } else {
            //oncomplete();
            a.close();
        }
    }

}

function api_new_problem(s1,s2, ready) {
    request({ NewProblem : [s1,s2] }, ready , function(){} );
}

function api_speedup(p, ready){
    request({ Speedup : p }, ready , function(){} );
}

function api_possible_simplifications(p, ready){
    request({ PossibleSimplifications : p }, function(r){ready(r.S)} , function(){} );
}

function api_simplify(p, s, ready){
    request({ Simplify : [p,s] }, function(r){ready(r.P)} , function(){} );
}

function api_harden(p, h, ready){
    request({ Harden : [p,h] }, ready , function(){} );
}

function api_rename(p, v, ready){
    request({ Rename : [p,v] }, ready , function(){} );
}

function api_autolb(p,label,iter, result, end) {
    request({ AutoLb : [p,label,iter] }, result , end );
}

function api_autoub(p,label,iter, result, end) {
    request({ AutoUb : [p,label,iter] }, result , end );
}

