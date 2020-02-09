import * as api from "./api.js"

let ctr = 0;

$(document).ready(function(){

    $( "#btn0" ).click(function(ev) {
        on_input_click();
    });

    $( "#btnclear" ).click(function(ev) {
        $( "#steps" ).html("");
    });

});

function escape(s){
    return $('<div/>').text(s).html();
}

function freeid(){
    return ++ctr;
}

function merge(v) {
    return v.join("");
}

function get_labels(problem){
    let x = problem[1];
    let labels = Array.from(new Set(x.left.concat(x.right).flat(Infinity)));
    labels.sort();
    return labels;
}

function get_renaming(problem){
    let x = problem[1];
    if( x.mapping != null ){
        let cur_to_old = {};
        for (let line of x.mapping) {
            let old = merge(line[0]);
            let cur = line[1];
            cur_to_old[cur] = old;
        }
        return cur_to_old;
    } else {
        return null;
    }
}

function make_performed_action(action){
    return $('<div class="card bg-info text-white m-2 p-2"/>').text(action);
}

function performed_initial() {
    return make_performed_action("Initial problem.");
}

function performed_simplification(s){
    return make_performed_action("Performed simplification "+escape(s));
}

function performed_addarrow(s){
    return make_performed_action("Added arrow "+escape(s));
}

function performed_harden(s){
    return make_performed_action("Kept only labels "+escape(s));
}

function performed_harden2(s){
    return make_performed_action("Removed label "+escape(s));
}

function performed_speedup() {
    return make_performed_action("Performed speedup.");
}


function make_div_diagram(problem){
    let x = problem[1];
    let id = freeid();
    let nodes = get_labels(problem);//[...new Set([].concat.apply([], x.diagram))];
    let visnodes = [];
    for (let node of nodes ) {
        visnodes.push({id: node, label: node});
    }
    let visnodes2 = new vis.DataSet(visnodes);
    let visedges = [];
    for (let edge of x.diagram ) {
        visedges.push({ from : edge[0], to : edge[1], arrows: 'to'});
    }
    let visedges2 = new vis.DataSet(visedges);
    let visdata = {
        nodes: visnodes2,
        edges: visedges2
    };

    let diagram = $('<div class="panel-resizable" style="width: 300px; height: 300px;" id="'+id+'" onmouseover="document.body.style.overflow=\'hidden\';"  onmouseout="document.body.style.overflow=\'auto\';"></div>');
    diagram.ready(function(ev){
        let container = document.getElementById(id);
        let network = new vis.Network(document.getElementById(id), visdata, {});
    });

    return diagram;
}

function make_spinner(s){
    return $('<div class="card card-body m-2 bg-light"/>').append($('<div class="spinner-border" role="status"></div>')).append(s);

}

function make_button_speedup(problem) {
    let blob = problem[0];
    let next = $('<button type="button" class="btn btn-primary ml-2">Speedup</button>');
    next.click(function(ev) {
        let spinner = make_spinner("Performing speedup...");
        append_generic(spinner);
        api.api_speedup(blob, function(x){ spinner.remove(); return append_new_problem_or_error(x, performed_speedup() ); } );
    });
    return next;
}

function make_button_edit(problem){
    let x = problem[1];
    let edit = $('<button type="button" class="btn btn-primary ml-2">Edit</button>');
    edit.click(function(ev) {
        let tleft = x.left.map(v => v.map(t => t.join("")).join(" ")).join("\n");
        let tright = x.right.map(v => v.map(t => t.join("")).join(" ")).join("\n");
        console.log(tleft);
        console.log(tright);
        $('#inf1').val(tleft);
        $('#inf2').val(tright);
    });
    return edit;
}

function make_div_simplifications(problem){
    let blob = problem[0];
    let simpls = $('<div/>');
    simpls.append('<p>Available simplifications:</p>');
    api.api_possible_simplifications(blob,function(v){
        for (let simpl of v ){
            let sblob = simpl[0];
            let sstr = simpl[1];
            let bstr = sstr[0] + "→" + sstr[1];
            var bsimpl = $('<button type="button" class="btn btn-primary m-2">'+escape(bstr)+'</button>');
            bsimpl.click(function(ev) {
                api.api_simplify(blob, sblob, function(x){return append_new_problem(x, performed_simplification(bstr));} );
            });
            simpls.append(bsimpl);
        }
    });
    return simpls;
}

function make_div_addarrow(problem){
    let blob = problem[0];
    let simpls = $('<div/>');
    simpls.append('<p>Available simplifications:</p>');
    api.api_possible_addarrow(blob,function(v){
        for (let simpl of v ){
            let sblob = simpl[0];
            let sstr = simpl[1];
            let bstr = sstr[0] + "→" + sstr[1];
            var bsimpl = $('<button type="button" class="btn btn-primary m-2">'+escape(bstr)+'</button>');
            bsimpl.click(function(ev) {
                api.api_addarrow(blob, sblob, function(x){return append_new_problem(x, performed_addarrow(bstr));} );
            });
            simpls.append(bsimpl);
        }
    });
    return simpls;
}

function make_div_harden(problem){
    let blob = problem[0];
    let x = problem[1];
    let labels = get_labels(problem);

    let hard = $('<div/>');
    let choices = $('<div/>');
    hard.append('<p>Please choose which labels should be kept.</p>');
    for(let label of labels){
        let check = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input"><p class="form-control-static custom-control-label">'+escape(label)+'</p></label></div>');
        choices.append(check);
    }
    let pred = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input" checked><p class="form-control-static custom-control-label">Add diagram predecessors</p></label></div>');
    hard.append(choices);
    hard.append('<hr/>');
    hard.append(pred);
    let hardbtn = $('<button type="button" class="btn btn-primary">Harden</button>');
    hardbtn.click(function(ev) {
        let checks = Array.from($('input[type=checkbox]',choices));
        let entries = Array.from(checks.entries());
        let selected = entries.filter(([i,x]) => x.checked).map(([i,x]) => i);
        let selectedlabels = selected.map(i => labels[i]);
        let usepred = $('input[type=checkbox]',pred).is(':checked');
        api.api_harden(blob,selectedlabels,usepred,function(x){return append_new_problem_or_error(x, performed_harden(merge(selectedlabels)));} );
    });
    hard.append(hardbtn);
    return hard;
}

function make_div_harden2(problem){
    let blob = problem[0];
    let x = problem[1];
    let labels = get_labels(problem);
    let simpls = $('<div/>');
    simpls.append('<p>Click on the label that you want to remove</p>');
    let pred = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input" checked><p class="form-control-static custom-control-label">Add diagram predecessors</p></label></div>');
    for (let simpl of labels ){
        var bsimpl = $('<button type="button" class="btn btn-primary m-2">'+escape(simpl)+'</button>');
        bsimpl.click(function(ev) {
            let others = labels.filter(x => x != simpl);
            let usepred = $('input[type=checkbox]',pred).is(':checked');
            api.api_harden(blob, others, usepred, function(x){return append_new_problem_or_error(x, performed_harden2(simpl));} );
        });
        simpls.append(bsimpl);
    }
    simpls.append('<hr/>');
    simpls.append(pred);
    return simpls;
}

function make_div_autolb(problem){
    let blob = problem[0];
    let x = problem[1];
    let divautolb = $('<div/>');
    let iterlabel = $('<label>Maximum number of iterations:</label>');
    let labelslabel = $('<label>Maximum number of labels:</label>');
    let rcslabel = $('<label>Maximum number of right closed subsets:</label>');

    let maxiterlb = $('<input class="form-control"/>').attr({ type: 'number', value: '15' });
    let maxlabelslb = $('<input class="form-control"/>').attr({ type: 'number', value: '5' });
    let maxrcslb = $('<input class="form-control"/>').attr({ type: 'number', value: '20' });

    let iterform = $('<div class="form-group"/>').append(iterlabel).append(maxiterlb);
    let labelsform = $('<div class="form-group"/>').append(labelslabel).append(maxlabelslb);
    let rcsform = $('<div class="form-group"/>').append(rcslabel).append(maxrcslb);

    divautolb.append(iterform);
    divautolb.append(labelsform);
    divautolb.append(rcsform);


    let collabel = $('<label>Input coloring:</label>');
    let collb = $('<input class="form-control"/>').attr({ type: 'number', value: '99' });
    let colform = $('<div class="form-group"/>').append(collabel).append(collb);
    if( x.left[0].length == 2 || x.right[0].length == 2 ){
        divautolb.append(colform);
    }

    let unreach = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input"><p class="form-control-static custom-control-label">Try to merge unreachable labels</p></label></div>');
    divautolb.append(unreach);
    let diagram = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input" checked><p class="form-control-static custom-control-label">Try to merge diagram neighbors</p></label></div>');
    divautolb.append(diagram);
    let arrows = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input" checked><p class="form-control-static custom-control-label">Try to add diagram edges</p></label></div>');
    divautolb.append(arrows);

    let autolb = $('<button type="button" class="btn btn-primary">Automatic Lower Bound</button>');
    autolb.click(function(ev) {
        let divdivresult = $('<div class="card m-2"/>');
        let closediv = $('<div class="text-left"/>');
        let close = $('<button type="button" class="btn btn-secondary m-2">Close LB</button>');
        closediv.append(close);
        divdivresult.append(closediv);

        let divresult = $('<div class="container-fluid m-0 p-0"/>');
        let spinner = make_spinner("Computing lower bound...");
        divdivresult.append(spinner);

        let onresult = function (_result) {
            if( _result.E != null ){
                alert(_result.E);
                return;
            }
            let result = _result.L;

            let toshow = $('<div/>')

            let rounds = result.filter(step => step[1] == "Speedup" ).length;
            let nt = result[result.length-1][2].is_trivial;
            if ( !nt )rounds += 1;
            toshow.append(make_performed_action("Lower bound of " + rounds + " rounds."));


            for( let step of result ) {
                let what = step[1];
                if( what == "Initial" )toshow.append(performed_initial());
                else if( what == "Speedup" )toshow.append(performed_speedup());
                else if( what.Simplified != null ){
                    for(let simpl of what.Simplified ){
                        let action = simpl[0];
                        let ssimpl = simpl[1] + "→" + simpl[2];
                        if( action == "merge" )toshow.append(performed_simplification(ssimpl));
                        else if(action == "addarrow")toshow.append(performed_addarrow(ssimpl));
                        else alert("this should not happen");
                    }
                }
                problem = [step[0],step[2]];
                append_new_problem_to(problem,toshow,null);
            }
            divresult.html(toshow);
        }
        let onend = function () {
            spinner.remove();
        }
        divdivresult.append(divresult);
        append_generic(divdivresult);
        let useunreach = $('input[type=checkbox]',unreach).is(':checked');
        let usediag = $('input[type=checkbox]',diagram).is(':checked');
        let useaddarrow = $('input[type=checkbox]',arrows).is(':checked');

        let ch = api.api_autolb(blob, parseInt(maxiterlb.val(),10), parseInt(maxlabelslb.val(),10) , parseInt(collb.val(),10) ,parseInt(maxrcslb.val(),10), useunreach, usediag, useaddarrow, onresult, onend);
        close.click(function(){
            divdivresult.remove();
            ch();
        });
    });
    divautolb.append(autolb);
    return divautolb;
}

function make_div_autoub(problem){
    let blob = problem[0];
    let x = problem[1];
    let divautoub = $('<div/>');
    let iterlabel = $('<label>Maximum number of iterations:</label>');
    let labelslabel = $('<label>Maximum number of labels:</label>');
    let rcslabel = $('<label>Maximum number of right closed subsets:</label>');

    let maxiterub = $('<input class="form-control"/>').attr({ type: 'number', value: '5' });
    let maxlabelsub = $('<input class="form-control"/>').attr({ type: 'number', value: '4' });
    let maxrcsub = $('<input class="form-control"/>').attr({ type: 'number', value: '20' });

    let iterform = $('<div class="form-group"/>').append(iterlabel).append(maxiterub);
    let labelsform = $('<div class="form-group"/>').append(labelslabel).append(maxlabelsub);
    let rcsform = $('<div class="form-group"/>').append(rcslabel).append(maxrcsub);

    divautoub.append(iterform);
    divautoub.append(labelsform);
    divautoub.append(rcsform);

    let collabel = $('<label>Input coloring:</label>');
    let colub = $('<input class="form-control"/>').attr({ type: 'number', value: '99' });
    let colform = $('<div class="form-group"/>').append(collabel).append(colub);
    if( x.left[0].length == 2 || x.right[0].length == 2 ){
        divautoub.append(colform);
    }
    let pred = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input" checked><p class="form-control-static custom-control-label">Slow and Accurate</p></label></div>');
    divautoub.append(pred);
    let det = $('<div class="custom-control custom-switch"><label><input type="checkbox" class="custom-control-input"><p class="form-control-static custom-control-label">Test</p></label></div>');
    divautoub.append(det);

    let autoub = $('<button type="button" class="btn btn-primary">Automatic Upper Bound</button>');
    autoub.click(function(ev) {
        let divdivresult = $('<div class="card m-2"/>');
        let closediv = $('<div class="text-left"/>');
        let close = $('<button type="button" class="btn btn-secondary m-2">Close UB</button>');
        closediv.append(close);
        divdivresult.append(closediv);
        
        let divresult = $('<div class="container-fluid m-0 p-0"/>');
        let spinner = make_spinner("Computing upper bound...");
        divdivresult.append(spinner);

        let onresult = function (_result) {
            if( _result.E != null ){
                alert(_result.E);
                return;
            }
            let result = _result.U;

            let toshow = $('<div/>');

            let rounds = result.filter(step => step[1] == "Speedup" ).length;
            let nt = result[result.length-1][2].is_trivial;
            if ( !nt )rounds += 1;
            toshow.append(make_performed_action("Upper bound of " + rounds + " rounds."));

            for(let step of result ) {
                let what = step[1];
                if( what == "Initial" )toshow.append(performed_initial());
                else if( what == "Speedup" )toshow.append(performed_speedup());
                else if( what.Simplified != null ){
                    let s = merge(what.Simplified);
                    toshow.append(performed_harden(s));
                }
                problem = [step[0],step[2]];
                append_new_problem_to(problem,toshow,null);
            }
            divresult.html(toshow);
        }
        let onend = function () {
            spinner.remove();
        }
        divdivresult.append(divresult);
        append_generic(divdivresult);
        let usepred = $('input[type=checkbox]',pred).is(':checked');
        let usedet = $('input[type=checkbox]',det).is(':checked');
        let ch = api.api_autoub(blob, parseInt(maxiterub.val(),10), parseInt(maxlabelsub.val(),10), parseInt(colub.val(),10),parseInt(maxrcsub.val(),10) , usepred, usedet, onresult, onend);
        close.click(function(){
            divdivresult.remove();
            ch();
        });
    });
    divautoub.append(autoub);
    return divautoub;
}

function make_div_triviality(problem){
    let x = problem[1];
    let trivial = "The problem is " + (x.is_trivial? "" : "NOT ") + "zero rounds solvable.";
    if( ! x.is_trivial && x.coloring > 1 ){
        trivial += " It can be solved in zero rounds given a " + x.coloring + " coloring.";
    }
    let div = $('<div/>').append(trivial);
    return div;
}

function make_div_mergeable(problem) {
    let x = problem[1];
    let div = null;
    if( x.mergeable.length > 0 ){
        let groups = x.mergeable.map( v => merge(v));
        let mergeable = groups.join(", ");
        div = $('<div/>').append("The following labels could be merged without changing the complexity of the problem: " + escape(mergeable))
    }
    return div;
}

function make_oldlabel(v,cur_to_old) {
    return merge(v.map(function(x){ return '<span class="rounded m-1 labelborder">'+escape(cur_to_old[x])+'</span><br/>'; }));
}

function make_div_renaming(problem){
    let renaming = get_renaming(problem);
    let table = $('<table class="table"/>');
    for (let [cur,old] of Object.entries(renaming)) {
        let tr = $('<tr/>');
        tr.append('<td><span class="rounded m-1 labelborder">'+escape(old)+'</span></td>');
        tr.append('<td>'+escape(cur)+'</td>');
        table.append(tr);
    }
    return table;
}

function make_div_newrenaming(problem){
    let div = $('<div/>');

    let renaming = get_renaming(problem);
    let table = $('<table class="table"/>');
    for (let [cur,old] of Object.entries(renaming)) {
        let tr = $('<tr/>');
        tr.append('<td><span class="rounded m-1 labelborder">'+escape(old)+'</span></td>');
        let input = $('<input/>').val(cur);
        tr.append($('<td/>').append(input));
        table.append(tr);
    }
    div.append(table);
    let rename = $('<button type="button" class="btn btn-primary">Rename</button>');
    rename.click(function(ev) {
        let labels = Array.from($('input',table)).map(x => x.value);
        let entries = Array.from(labels.entries());
        let newmapping = entries.map(([i,x]) => [problem[1].mapping[i][0],x]);
        console.log(newmapping);
        api.api_rename(problem[0],newmapping,function(x){return append_new_problem_or_error(x, make_performed_action("Renaming."));} );
    });
    div.append(rename);
    return div;
}

function make_table(v,f){
    let s = '<table class="table">';
    for (let line of v) {
        s += '<tr>';
        for (let elem of line) {
            s += '<td>'+f(elem)+'</td>';
        }
        s += '</tr>';
    }
    s += '</table>';
    return s;
}

function make_card(classes1,classes2,title,content,defaultshow,id){
    let r = $('<div class="card '+classes1+'"><div class="card-header '+classes2+'"><button class="btn btn-link" data-toggle="collapse" data-target=".collapse'+id+'">'+
        title + '</button></div><div class="collapse' +id+ ' collapse ' + (defaultshow?"show":"") + '"><div class="card-body"></div></div></div>');
    r.find('.card-body').first().append(content);
    return r;
}


function generate_html_for_problem(problem, reason) {
    let blob = problem[0];
    let x = problem[1];
    
    let divtrivial = make_div_triviality(problem);
    let trivial = $('<div class="card card-body m-0 p-2"/>').append(divtrivial);
    let col_trivial = $('<div class="col-auto m-2 p-0">').append(trivial);

    let col_left_old = $("<div/>");
    let col_right_old = $("<div/>");;
    let col_renaming = $("<div/>");;
    if( x.mapping != null ){
        let id = freeid();
        let cur_to_old = get_renaming(problem);
        let left_old = make_table(x.left,  function(v){return make_oldlabel(v,cur_to_old);});
        let right_old = make_table(x.right, function(v){return make_oldlabel(v,cur_to_old);});
        let renaming = make_div_renaming(problem);
        col_left_old = make_card("m-2","p-0","<h6>Active (Before Renaming)</h6><h6><small>Any choice satisfies previous Passive</small></h6>",left_old,false,id);
        col_right_old = make_card("m-2","p-0","<h6>Passive (Before Renaming)</h6><h6><small>Exists choice satisfying previous Active</small></h6>",right_old,false,id);
        col_renaming = make_card("m-2","p-0","<h6>Renaming</h6><h6><small>Old and new labels</small></h6>",renaming,false,id);
    }
    
    let id_new_leftright = freeid();
    let left_new = make_table(x.left,function(x){return merge(x.map(y => escape(y)))});
    let right_new = make_table(x.right,function(x){return merge(x.map(y => escape(y)))});
    let col_left_new = make_card("m-2","p-0","<h6>Active</h6><h6><small>Any choice satisfies previous Passive</small></h6>",left_new,true,id_new_leftright);
    let col_right_new = make_card("m-2","p-0","<h6>Passive</h6><h6><small>Exists choice satisfying previous Active</small></h6>",right_new,true,id_new_leftright);

    let diagram = make_div_diagram(problem);
    let col_diagram = make_card("m-2","p-0","<h6>Diagram</h6><h6><small>Strength of right side labels</small></h6>",diagram,true,id_new_leftright);

    let divmergeable = make_div_mergeable(problem);
    let mergeable = $('<div class="card card-body m-0 p-2"/>').append(divmergeable);
    let col_mergeable = divmergeable == null? $('<div/>') : $('<div class="col-auto m-2 p-0">').append(mergeable);

    let next = make_button_speedup(problem);
    let edit = make_button_edit(problem);
    let simpls = make_div_simplifications(problem);
    let simpls_card = make_card("m-2","p-0","<h7>Simplifications</h7>",simpls,false,freeid());
    let addarrow = make_div_addarrow(problem);
    let addarrow_card = make_card("m-2","p-0","<h7>Add arrows</h7>",addarrow,false,freeid());
    let hard = make_div_harden(problem);
    let hard_card = make_card("m-2","p-0","<h7>Harden v1</h7>",hard,false,freeid());
    let hard2 = make_div_harden2(problem);
    let hard2_card = make_card("m-2","p-0","<h7>Harden v2</h7>",hard2,false,freeid());
    let divautolb = make_div_autolb(problem);
    let divautoub = make_div_autoub(problem);
    let autolb_card = make_card("m-2","p-0","<h7>Automatic Lower Bound</h7>",divautolb,false,freeid());
    let autoub_card = make_card("m-2","p-0","<h7>Automatic Upper Bound</h7>",divautoub,false,freeid());


    let tools = $('<div/>');
    tools.append(next);
    tools.append(edit);
    tools.append(simpls_card);
    tools.append(addarrow_card);
    tools.append(hard_card);
    tools.append(hard2_card);
    tools.append(autolb_card);
    tools.append(autoub_card);
    if( x.mapping != null ){
        let divnewrenaming = make_div_newrenaming(problem);
        let newrenaming_card = make_card("m-2","p-0","<h7>New Renaming</h7>",divnewrenaming,false,freeid());
        tools.append(newrenaming_card);
    }

    let col_tools = make_card("m-2 d-print-none","p-0","<h6>Tools</h6><h6><small>Speedup, edit, simplifications, ...</small></h6>",tools,true,id_new_leftright);

    let row = $('<div class="row p-0 m-2"/>').append(col_trivial,$('<div class="w-100"/>'),col_mergeable, $('<div class="w-100"/>'),col_left_old,col_right_old,col_renaming,$('<div class="w-100"/>'),col_left_new,col_right_new,col_diagram,col_tools);
    let result = $('<div class="card card-body m-2 p-2 bg-light"/>');

    let div = $('<div/>');

    let closediv = $('<div class="text-left"/>');
    let close = $('<button type="button" class="btn btn-secondary ml-3 mt-3">Close</button>');
    close.click(function(){
        div.remove();
    });
    closediv.append(close);

    result.append(row);

    result.prepend(closediv);

    if( reason != null ){
        div.append(reason);
    }
    div.append(result);
    return div;
}


function append_generic(x) {
    $("#steps").append(x);
}

function append_new_problem_to(x,to, reason) {
    let html = generate_html_for_problem(x,reason);
    to.append(html);
}

function append_new_problem(x, reason) {
    append_new_problem_to(x,$("#steps"),reason)
}

function append_new_problem_or_error(x, reason) {
    if( x.E != null ){
        alert(x.E);
        return;
    }
    append_new_problem_to(x.P,$("#steps"),reason)
}


function on_input_click() {
    let f1 = $('#inf1').val();
    let f2 = $('#inf2').val();
    let text = "";
    for(let line of f1.split("\n")) {
        if( line != "" ){
            text += line + "\n";
        }
    }
    let a = text;
    text = "";
    for(let line of f2.split("\n")) {
        if( line != "" ){
            text += line + "\n";
        }
    }
    let b = text;
    api.api_new_problem(a,b, function(x){return append_new_problem_or_error(x, performed_initial());} );
}
