function request(req, onresult, oncomplete) {
    let a = new WebSocket("ws://" + location.host + "/api");
    let r = JSON.stringify(req);
    a.onopen = function() {
        a.send(r);
    }
    a.onmessage = function(s){
        let m = s.data;
        let o = JSON.parse(m);
        if( o != "Done" ){
            onresult(o);
        } else {
            oncomplete();
            a.close();
        }
    }
}

function api_new_problem(s1,s2, ready) {
    request({ NewProblem : [s1,s2] }, function(r){ready(r.P)} , function(){} );
}

function api_speedup(p, ready){
    request({ Speedup : p }, function(r){ready(r.P)} , function(){} );
}

function api_possible_simplifications(p, ready){
    request({ PossibleSimplifications : p }, function(r){ready(r.S)} , function(){} );
}

function api_simplify(p, s, ready){
    request({ Simplify : [p,s] }, function(r){ready(r.P)} , function(){} );
}

function api_harden(p, h, ready){
    request({ Harden : [p,h] }, function(r){ready(r.OP)} , function(){} );
}

function api_rename(p, v, ready){
    request({ Rename : [p,v] }, function(r){ready(r.P)} , function(){} );
}

function api_autolb(p,label,iter, result, end) {
    request({ AutoLb : [p,label,iter] }, function(r){result(r.L)} , end );
}

function api_autoub(p,label,iter, result, end) {
    request({ AutoUb : [p,label,iter] }, function(r){result(r.U)} , end );
}

