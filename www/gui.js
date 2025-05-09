
import * as api from "./api.js"

let version = 2;

function handle_result(x, onresult, onerror, progress) {
    if( x.E != null ) {
        onerror(x.E);
    }
    if( x.P != null ){
        let p = x.P;
        fix_problem(p);
        onresult(p);
    }
    if( x.W != null ){
        onerror(x.W,true);
    }
    if( x.AutoUb != null ){
        for( let step of x.AutoUb[1] ){
            fix_problem(step[1]);
        }
        onresult(x.AutoUb)
    }
    if( x.AutoLb != null ){
        for( let step of x.AutoLb[1] ){
            fix_problem(step[1]);
        }
        onresult(x.AutoLb)
    }

    if( x.Event != null ){
        progress.type = x.Event[0];
        if(x.Event.length > 1) {
            progress.cur = x.Event[1];
            progress.max = x.Event[2];
        }
    }
};

function new_problem(left, right, onresult, onerror, progress) {
    let ondata = x => handle_result(x, onresult, onerror, progress);
    api.request({ NewProblem : [left,right] }, ondata , function(){});
}

function speedup(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Speedup : problem }, ondata , function(){});
}

function demisifiable(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Demisifiable : problem }, ondata , function(){});
}

function add_active_predecessors(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AddActivePredecessors : problem }, ondata , function(){});
}

function remove_trivial_lines(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ RemoveTrivialLines : problem }, ondata , function(){});
}

function fixpoint_gendefault(problem, partial, triviality_only, sublabels, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ DefaultDiagram : [problem,partial,triviality_only,sublabels] }, ondata , function(){});
}

function fixpoint_basic(problem, partial, triviality_only, sublabels, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointBasic : [problem,partial,triviality_only,sublabels] }, ondata , function(){});
}

function fixpoint_loop(problem, partial, triviality_only, sublabels, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointLoop : [problem,partial,triviality_only,sublabels] }, ondata , function(){});
}

function fixpoint_custom(problem, diagram, partial, triviality_only, sublabels, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointCustom : [problem, diagram, partial, triviality_only, sublabels] }, ondata , function(){});
}

function fixpoint_dup(problem, dups, partial, triviality_only, sublabels, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointDup : [problem, dups, partial, triviality_only, sublabels] }, ondata , function(){});
}

function give_orientation(problem, outdegree, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Orientation : [problem,parseInt(outdegree)] }, ondata , function(){});
}

function inverse_speedup(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ InverseSpeedup : problem }, ondata , function(){});
}

function all_different_labels(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AllDifferentLabels : problem }, ondata , function(){});
}

function delta_edge_coloring(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ DeltaEdgeColoring : problem }, ondata , function(){});
}

function compute_coloring_solvability(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ ColoringSolvability : problem }, ondata , function(){});
}

function apply_marks_technique(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Marks : problem }, ondata , function(){});
}

function speedupmaximize(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SpeedupMaximize : problem }, ondata , function(){});
}

function speedupmaximizerenamegen(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SpeedupMaximizeRenamegen : problem }, ondata , function(){});
}

function simplify_merge(problem, from, to, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SimplifyMerge : [problem, parseInt(from), parseInt(to)] }, ondata , function(){});
}

function simplify_merge_sd(problem, sd, recompute_diagram, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SimplifySD : [problem, sd, recompute_diagram] }, ondata , function(){});
}

function simplify_group(problem, labels, to, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SimplifyMergeGroup : [problem, labels.map(x => parseInt(x)), parseInt(to)] }, ondata , function(){});
}

function simplify_addarrow(problem, from, to, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ SimplifyAddarrow : [problem, parseInt(from), parseInt(to)] }, ondata , function(){});
}

function harden_remove(problem, label, keep_predecessors, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ HardenRemove : [problem, parseInt(label), keep_predecessors] }, ondata , function(){});
}

function critical_harden(problem, b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, keep_predecessors, b_maximize_rename, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ CriticalHarden : [problem, b_coloring, parseInt(coloring), b_coloring_passive, parseInt(coloring_passive), parseInt(zerosteps), keep_predecessors, b_maximize_rename] }, ondata , function(){});
}

function critical_relax(problem, b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, b_maximize_rename, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ CriticalRelax : [problem, b_coloring, parseInt(coloring), b_coloring_passive, parseInt(coloring_passive), parseInt(zerosteps), b_maximize_rename] }, ondata , function(){});
}


function harden_keep(problem, labels, keep_predecessors, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ HardenKeep : [problem, labels.map(x => parseInt(x)), keep_predecessors] }, ondata , function(){});
}

function merge_equivalent_labels(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ MergeEquivalentLabels : problem }, ondata , function(){});
}

function maximize(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Maximize : problem }, ondata , function(){});
}

function fulldiagram(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FullDiagram : problem }, ondata , function(){});
}

function renamegenerators(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ RenameGenerators : problem }, ondata , function(){});
}

function rename(problem, renaming, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Rename : [problem,renaming] }, ondata , function(){});
}

function autoub(problem, b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, coloring_given, coloring, coloring_given_passive, coloring_passive, onresult, onerror, progress, oncomplete){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AutoUb : [problem, b_max_labels, parseInt(max_labels), b_branching, parseInt(branching), b_max_steps, parseInt(max_steps), coloring_given, parseInt(coloring), coloring_given_passive, parseInt(coloring_passive)] }, ondata, oncomplete);
}

function autolb(problem, b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, coloring_given, coloring, coloring_given_passive, coloring_passive, onresult, onerror, progress, oncomplete){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AutoLb : [problem, b_max_labels, parseInt(max_labels), b_branching, parseInt(branching),  b_max_steps, parseInt(max_steps), coloring_given, parseInt(coloring), coloring_given_passive, parseInt(coloring_passive)] }, ondata, oncomplete);
}

function check_zero_with_input(problem, active, passive, reverse, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ CheckZeroWithInput : [problem, active, passive, reverse] }, ondata , function(){});
}

function dual(problem, active, passive, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Dual : [problem, active, passive] }, ondata , function(){});
}

function doubledual(problem, active, passive, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ DoubleDual : [problem, active, passive] }, ondata , function(){});
}

function doubledual2(problem, active, passive,diagram,input_active,input_passive, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ DoubleDual2 : [problem, active, passive,diagram,input_active,input_passive] }, ondata , function(){});
}

function fix_problem(p) {
    p.map_label_text = vec_to_map(p.mapping_label_text);
    p.map_label_oldlabels = vec_to_map(p.mapping_label_oldlabels) ?? null;
    p.map_oldlabel_labels = vec_to_map(p.mapping_oldlabel_labels) ?? null;
    p.map_oldlabel_text = vec_to_map(p.mapping_oldlabel_text) ?? null;
    p.labels = p.mapping_label_text.map(x => x[0]);
    let problem = p;
    let numlabels = problem.mapping_label_text.length;
    let is_zero = problem.trivial_sets != null && problem.trivial_sets.length > 0;
    let is_nonzero = problem.trivial_sets != null && problem.trivial_sets.length == 0;
    let orientation_is_zero = problem.orientation_trivial_sets != null && problem.orientation_trivial_sets.length > 0;
    let orientation_is_nonzero = problem.orientation_trivial_sets != null && problem.orientation_trivial_sets.length == 0;
    let numcolors = problem.coloring_sets != null ? problem.coloring_sets.length : -1;
    let orientation_numcolors = problem.orientation_coloring_sets != null ? problem.orientation_coloring_sets.length : -1;
    let zerosets = !is_zero ? [] : problem.trivial_sets.map(x => labelset_to_string(x,problem.map_label_text));
    let orientation_zerosets = !orientation_is_zero ? [] : problem.orientation_trivial_sets.map(x => "("+labelset_to_string(x[0],problem.map_label_text)+","+labelset_to_string(x[1],problem.map_label_text)+")");
    let coloringsets = numcolors < 2 ? [] : problem.coloring_sets.map(x => labelset_to_string(x,problem.map_label_text));
    let orientation_coloringsets = orientation_numcolors < 2 ? [] : problem.orientation_coloring_sets.map(x => "("+labelset_to_string(x[0],problem.map_label_text)+","+labelset_to_string(x[1],problem.map_label_text)+")");
    let mergeable = (problem.diagram_direct ?? [[]])[0].filter(x => x[1].length > 1); 
    let is_mergeable = mergeable.length > 0;
    let mergesets = !is_mergeable ? [] : mergeable.map(x => labelset_to_string(x[1],problem.map_label_text));
    let demisifiable = (problem.demisifiable ?? []).map(x => [labelset_to_string(x[0],problem.map_label_text),labelset_to_string(x[1],problem.map_label_text)]);
    //console.log(demisifiable);
    let is_demisifiable = demisifiable.length > 0;
    if( p.fixpoint_diagram !== null ){
        p.fixpoint_diagram[1].map_label_text = vec_to_map(p.fixpoint_diagram[1].mapping_newlabel_text);
    }
    let fp_procedure_works = (problem.fixpoint_procedure_works != null && problem.fixpoint_procedure_works);
    let fp_procedure_does_not_work = (problem.fixpoint_procedure_works != null && !problem.fixpoint_procedure_works);

    let marks_works = (problem.marks_works != null && problem.marks_works);
    let marks_does_not_work = (problem.marks_works != null && !problem.marks_works);

    let zero_with_input =  (problem.is_trivial_with_input != null && problem.is_trivial_with_input);
    let non_zero_with_input = (problem.is_trivial_with_input != null && !problem.is_trivial_with_input);

    let triviality_with_input = null;
    if( zero_with_input ){
        let input_to_string = vec_to_map(problem.triviality_with_input[0]);
        let output_to_string = p.map_label_text;
        let mapping = problem.triviality_with_input[1];
        triviality_with_input = mapping.map(x => [labelset_to_string([x[0]],input_to_string),  labelset_to_string(x[1],output_to_string)]);
    }
    p.info = { orientation_coloringsets:orientation_coloringsets, orientation_numcolors:orientation_numcolors, orientation_zerosets:orientation_zerosets,orientation_is_zero:orientation_is_zero, orientation_is_nonzero:orientation_is_nonzero, numlabels : numlabels, is_zero : is_zero, is_nonzero : is_nonzero, numcolors : numcolors, zerosets : zerosets, coloringsets : coloringsets, is_mergeable : is_mergeable, mergesets : mergesets, is_demisifiable : is_demisifiable, demisifiable : demisifiable, fp_procedure_works : fp_procedure_works, fp_procedure_does_not_work : fp_procedure_does_not_work, marks_works : marks_works, marks_does_not_work : marks_does_not_work, zero_with_input:zero_with_input, non_zero_with_input: non_zero_with_input, triviality_with_input : triviality_with_input};
}



function on_new_what(stuff, action, progress, p, what, removeprogress = true){
    let idx = stuff.indexOf(progress);
    if( removeprogress ){
        stuff.splice(idx,1);
    }
    if( what == "problem" ){
        stuff.push({ type : "performed", data : action });
        stuff.push({ type : "problem", data : p });
    }else if( what == "sequence" ){
        let action_copy = JSON.parse(JSON.stringify(action));
        let len = p[0];
        let sequence = p[1];
        let substuff = [];
        action_copy.len = len;
        substuff.push({ type : "performed", data : action_copy });
        for( var step of sequence ){
            let operation = step[0];
            if( operation == "Initial" ){
                substuff.push({ type : "performed", data : {type:"initial"} });
            } else if( operation == "Speedup" ){
                substuff.push({ type : "performed", data : {type:"speedup"} });
            } else if( operation.Harden != null) {
                substuff.push({ type : "performed", data : {type:"hardenkeep", labels:operation.Harden.map(x => step[1].map_label_text[x])} });
            } else if( operation.Merge != null) {
                let before_merge = operation.Merge[1];
                for( let merge of operation.Merge[0] ){
                    fix_problem(before_merge);
                    substuff.push({ type : "performed", data : {type:"simplificationmerge", from: before_merge.map_label_text[merge[0]], to : before_merge.map_label_text[merge[1]]} });
                }
            }
            substuff.push({ type : "problem", data : step[1] });
        }
        stuff.splice(idx+1,0,{ type : "sub", data : substuff });
    }
}


function call_api_generating_problem(stuff, action, f, params, removeprogress = true) {
    return call_api_generating_what(stuff, action, f, params, "problem", removeprogress);
}

function call_api_generating_sequence(stuff, action, f, params, removeprogress = true) {
    return call_api_generating_what(stuff, action, f, params, "sequence", removeprogress);
}


function call_api_generating_what(stuff, action, f, params, what, removeprogress = true) {
    let progress = { type : "computing", data : {type : "empty", cur : 1, max : 1, onstop : function(){}} };
    stuff.push(progress);
    let remove_progress_bar = function() {
        //console.log("removing progress bar");
        let idx = stuff.indexOf(progress);
        if(idx != -1)stuff.splice(idx,1);
    }
    let termination_handle = removeprogress?
        f(...params, p => on_new_what(stuff, action, progress, p, what, removeprogress),(e,warning=false) =>  { if(!warning)remove_progress_bar() ; stuff.push({ type : "error", data : e, warning });} ,progress.data) :
        f(...params, p => on_new_what(stuff, action, progress, p, what, removeprogress),(e,warning=false) =>  { if(!warning)remove_progress_bar() ; stuff.push({ type : "error", data : e, warning });} ,progress.data, function(){
            remove_progress_bar();
        });

    progress.data.onstop = function() {
        remove_progress_bar();
        //console.log("killing worker");
        termination_handle();
    }
}

function vec_to_map(v){
    if( v == null ){
        return null;
    }
    return Object.assign({}, ...v.map((x) => ({[x[0]]: x[1]})));
}

function labelset_to_string(v, mapping, sep = "") {
    return v.map(x => mapping[x]).join(sep);
}


function constraint_to_text(constraint,mapping) {
    return constraint.lines.map(line => {
        return line.parts.map(part => {
            let exp = "";
            if( part.gtype == "Star" )exp="*";
            else if( part.gtype.Many != 1 )exp = "^" + part.gtype.Many;
            return part.group.map( label => mapping[label] ).join("") + exp;
        }).join(" ")
    }).join("\n")
}

Vue.component('re-performed-action', {
    props: ['action','handle','stuff'],
    methods: {
        on_close() {
            let idx = this.stuff.indexOf(this.handle);
            this.stuff.splice(idx,1);
        }
    },
    computed: {
        actionview: function() {
            switch( this.action.type ) {
                case "initial":
                    return "Initial problem";
                case "mergeequal":
                    return "Merged equivalent labels.";
                case "simplificationmerge":
                    return "Performed Simplification: Merged " + this.action.from + "→" + this.action.to;
                case "simplificationaddarrow":
                    return "Performed Simplification: Added Arrow " + this.action.from + "→" + this.action.to;
                case "hardenkeep":
                    return "Performed Hardening: Kept Label Set " + this.action.labels.join("");
                case "simplifymergegroup":
                    return "Performed Simplification: Merged Set " + this.action.labels.join("") + "→" + this.action.to;
                case "simplifymergesd":
                    return "Performed SubDiagram Merging\n" + this.action.sd;
                case "zerowithinput":
                    return "Checked whether the problem is zero-round solvable with the following input:\n\n"+this.action.active+"\n\n" + this.action.passive + "\n\nReverse: " + this.action.reverse;
                case "dual":
                    return "Computed dual wrt the following problem:\n\n"+this.action.active+"\n\n" + this.action.passive;
                case "doubledual":
                    return "Computed double dual wrt the following problem:\n\n"+this.action.active+"\n\n" + this.action.passive;
                case "doubledual2":
                    return "Computed dual wrt the following problem:\n\n"+this.action.active+"\n\n" + this.action.passive+"\n\nwrt the following diagram:\n\n" + this.action.diagram + "\n\nwrt the following input:\n\n"+this.action.input_active+"\n\n" + this.action.input_passive;
                    case "hardenremove":
                case "hardenremove":
                    return "Performed Hardening: Removed Label " + this.action.label;
                case "criticalharden":
                    return "Performed Hardening by Critical Sets";
                case "criticalrelax":
                    return "Performed Relaxation by Critical Sets";
                case "orientation":
                    return "Gave input orientation. Outdegree = " + this.action.outdegree;
                case "speedup":
                    return "Performed speedup";
                case "demisifiable":
                    return "Computed deMISifiable sets.";
                case "add-active-predecessors":
                    return "Added Predecessors On Active Side.";
                case "remove-trivial-lines":
                    return "Removed Trivial Lines.";
                case "fixpoint-basic":
                    return "Generated Fixed Point with Default Diagram" + (this.action.sub !== null ? " for labels " + this.action.sub : "");
                case "fixpoint-gendefault":
                    return "Generated Default Fixed Point Diagram" + (this.action.sub !== null ? " for labels " + this.action.sub : "");
                case "fixpoint-loop":
                    return "Generated Fixed Point with Automatic Diagram Fixing" + (this.action.sub !== null ? " for labels " + this.action.sub : "");
                case "fixpoint-custom":
                    return "Generated Fixed Point with Custom Diagram" + (this.action.sub !== null ? " for labels " + this.action.sub : "") + ":\n" + this.action.diagram;
                case "fixpoint-dup":
                    return "Generated Fixed Point With Label Duplication" + (this.action.sub !== null ? " for labels " + this.action.sub : "") + ": "+ this.action.dups;
                case "inversespeedup":
                    return "Performed inverse speedup";
                case "alldifferentlabels":
                    return "Made All Labels Different";
                case "deltaedgecoloring":
                    return "Transformed Labels Assuming a Delta Edge Coloring";
                case "coloring":
                    return "Computed hypergraph strong coloring solvability";
                case "marks":
                    return "Applied Marks' technique";
                case "speedupmaximize":
                    return "Performed speedup and maximized";
                case "speedupmaximizerenamegen":
                    return "Performed speedup, maximized, and renamed by generators";
                case "maximize":
                    return "Maximized passive side";
                case "fulldiagram":
                    return "Computed full diagram";
                case "renamegenerators":
                    return "Renamed by generators";
                case "rename":
                    return "Renamed";
                case "autoub":
                    return "Automatic Upper Bound. Obtained Upper Bound of " + this.action.len + " Rounds.";
                case "autolb":
                    if(this.action.len == 999 ){
                        return "Automatic Lower Bound. Obtained a Fixed Point."
                    }else {
                        return "Automatic Lower Bound. Obtained Lower Bound of " + this.action.len + " Rounds.";
                    }
                default:
                    return "Unknown " + this.action.type
            }
        }
    },
    template: `
        <div class="card bg-primary text-white m-2 p-2">
            <div class="position-absolute top-0 end-0 m-1 p-1"><button type="button" class="close" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
            </button></div><span style="white-space: break-spaces;">{{ actionview }}</span>
        </div>
    `
})

Vue.component('re-error', {
    props: ['error','warning','handle','stuff'],
    methods: {
        on_close() {
            let idx = this.stuff.indexOf(this.handle);
            this.stuff.splice(idx,1);
        }
    },
    template: `
        <div class="card m-2 p-2" :class="this.warning? 'bg-warning text-black' : 'bg-danger text-white'">
            <span>
                {{ this.error }}
                <button type="button" class="close" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
            </span>
        </div>
    `
})

Vue.component('re-computing', {
    props: ['action'],
    computed: {
        state: function() {
            switch( this.action.type ) {
                case "fixpoint autofix":
                    return {bar : false, msg: "Computing Fixed Point with Automatic Diagram Fixing ("+this.action.max+" labels)"}; 
                case "autoub":
                    return {bar : false, msg: "Computing an Upper Bound Automatically"}; 
                case "autolb":
                    return {bar : false, msg: "Computing a Lower Bound Automatically"}; 
                case "coloring graph":
                    return {bar : true, msg: "Computing graph for determining coloring solvability", max : this.action.max, cur : this.action.cur };
                case "clique":
                    return {bar : false, msg: "Computing largest clique"};
                case "diagram":
                    return {bar : true, msg: "Computing diagram", max : this.action.max, cur : this.action.cur };
                case "recompute diagram":
                    return {bar : false, msg: "Computing diagram"};
                case "discard non maximal":
                    return {bar : false, msg: "Discarding non-maximal lines"};
                case "remove weak":
                    return {bar : false, msg: "Removing weak lines"};
                case "discard labels at most one side":
                    return {bar : false, msg: "Discarding labels that appear on at most one side"};
                case "discard unused internal":
                    return {bar : false, msg: "Discard unused internal variables"};
                case "new labels":
                    return {bar : false, msg: "Computing new labels"};
                case "enumerating configurations":
                    return {bar : false, msg: "Enumerating configurations"};
                case "demisifiable":
                    return {bar : true, msg: "Computing deMISifiable sets", max : this.action.max, cur : this.action.cur };
                case "combining line pairs":
                    return {bar : true, msg: "Maximizing, combining lines ("+this.action.max+")", max : this.action.max, cur : this.action.cur };
                case "triviality":
                    return {bar : true, msg: "Computing triviality", max : this.action.max, cur : this.action.cur };
                case "orientationtriviality":
                    return {bar : true, msg: "Computing orientation triviality", max : this.action.max, cur : this.action.cur };
                case "setting up node constraints":
                    return {bar : true, msg: "Setting up node constraints", max : this.action.max, cur : this.action.cur };
                case "setting up edge constraints":
                    return {bar : true, msg: "Setting up edge constraints", max : this.action.max, cur : this.action.cur };
                case "sanitizing":
                    return {bar : true, msg: "Sanitizing the CNF formula" };
                case "calling the sat solver":
                    return {bar : true, msg: "Calling the SAT solver..." };
                default:
                    return {bar : false, msg: ""};
            }
        }
    },
    methods : {
        on_close() {
            //console.log("close button clicked");
            if(this.action.onstop != null){
                //console.log("stopping some work");
                this.action.onstop()
            }
        }
    },
    template: `
        <div class="card card-body m-2 bg-light">
            <div class="spinner-border" role="status"></div>
            {{ state.msg }}
            <div v-if="state.bar" class="progress">
                <div class="progress-bar" role="progressbar" :style="'width : ' + Math.floor(state.cur *100 / state.max) + '%'"></div>
            </div>
            <button type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close" v-on:click="on_close">
                <span aria-hidden="true">&times;</span>
            </button>
        </div>
        
    `
})


Vue.component('re-problem-info', {
    props: ['problem'],
    template: `
        <div class="row p-0 m-2">
            <div class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>{{ this.problem.info.numlabels }} Labels.</div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.is_zero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem IS zero round solvable.</div>
                    <div>The following sets allow zero round solvability:
                        <span v-for="set in this.problem.info.zerosets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.problem.info.is_nonzero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT zero round solvable.</div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.numcolors >= 2" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is solvable in zero rounds given a {{ this.problem.info.numcolors }} coloring.</div>
                    <div>The following sets are colors:
                        <span v-for="set in this.problem.info.coloringsets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.problem.info.numcolors ==0 && !this.problem.info.is_zero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT solvable even if given a 2-coloring.</div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.orientation_is_zero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem IS zero round solvable with the given orientation.</div>
                    <div>The following sets allow zero round solvability:
                        <span v-for="set in this.problem.info.orientation_zerosets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.problem.info.orientation_is_nonzero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT zero round solvable with the given orientation.</div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.orientation_numcolors >= 2" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is solvable in zero rounds given a {{ this.problem.info.orientation_numcolors }} coloring and the orientation.</div>
                    <div>The following sets are colors:
                        <span v-for="set in this.problem.info.orientation_coloringsets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.problem.info.orientation_numcolors ==0 && !this.problem.info.orientation_is_zero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT solvable even if given a 2-coloring and the orientation.</div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.is_mergeable" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The following labels can be merged:
                        <span v-for="set in this.problem.info.mergesets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.is_demisifiable" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>DeMISifiable merges:
                        <div v-for="set in this.problem.info.demisifiable">{{ set[0] }} <span v-if="set[1].length > 0">by removing labels {{ set[1] }}</span></div>
                    </div>
                </div>
            </div>
            <div class="w-100"/>
            <div v-if="this.problem.info.fp_procedure_works" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem can be relaxed into a non-trivial fixed point.</div>
                </div>
            </div>
            <div v-if="this.problem.info.fp_procedure_does_not_work" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The fixed point procedure failed to produce a non-trivial fixed point relaxation.</div>
                </div>
            </div>
            <div v-if="this.problem.info.marks_works" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>Marks' technique gives a lower bound.</div>
                </div>
            </div>
            <div v-if="this.problem.info.marks_does_not_work" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>Marks' technique does not give a lower bound.</div>
                </div>
            </div>
            <div v-if="this.problem.info.zero_with_input" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem IS zero-round solvable with the given input.</div>
                    <div>There exists the following mapping:
                        <div v-for="pair in this.problem.info.triviality_with_input">{{ pair[0] }} → {{ pair[1] }}</div>
                    </div>
                </div>
            </div>
            <div v-if="this.problem.info.non_zero_with_input" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT zero-round solvable with the given input.</div>
                </div>
            </div>
        </div>
    `
})


Vue.component('re-constraint', {
    props: ['problem','side','mode','mode2'],
    computed: {
        table : function() {
            let problem = this.problem;
            let constraint = this.side == "active" ? problem.active : problem.passive;
            return constraint.lines.map((row,i) => row.parts.map((elem,j) => {
                let renamed = labelset_to_string(elem.group,this.problem.map_label_text);
                let original = problem.mapping_label_oldlabels == null ? null : elem.group.map(x => labelset_to_string(this.problem.map_label_oldlabels[x],this.problem.map_oldlabel_text));
                let gen_renamed = null;
                let gen_original = null;

                if( this.side == "passive" && problem.passive_gen != null ){
                    let elem_gen = problem.passive_gen.lines[i].parts[j];
                    gen_renamed = labelset_to_string(elem_gen.group,this.problem.map_label_text);
                    gen_original = problem.mapping_label_oldlabels == null ? null : elem_gen.group.map(x => labelset_to_string(this.problem.map_label_oldlabels[x],this.problem.map_oldlabel_text));
                }

                let r = {  renamed : renamed, original : original, gen_renamed : gen_renamed, gen_original : gen_original};
                if( elem.gtype == "One" || elem.gtype.Many == 1 ){
                } else if( elem.gtype == "Star" ){
                    r.star = true;
                } else {
                    r.rep = elem.gtype.Many;
                }
                
                return r;
            }));
        }
    },
    template: `
        <table class="table">
            <tr v-for="row in this.table">
                <td v-for="elem in row">
                    <div v-if="mode2 != 'gen' || elem.gen_renamed == null">
                        <div v-if="mode == 'original'">
                            <span v-for="set in elem.original" class="rounded m-1 labelborder">{{ set }}</span>
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                        <div v-if="mode == 'renamed'">
                            {{ elem.renamed }}
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                        <div v-if="mode == 'both'">
                            {{ elem.renamed }}
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                            <hr/>
                            <span v-for="set in elem.original" class="rounded m-1 labelborder">{{ set }}</span>
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                    </div>
                    <div v-if="mode2 == 'gen' && elem.gen_renamed != null">
                        <div v-if="mode == 'original'">
                            <span v-for="set in elem.gen_original" class="rounded m-1 labelborder">{{ set }}</span>
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                        <div v-if="mode == 'renamed'">
                            {{ elem.gen_renamed }}
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                        <div v-if="mode == 'both'">
                            {{ elem.gen_renamed }}
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                            <hr/>
                            <span v-for="set in elem.gen_original" class="rounded m-1 labelborder">{{ set }}</span>
                            <sup v-if="elem.rep">{{ elem.rep }}</sup>
                            <span v-if="elem.star">*</span>
                        </div>
                    </div>
                </td>
            </tr>
        </table>
    `   
})


Vue.component('re-card',{
    props: ['title','subtitle','show'],
    template : `
        <div class="card m-2">
            <div class="card-header p-0">
                <button class="btn btn-link text-left" data-toggle="collapse" :data-target="'.collapse'+this._uid">
                    {{ this.title }}<br/>
                    <small v-if="this.subtitle!=''">{{ this.subtitle }}</small>
                </button>
            </div>
            <div :class="'collapse'+this._uid + ' collapse ' + (this.show?'show':'')">
                <div class="card-body">
                    <slot></slot>
                </div>
            </div>
        </div>
    `
})

Vue.component('re-renaming', {
    props: ["problem"],
    computed: {
        table: function() {
            return this.problem.mapping_label_oldlabels.map(x => ({
                old: labelset_to_string(this.problem.map_label_oldlabels[x[0]],this.problem.map_oldlabel_text), 
                cur: this.problem.map_label_text[x[0]]
            }));
        }
    },
    template: `
        <table class="table">
            <tr v-for="row in this.table">
                <td><span class="rounded m-1 labelborder">{{ row.old }}</span></td>
                <td>{{ row.cur }}</td>
            </tr>
        </table>
    `
})

Vue.component('re-inverse-renaming', {
    props: ["problem"],
    computed: {
        table: function() {
            return this.problem.mapping_oldlabel_labels.map(x => ({
                cur: labelset_to_string(this.problem.map_oldlabel_labels[x[0]],this.problem.map_label_text), 
                old: this.problem.map_oldlabel_text[x[0]]
            }));
        }
    },
    template: `
        <table class="table">
            <tr v-for="row in this.table">
                <td>{{ row.old }}</td>
                <td><span class="rounded m-1 labelborder">{{ row.cur }}</span></td>
            </tr>
        </table>
    `
})



Vue.component('re-diagram', {
    props: ["problem"],
    data : function() {
        return {
            hierarchical : false,
            physics : true,
            network : [null]
        }
    },
    computed: {
        visdata : function() {
            let nodes = [];
            for( let node of this.problem.diagram_direct[0] ){
                nodes.push({ id : node[0], label: node[1].map(x => this.problem.map_label_text[x]).join(",") });
            }
            let edges = [];
            for( let edge of this.problem.diagram_direct[1] ){
                edges.push({ from : edge[0], to : edge[1], arrows: 'to'});
            }
            let visnodes = new vis.DataSet(nodes);
            let visedges = new vis.DataSet(edges);
            let visdata = {
                nodes: visnodes,
                edges: visedges
            };
            return visdata;
        },
        options : function() {
            return {
                interaction: {
                    zoomView: false,
                    navigationButtons: true,
                    multiselect : true
                },
                physics:{
                    enabled: this.physics,
                    hierarchicalRepulsion: {
                        nodeDistance: 200,
                    }
                },
                nodes: {
                    color:{
                        highlight: '#FF7f7f'
                    }
                },
                edges: {
                    physics: true,
                    smooth: false,
                    color:{
                        highlight: '#FF7f7f'
                    }
                },
                layout: {
                    hierarchical: this.hierarchical ? {
                      direction: 'LR',
                      sortMethod: 'directed',
                      levelSeparation: 200,
                    } : false
                }
            };
        }
    },
    methods: {
        show_diagram : function() {
            let id = "diagram" + this._uid;
        let network = new vis.Network(document.getElementById(id), this.visdata, {});
        network.setOptions(this.options);
        let p = this.problem;
        network.on("select", function() {
            p.selected = network.getSelectedNodes();
        });
        network.on("selectEdge", function() {
            p.selectedEdges = network.getSelectedEdges().map(x => network.getConnectedNodes(x));
        });

        /*network.on("stabilized", function () {
            this.options.layout = {"hierarchical": {"enabled": false}};
            network.setOptions({
                "layout": {"hierarchical": {"enabled": false}},                  
            });
        });*/
         

        //prevent vue from adding getters and setters, otherwise some things of vis break
        this.network[0] = network;
        //console.log(this.id + " " + this.visdata.nodes.length);
        },
        on_export : function() {
            let s = "# nodes\n";
            let map = this.problem.map_label_text;
            for( let node of this.problem.diagram_direct[0]  ){
                s += map[node[0]] + " = " + node[1].map(x => this.problem.map_label_text[x]).join(" ") + "\n";
            }
            s += "# edges\n";
            for( let edge of this.problem.diagram_direct[1] ){
                s += map[edge[0]] + " -> " + map[edge[1]] + "\n";
            }
            copyToClipboard(s);
        }
    },
    watch : {
        'physics' : function() {
            if(this.network[0] != null){
                this.network[0].setOptions(this.options);
            }
        },
        'hierarchical' : function() {
            if(this.network[0] != null){
                this.network[0].setOptions(this.options);
            }
        },
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this));
            this.show_diagram();
        }
    },
    mounted: function() {
        this.show_diagram();
    },
    template: `
        <div> 
            <div class="panel-resizable" style="width: 300px; height: 300px;" :id="'diagram'+this._uid">
            </div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="physics"><p class="form-control-static custom-control-label">Physics</p></label><br/>
                <label><input type="checkbox" class="custom-control-input" v-model="hierarchical"><p class="form-control-static custom-control-label">Hierarchical</p></label>
            </div>
            <button type="button" class="btn btn-primary m-1" v-on:click="on_export">Export to Clipboard</button>
        </div>
    `
})


Vue.component('re-orientation-give',{
    props: ['problem','stuff'],
    data : function() {
        return {
            outdegree : 1
        }
    },
    methods: {
        on_orientation() {
            //console.log(this.outdegree)
            call_api_generating_problem(this.stuff,{type:"orientation", outdegree: this.outdegree},give_orientation,[this.problem, this.outdegree]);
        }
    },
    template: `
        <form class="form-inline m-2"><input class="form-control m-2" type="number" v-model="outdegree"><button type="button" class="btn btn-primary m-1" v-on:click="on_orientation">Fix Outdegree</button></form>
    `
})


Vue.component('re-speedup',{
    props: ['problem','stuff'],
    methods: {
        on_speedup() {
            call_api_generating_problem(this.stuff,{type:"speedup"},speedup,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_speedup">Speedup</button>
    `
})

Vue.component('re-demisifiable',{
    props: ['problem','stuff'],
    methods: {
        on_demisifiable() {
            call_api_generating_problem(this.stuff,{type:"demisifiable"},demisifiable,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_demisifiable">DeMISify</button>
    `
})

Vue.component('re-add-active-predecessors',{
    props: ['problem','stuff'],
    methods: {
        on_button() {
            call_api_generating_problem(this.stuff,{type:"add-active-predecessors"},add_active_predecessors,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_button">Add Active Predecessors</button>
    `
})

Vue.component('re-remove-trivial-lines',{
    props: ['problem','stuff'],
    methods: {
        on_button() {
            call_api_generating_problem(this.stuff,{type:"remove-trivial-lines"},remove_trivial_lines,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_button">Remove Trivial Lines</button>
    `
})


Vue.component('re-inverse-speedup',{
    props: ['problem','stuff'],
    methods: {
        on_speedup() {
            call_api_generating_problem(this.stuff,{type:"inversespeedup"},inverse_speedup,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_speedup">Inverse Speedup</button>
    `
})

Vue.component('re-all-different-labels',{
    props: ['problem','stuff'],
    methods: {
        on_click() {
            call_api_generating_problem(this.stuff,{type:"alldifferentlabels"},all_different_labels,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_click">All Different Labels</button>
    `
})

Vue.component('re-delta-edge-coloring',{
    props: ['problem','stuff'],
    methods: {
        on_speedup() {
            call_api_generating_problem(this.stuff,{type:"deltaedgecoloring"},delta_edge_coloring,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_speedup">Delta Edge Coloring</button>
    `
})


Vue.component('re-coloring',{
    props: ['problem','stuff'],
    methods: {
        on_click() {
            call_api_generating_problem(this.stuff,{type:"coloring"},compute_coloring_solvability,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_click">Coloring</button>
    `
})

Vue.component('re-marks',{
    props: ['problem','stuff'],
    methods: {
        on_click() {
            call_api_generating_problem(this.stuff,{type:"marks"},apply_marks_technique,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_click">Apply Marks</button>
    `
})

Vue.component('re-speedup-maximize',{
    props: ['problem','stuff'],
    methods: {
        on_speedup() {
            call_api_generating_problem(this.stuff,{type:"speedupmaximize"},speedupmaximize,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_speedup">Speedup+Maximize</button>
    `
})

Vue.component('re-speedup-maximize-rename',{
    props: ['problem','stuff'],
    methods: {
        on_speedup() {
            call_api_generating_problem(this.stuff,{type:"speedupmaximizerenamegen"},speedupmaximizerenamegen,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_speedup">Speedup+Maximize+Rename</button>
    `
})

Vue.component('re-rename-generators',{
    props: ['problem','stuff'],
    methods: {
        on_rename() {
            call_api_generating_problem(this.stuff,{type:"renamegenerators"},renamegenerators,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_rename">Rename by generators</button>
    `
})

Vue.component('re-rename',{
    props: ['problem','stuff'],
    data: function(){ return {
        table: this.problem.mapping_label_text.map(x => {
                let label = x[0];
                let text = x[1];
                let oldtext = this.problem.map_label_oldlabels == null ? null : labelset_to_string(this.problem.map_label_oldlabels[label],this.problem.map_oldlabel_text);
                let without_parenthesis = text.replace("(","").replace(")","");
                if( oldtext == null ) {
                    return [label,text,"",without_parenthesis];
                } else {
                    return [label,text,oldtext,without_parenthesis];
                }
        })
    }},
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_rename() {
            call_api_generating_problem(this.stuff,{type:"rename"},rename,[this.problem,this.table.map(x => [x[0],x[3]])]);
        },
        on_removegen() {
            for(let i=0;i<this.table.length;i++){
                if( this.table[i][3][0] == "<" && this.table[i][3][this.table[i][3].length-1] == ">" ) {
                    this.table[i][3] = this.table[i][3].substring(1,this.table[i][3].length-1);
                }
            }
            this.$forceUpdate();
        }
    },
    template: `
    <re-card title="New renaming" subtitle="(manually rename labels)">
        <table class="table">
            <tr v-for="(row,index) in this.table">
                <td class="align-middle" v-if="row[2]!=''"><span class="rounded m-1 labelborder">{{ row[2] }}</span></td>
                <td class="align-middle">{{ row[1] }}</td>
                <td class="align-middle"><input class="form-control" v-model="table[index][3]"></input></td>
            </tr>
        </table>
        <button type="button" class="btn btn-primary m-1" v-on:click="on_rename">Rename</button>
        <button type="button" class="btn btn-primary m-1" v-on:click="on_removegen">Remove &lt; &gt;</button>
    </re-card>
    `
})


Vue.component('re-maximize',{
    props: ['problem','stuff'],
    methods: {
        on_maximize() {
            call_api_generating_problem(this.stuff,{type:"maximize"},maximize,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_maximize">Maximize</button>
    `
})

Vue.component('re-fulldiagram',{
    props: ['problem','stuff'],
    methods: {
        on_fulldiagram() {
            call_api_generating_problem(this.stuff,{type:"fulldiagram"},fulldiagram,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_fulldiagram">Full Diagram</button>
    `
})

Vue.component('re-merge',{
    props: ['problem','stuff'],
    methods: {
        on_merge() {
            call_api_generating_problem(this.stuff,{type:"mergeequal"},merge_equivalent_labels,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-if="this.problem.info.is_mergeable" v-on:click="on_merge">Merge</button>
    `
})

Vue.component('re-edit',{
    props: ['problem'],
    methods: {
        on_edit() {
            this.$root.$emit('event_edit',[constraint_to_text(this.problem.active, this.problem.map_label_text),constraint_to_text(this.problem.passive, this.problem.map_label_text)])
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-1" v-on:click="on_edit">Edit</button>
    `
})

Vue.component('re-simplify',{
    props: ['problem','stuff'],
    data: function() {return {
        from : this.problem.labels[0],
        to : this.problem.labels[1]
    }},
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_merge() {
            call_api_generating_problem(
                this.stuff,
                {type:"simplificationmerge", from:this.problem.map_label_text[this.from], to : this.problem.map_label_text[this.to]},
                simplify_merge,[this.problem, this.from, this.to]
            );
        },
        on_addarrow() {
            call_api_generating_problem(
                this.stuff,
                {type:"simplificationaddarrow", from:this.problem.map_label_text[this.from], to : this.problem.map_label_text[this.to]},
                simplify_addarrow,[this.problem, this.from, this.to]
            );
        },
        on_from_diagram(){
            let selected = this.problem.selectedEdges;
            if( selected != null && selected.length > 0 ){
                this.from = selected[0][0];
                this.to = selected[0][1];
                //console.log("set from and to " + this.from + " " + this.to);
            }
        }
    },
    template: `
        <re-card title="Simplify" subtitle="(by merging or adding arrows)">
            <div>From: <re-label-picker :problem="this.problem" v-model="from"></re-label-picker>
            To: <re-label-picker :problem="this.problem" v-model="to"></re-label-picker></div>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_from_diagram">From diagram selection</button>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_merge">Merge</button>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_addarrow">Add Arrow</button>
        </re-card>
    `
})


Vue.component('re-harden-remove',{
    props: ['problem','stuff'],
    data : function() {return {
        label : 0,
        keep_predecessors : true
    }},
    methods: {
        on_remove() {
            call_api_generating_problem(
                this.stuff,
                {type:"hardenremove", label:this.problem.map_label_text[this.label]},
                harden_remove,[this.problem, this.label, this.keep_predecessors]
            );
        }
    },
    template: `
        <re-card title="Harden" subtitle="(by removing labels)">
            <re-label-picker :problem="this.problem" v-model="label"></re-label-picker>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="keep_predecessors"><p class="form-control-static custom-control-label">Replace With Predecessors</p></label>
            </div>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_remove">Remove</button>
        </re-card>
    `
})


Vue.component('re-critical',{
    props: ['problem','stuff'],
    data : function() {return {
        b_coloring : false,
        b_coloring_passive : false,
        b_maximize_rename : true,
        coloring : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.active.degree.Finite*(this.problem.passive.degree.Finite - 1) +1) : 4,
        coloring_passive : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.passive.degree.Finite*(this.problem.active.degree.Finite - 1) +1) : 4,
        zerosteps : 1,
        keep_predecessors : true
    }},
    methods: {
        on_harden() {
            call_api_generating_problem(
                this.stuff,
                {type:"criticalharden"},
                critical_harden,[this.problem, this.b_coloring, this.coloring, this.b_coloring_passive, this.coloring_passive, this.zerosteps, this.keep_predecessors, this.b_maximize_rename]
            );
        },
        on_relax() {
            call_api_generating_problem(
                this.stuff,
                {type:"criticalrelax"},
                critical_relax,[this.problem, this.b_coloring, this.coloring, this.b_coloring_passive, this.coloring_passive, this.zerosteps, this.b_maximize_rename]
            );
        }
    },
    template: `
        <re-card title="Critical Sets" subtitle="(harden or relax by using critical sets)">
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_coloring"><p class="form-control-static custom-control-label">A coloring is given</p></label>
            </div>
            <div v-if="this.b_coloring">Coloring: <input class="form-control m-2" type="number" v-model="coloring"></div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_coloring_passive"><p class="form-control-static custom-control-label">A coloring is given (passive side)</p></label>
            </div>
            <div v-if="this.b_coloring_passive">Coloring: <input class="form-control m-2" type="number" v-model="coloring_passive"></div>
            
            <div>Steps for checking triviality: <input class="form-control m-2" type="number" v-model="zerosteps"></div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="keep_predecessors"><p class="form-control-static custom-control-label">(On Harden) Replace With Predecessors</p></label>
            </div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_maximize_rename"><p class="form-control-static custom-control-label">Also maximize and rename by generators</p></label>
            </div>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_harden">Harden</button>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_relax">Relax</button>
        </re-card>
    `
})



Vue.component('re-group-simplify',{
    props: ['problem','stuff'],
    data: function(){ return {
        table: this.problem.mapping_label_text.map(x => {
                let label = x[0];
                let text = x[1];
                let oldtext = this.problem.map_label_oldlabels == null ? null : labelset_to_string(this.problem.map_label_oldlabels[label],this.problem.map_oldlabel_text);
                if( oldtext == null ) {
                    return [label,text,"",false];
                } else {
                    return [label,text,oldtext,false];
                }
        }),
        keep_predecessors : true,
        to : this.problem.labels[0]
    }},
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_merge(){
            let tomerge = this.table.filter(x => x[3]).map(x => x[0]);
            call_api_generating_problem(
                this.stuff,
                {type:"simplifymergegroup", labels:tomerge.map(x => this.problem.map_label_text[x]), to : this.problem.map_label_text[this.to]},
                simplify_group,[this.problem, tomerge, this.to]
            );
        },
        on_from_diagram(){
            let selected = this.problem.selected;
            if( selected != null ){
                for(let i = 0; i < this.table.length; i++){
                    let row = this.table[i];
                    row.splice(3,1,selected.includes(row[0]));
                    this.table.splice(i,1,row);
                }
            }
        }
    },
    template: `
        <re-card title="Group Simplify" subtitle="(choose a group of labels)">
            From:
            <div v-for="(row,index) in this.table">
                <div class="custom-control custom-switch ml-2">
                    <label>
                        <input type="checkbox" class="custom-control-input" v-model="table[index][3]">
                        <p class="form-control-static custom-control-label">
                            <span>{{ row[1] }}</span>
                            <span v-if="row[2]!=''" class="rounded m-1 labelborder">{{ row[2] }}</span>
                        </p>
                    </label>  
                </div>
            </div> 
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_from_diagram">From diagram selection</button>
            To: <re-label-picker :problem="this.problem" v-model="to"></re-label-picker>     
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_merge">Merge (simplify)</button>
        </re-card>
    `
})



Vue.component('re-sd-simplify',{
    props: ['problem','stuff'],
    data: function(){ return {
            text : "",
            recompute_diagram : true
        }    
    },
    methods: {
        on_sd(){
            call_api_generating_problem(
                this.stuff,
                {type:"simplifymergesd", sd:this.text},
                simplify_merge_sd,[this.problem, this.text,this.recompute_diagram]
            );
        },
        on_magic(){
            this.text = `t A new
c A in > 0
c A out == 1
c B out == 0
e A B
m A B

t A new
t B new
c A in > 0
c A out == 1
e A B
m A B`;
        }
    },
    template: `
        <re-card title="SubDiagram Merge" subtitle="(... still need a good description ...)">
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_magic"></button>
            <textarea rows="4" cols="30" class="form-control m-1" v-model="text"></textarea>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="recompute_diagram"><p class="form-control-static custom-control-label">Always Recompute Full Diagram</p></label>
            </div>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_sd">Merge</button>
        </re-card>
    `
})

Vue.component('re-group-harden',{
    props: ['problem','stuff'],
    data: function(){ return {
        table: this.problem.mapping_label_text.map(x => {
                let label = x[0];
                let text = x[1];
                let oldtext = this.problem.map_label_oldlabels == null ? null : labelset_to_string(this.problem.map_label_oldlabels[label],this.problem.map_oldlabel_text);
                if( oldtext == null ) {
                    return [label,text,"",false];
                } else {
                    return [label,text,oldtext,false];
                }
        }),
        keep_predecessors : true
    }},
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_remove() {
            let toremove = this.table.filter(x => x[3]).map(x => x[0]);
            let tokeep = this.problem.labels.filter(x => !toremove.includes(x));
            call_api_generating_problem(
                this.stuff,
                {type:"hardenkeep", labels:tokeep.map(x => this.problem.map_label_text[x])},
                harden_keep,[this.problem, tokeep, this.keep_predecessors]
            );
        },
        on_keep() {
            let tokeep = this.table.filter(x => x[3]).map(x => x[0]);
            call_api_generating_problem(
                this.stuff,
                {type:"hardenkeep", labels:tokeep.map(x => this.problem.map_label_text[x])},
                harden_keep,[this.problem, tokeep, this.keep_predecessors]
            );
        },
        on_from_diagram(){
            let selected = this.problem.selected;
            if( selected != null ){
                for(let i = 0; i < this.table.length; i++){
                    let row = this.table[i];
                    row.splice(3,1,selected.includes(row[0]));
                    this.table.splice(i,1,row);
                }
            }
        }
    },
    template: `
        <re-card title="Group Harden" subtitle="(choose a group of labels)">
            <div v-for="(row,index) in this.table">
                <div class="custom-control custom-switch ml-2">
                    <label>
                        <input type="checkbox" class="custom-control-input" v-model="table[index][3]">
                        <p class="form-control-static custom-control-label">
                            <span>{{ row[1] }}</span>
                            <span v-if="row[2]!=''" class="rounded m-1 labelborder">{{ row[2] }}</span>
                        </p>
                    </label>  
                </div>
            </div>  
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_from_diagram">From diagram selection</button>       
            <hr/>       
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="keep_predecessors"><p class="form-control-static custom-control-label">Replace With Predecessors</p></label>
            </div>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_remove">Remove (harden)</button>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_keep">Keep (harden)</button>
        </re-card>
    `
})

Vue.component('re-auto-lb',{
    props: ['problem','stuff'],
    data: function() {
        return {
            b_max_labels : false,
            b_branching : false,
            b_max_steps : false,
            max_labels : this.problem.labels.length + 4,
            max_steps : 15,
            branching : 4,
            coloring_given : false,
            coloring : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.active.degree.Finite*(this.problem.passive.degree.Finite - 1) +1) : 4,
            coloring_given_passive : false,
            coloring_passive : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.passive.degree.Finite*(this.problem.active.degree.Finite - 1) +1) : 4,
        }
    },
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_autolb() {
            call_api_generating_sequence(this.stuff,{type:"autolb"},autolb,[this.problem, this.b_max_labels, this.max_labels, this.b_branching, this.branching, this.b_max_steps, this.max_steps, this.coloring_given, this.coloring, this.coloring_given_passive, this.coloring_passive], false);
        },
    },
    template: `
        <re-card title="Automatic Lower Bound" subtitle="">
            <div v-if="this.problem.active.degree.Finite > 2 && this.problem.passive.degree.Finite > 2">On Hypergraphs, coloring here refers to strong coloring.</div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="coloring_given"><p class="form-control-static custom-control-label">A coloring is given</p></label>
            </div>
            <div v-if="this.coloring_given">Coloring: <input class="form-control m-2" type="number" v-model="coloring"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="coloring_given_passive"><p class="form-control-static custom-control-label">A coloring is given (passive side)</p></label>
            </div>
            <div v-if="this.coloring_given_passive">Coloring: <input class="form-control m-2" type="number" v-model="coloring_passive"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_max_labels"><p class="form-control-static custom-control-label">Manually set Max Labels</p></label>
            </div>
            <div v-if="this.b_max_labels">Max Labels: <input class="form-control m-2" type="number" v-model="max_labels"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_branching"><p class="form-control-static custom-control-label">Manually set Branching</p></label>
            </div>
            <div v-if="this.b_branching">Branching: <input class="form-control m-2" type="number" v-model="branching"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_max_steps"><p class="form-control-static custom-control-label">Manually set Max Steps</p></label>
            </div>
            <div v-if="this.b_max_steps">Max Steps: <input class="form-control m-2" type="number" v-model="max_steps"></div>

            <button type="button" class="btn btn-primary m-2" v-on:click="on_autolb">Automatic Lower Bound</button>
        </re-card>
    `
})

Vue.component('re-auto-ub',{
    props: ['problem','stuff'],
    data: function() {
        return {
            b_max_labels : false,
            b_branching : false,
            b_max_steps : false,
            max_labels : this.problem.labels.length + 4,
            branching : 4,
            max_steps : 8,
            coloring_given : false,
            coloring : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.active.degree.Finite*(this.problem.passive.degree.Finite - 1) +1) : 4,
            coloring_given_passive : false,
            coloring_passive : (this.problem.active.degree.Finite != null && this.problem.passive.degree.Finite != null) ? (this.problem.passive.degree.Finite*(this.problem.active.degree.Finite - 1) +1) : 4,
        }
    },
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_autoub() {
            call_api_generating_sequence(this.stuff,{type:"autoub"},autoub,[this.problem, this.b_max_labels, this.max_labels, this.b_branching, this.branching, this.b_max_steps, this.max_steps, this.coloring_given, this.coloring, this.coloring_given_passive, this.coloring_passive], false);
        },
    },
    template: `
        <re-card title="Automatic Upper Bound" subtitle="">
            <div v-if="this.problem.active.degree.Finite > 2 && this.problem.passive.degree.Finite > 2">On Hypergraphs, coloring here refers to strong coloring.</div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="coloring_given"><p class="form-control-static custom-control-label">A coloring is given</p></label>
            </div>
            <div v-if="this.coloring_given">Coloring: <input class="form-control m-2" type="number" v-model="coloring"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="coloring_given_passive"><p class="form-control-static custom-control-label">A coloring is given (passive side)</p></label>
            </div>
            <div v-if="this.coloring_given_passive">Coloring: <input class="form-control m-2" type="number" v-model="coloring_passive"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_max_labels"><p class="form-control-static custom-control-label">Manually set Max Labels</p></label>
            </div>
            <div v-if="this.b_max_labels">Max Labels: <input class="form-control m-2" type="number" v-model="max_labels"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_branching"><p class="form-control-static custom-control-label">Manually set Branching</p></label>
            </div>
            <div v-if="this.b_branching">Branching: <input class="form-control m-2" type="number" v-model="branching"></div>

            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="b_max_steps"><p class="form-control-static custom-control-label">Manually set Max Steps</p></label>
            </div>
            <div v-if="this.b_max_steps">Max Steps: <input class="form-control m-2" type="number" v-model="max_steps"></div>

            <button type="button" class="btn btn-primary m-2" v-on:click="on_autoub">Automatic Upper Bound</button>
        </re-card>
    `
})

Vue.component('re-operations',{
    props: ['problem','stuff'],
    template: `
        <re-card title="Operations" subtitle="(speedup, maximize, edit, gen renaming, merge)">
            <div class="m-2"><re-speedup :problem="problem" :stuff="stuff"></re-speedup> apply round elimination</div>
            <div class="m-2"><re-maximize :problem="problem" :stuff="stuff"></re-maximize> maximize passive side (and compute full diagram, triviality, ...)</div>
            <div class="m-2"><re-fulldiagram :problem="problem" :stuff="stuff"></re-fulldiagram> compute full diagram without showing maximized passive side </div>
            <div class="m-2" v-if="this.problem.info.is_mergeable"><re-merge :problem="problem" :stuff="stuff"></re-merge>merge equivalent labels</div>
            <div class="m-2"><re-edit :problem="problem" :stuff="stuff"></re-edit>copy problem up</div>
            <div class="m-2"><re-inverse-speedup :problem="problem" :stuff="stuff"></re-inverse-speedup> apply inverse round elimination</div>
            <div class="m-2"><re-all-different-labels :problem="problem" :stuff="stuff"></re-all-different-labels> make each label different</div>
            <div class="m-2"><re-delta-edge-coloring :problem="problem" :stuff="stuff"></re-delta-edge-coloring> transform labels assuming a delta edge coloring</div>
            <div class="m-2" v-if="this.problem.mapping_label_oldlabels != null"><re-rename-generators :problem="problem" :stuff="stuff"></re-rename-generators>rename by using diagram generators</div>
            <div class="m-2"><re-speedup-maximize :problem="problem" :stuff="stuff"></re-speedup-maximize><re-speedup-maximize-rename :problem="problem" :stuff="stuff"></re-speedup-maximize-rename></div>
            <re-orientation-give :problem="problem" :stuff="stuff"></re-orientation-give>
            <div class="m-2" v-if="this.problem.info.numcolors == -1"><re-coloring :problem="problem" :stuff="stuff"></re-coloring> compute hypergraph strong coloring solvability</div>
            <div class="m-2"><re-marks :problem="problem" :stuff="stuff"></re-marks> apply Marks' technique</div>
            <div class="m-2"><re-demisifiable :problem="problem" :stuff="stuff"></re-demisifiable> compute reversible merges</div>
            <div class="m-2"><re-add-active-predecessors :problem="problem" :stuff="stuff"></re-add-active-predecessors> ...</div>
            <div class="m-2"><re-remove-trivial-lines :problem="problem" :stuff="stuff"></re-remove-trivial-lines> ...</div>
        </re-card>
    `
})

Vue.component('re-tools', {
    props: ["problem","stuff"],
    computed: {
        
    },
    template: `
        <div>
            <re-operations :problem="problem" :stuff="stuff"></re-operations>
            <re-simplify :problem="problem" :stuff="stuff"></re-simplify>
            <re-group-simplify :problem="problem" :stuff="stuff"></re-group-simplify>
            <re-sd-simplify :problem="problem" :stuff="stuff"></re-sd-simplify>
            <re-harden-remove :problem="problem" :stuff="stuff"></re-harden-remove>
            <re-group-harden :problem="problem" :stuff="stuff"></re-group-harden>
            <re-rename :problem="problem" :stuff="stuff"></re-rename>
            <re-fixpoint :problem="problem" :stuff="stuff"></re-fixpoint>
            <re-critical :problem="problem" :stuff="stuff"></re-critical>
            <re-auto-lb :problem="problem" :stuff="stuff"></re-auto-lb>
            <re-auto-ub :problem="problem" :stuff="stuff"></re-auto-ub>
            <re-zero-input :problem="problem" :stuff="stuff"></re-zero-input>
            <re-dual :problem="problem" :stuff="stuff"></re-dual>
        </div>
    `
})



Vue.component('re-problem', {
    props: ["problem","stuff","handle"],
    data : function() {
        return {
            mode : "renamed",
            mode2 : "all"
        }
    },
    methods: {
        on_close() {
            let idx = this.stuff.indexOf(this.handle);
            this.stuff.splice(idx,1);
        }
    },
    template: `
        <div class="card card-body m-2 p-2 bg-light position-relative">
            <div class="row p-0 m-0">
                <div v-if="this.problem.mapping_label_oldlabels != null">
                    <div class="btn-group btn-group-toggle pt-3 pl-3" data-toggle="buttons">
                        <label class="btn btn-primary active">
                            <input type="radio" name="options" autocomplete="off" value="renamed" v-model="mode">New</label>
                        <label class="btn btn-primary">
                            <input type="radio" name="options" autocomplete="off" value="original" v-model="mode">Old</label>
                        <label class="btn btn-primary">
                            <input type="radio" name="options" autocomplete="off" value="both" v-model="mode">Both</label>
                    </div>
                </div>
                <div>
                    <div class="btn-group btn-group-toggle pt-3 pl-3" data-toggle="buttons">
                        <label class="btn btn-primary active">
                            <input type="radio" name="options" autocomplete="off" value="all" v-model="mode2">All</label>
                        <label class="btn btn-primary">
                            <input type="radio" name="options" autocomplete="off" value="gen" v-model="mode2">Gen</label>
                    </div>
                </div>
                <div/>
                <button type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
            </div>
            <re-problem-info :problem="this.problem"></re-problem-info>
            <div class="row p-0 m-2 align-items-start">
                <re-card title="Active" subtitle="Any choice satisfies previous Passive" show="true">
                    <re-constraint side="active" :mode="this.mode" :problem="this.problem"></re-constraint>
                </re-card>
                <re-card title="Passive" subtitle="Exists choice satisfying previous Active" show="true">
                    <re-constraint side="passive" :mode="this.mode" :mode2="this.mode2" :problem="this.problem"></re-constraint>
                </re-card>
                <re-card title="Renaming" subtitle="Old and new labels" show="true" v-if="this.problem.mapping_label_oldlabels != null">
                    <re-renaming :problem="problem"></re-renaming>
                </re-card>
                <re-card title="Renaming" subtitle="Old and new labels" show="true" v-if="this.problem.mapping_oldlabel_labels != null">
                    <re-inverse-renaming :problem="problem"></re-inverse-renaming>
                </re-card>
                <re-card :title="this.problem.passive.is_maximized ? 'Diagram' : 'Partial Diagram'" subtitle="Strength of passive labels" show="true" v-if="this.problem.diagram_direct != null">
                    <re-diagram :problem="problem"></re-diagram>
                </re-card>
                <re-card title="Tools" subtitle="Speedup, edit, simplifications, ..." show="true">
                    <re-tools :problem="problem" :stuff="stuff"></re-tools>
                </re-card>
            </div>
        </div>
    `
})

Vue.component('re-label-picker', {
    props: ["problem", "value"],
    computed : {
        data: function() {
            return this.problem.mapping_label_text.map(x => {
                let label = x[0];
                let text = x[1];
                let oldtext = this.problem.map_label_oldlabels == null ? null : labelset_to_string(this.problem.map_label_oldlabels[label],this.problem.map_oldlabel_text);
                if( oldtext == null ) {
                    return [label,text];
                } else {
                    return [label,text,oldtext];
                }
            })
        }
    },
    watch : {
        'value' : function() {
            //console.log("value changed to " + this.value );
            $(this.$el).val(this.value);
            $(this.$el).selectpicker('refresh');
        }
    },
    mounted: function() {
        let id = "#select" + this._uid;
        let t = this;
        $(this.$el).selectpicker();
        $(this.$el).on('change', function(){
            t.$emit('input', $(id).val())
        });
    },
    beforeDestroy: function() {
        $(this.$el).selectpicker('destroy');
    },
    template: `
    <select class="selectpicker" data-live-search="true" v-bind:value="value" :id="'select'+this._uid">
        <option v-for="x in this.data" :value="x[0]" :data-tokens="x.length==3?'['+x[2]+'] → '+x[1]:x[1]">
            <template v-if="x.length == 3">[{{ x[2] }}] → </template>{{ x[1] }}
        </option>
    </select>
    `
})


Vue.component('re-begin', {
    props: ["all"],
    data : function(){ return {
            active : this.all.active,
            passive : this.all.passive
        }
    },
    computed : {
        stuff: function() {
            return this.all.stuff;
        }
    },
    methods: {
        on_start() {
            call_api_generating_problem(this.stuff,{type:"initial"},new_problem,[this.active, this.passive]);
        },
        on_clear() {
            this.stuff.splice(0)
        }
    },
    mounted: function(){
        this.$root.$on('event_edit', x => {
            this.active = x[0];
            this.passive = x[1];
        })
    },
    template: `
    <div class="container-fluid m-0 p-0">
    <div class="container-fluid rounded bg-light m-0 mb-5 p-5">
        <div class="row">
            <div class="col-md">
                <h4>Active</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="active"></textarea>
            </div>
            <div class="col-md">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="passive"></textarea>
            </div>
            <div class="m-2 col-sm mt-auto text-right">
                <button type="button" class="btn btn-primary" v-on:click="on_start">Start</button>
                <button type="button" class="btn btn-primary" v-on:click="on_clear">Clear</button>
                <re-export :stuff="stuff" :active="active" :passive="passive"></re-export>
            </div>
        </div>
    </div>

    <div class="container-fluid m-0 p-0" id="steps"></div>
</div>
    `
})


Vue.component('re-fixpoint',{
    props: ['problem','stuff'],
    data: function(){ return {
        table: this.problem.mapping_label_text.map(x => {
                let enabled = this.problem.fixpoint_diagram !== null && this.problem.fixpoint_diagram[0] !== null && this.problem.fixpoint_diagram[0].indexOf(x[0]) !== -1;
                let label = x[0];
                let text = x[1];
                let oldtext = this.problem.map_label_oldlabels == null ? null : labelset_to_string(this.problem.map_label_oldlabels[label],this.problem.map_oldlabel_text);
                if( oldtext == null ) {
                    return [label,text,"",enabled];
                } else {
                    return [label,text,oldtext,enabled];
                }
        }),
        partial : this.problem.fixpoint_diagram !== null && this.problem.fixpoint_diagram[0] !== null,
        triviality_only : false
    }},
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_from_diagram(){
            let selected = this.problem.selected;
            if( selected != null ){
                for(let i = 0; i < this.table.length; i++){
                    let row = this.table[i];
                    row.splice(3,1,selected.includes(row[0]));
                    this.table.splice(i,1,row);
                }
            }
        }
    },
    template: `
        <re-card :show="this.problem.fixpoint_diagram !== null" title="Fixed Point Tools" subtitle="(automatic procedure for fixed point generation)">
            <div class="custom-control custom-switch ml-2">
                <label>
                    <input type="checkbox" class="custom-control-input" v-model="partial">
                    <p class="form-control-static custom-control-label">
                        <span class="rounded m-1 labelborder">Partial Fixpointing</span>
                    </p>
                </label>  
            </div>
            <div class="custom-control custom-switch ml-2">
                <label>
                    <input type="checkbox" class="custom-control-input" v-model="triviality_only">
                    <p class="form-control-static custom-control-label">
                        <span class="rounded m-1 labelborder">Only determine triviality</span>
                    </p>
                </label>  
            </div>
            <hr/>
            <div v-if="partial">
                <div v-for="(row,index) in this.table">
                    <div class="custom-control custom-switch ml-2">
                        <label>
                            <input type="checkbox" class="custom-control-input" v-model="table[index][3]">
                            <p class="form-control-static custom-control-label">
                                <span>{{ row[1] }}</span>
                                <span v-if="row[2]!=''" class="rounded m-1 labelborder">{{ row[2] }}</span>
                            </p>
                        </label>  
                    </div>
                </div>
                <button type="button" class="btn btn-primary ml-2" v-on:click="on_from_diagram">From diagram selection</button>       
                <hr/>
            </div>
            <div class="m-2"><re-fixpoint-basic :problem="problem" :stuff="stuff" :partial="partial" :table="table" :triviality_only="triviality_only"></re-fixpoint-basic> (with default diagram)</div>
            <div class="m-2"><re-fixpoint-loop :problem="problem" :stuff="stuff" :partial="partial" :table="table"  :triviality_only="triviality_only"></re-fixpoint-loop> (with default diagram, automatic fixing)</div>
            <div v-if="this.problem.fixpoint_diagram === null" class="m-2">
                <re-fixpoint-gendefault :problem="problem" :stuff="stuff" :partial="partial" :table="table"  :triviality_only="triviality_only"></re-fixpoint-gendefault> for additional options, click here
            </div>
            <div v-else class="m-2">
                <re-fixpoint-dup :problem="problem" :stuff="stuff" :partial="partial" :table="table"  :triviality_only="triviality_only"></re-fixpoint-dup>
            </div>
            <re-fixpoint-custom :problem="problem" :stuff="stuff" :partial="partial" :table="table"  :triviality_only="triviality_only"></re-fixpoint-custom>
        </re-card>
    `
})

Vue.component('re-fixpoint-gendefault',{
    props: ['problem','stuff','partial','table','triviality_only'],
    methods: {
        on_fixpoint() {
            let sublabels = this.partial? this.table.filter(x => x[3]).map(x => x[0]) : [];
            let sublabels_text = this.partial? labelset_to_string(sublabels,this.problem.map_label_text) : null;
            call_api_generating_problem(this.stuff,{type:"fixpoint-gendefault", sub : sublabels_text},fixpoint_gendefault,[this.problem,this.partial,this.triviality_only, sublabels]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Generate Default Diagram</button>
    `
})

Vue.component('re-fixpoint-basic',{
    props: ['problem','stuff','partial','table','triviality_only'],
    methods: {
        on_fixpoint() {
            let sublabels = this.partial? this.table.filter(x => x[3]).map(x => x[0]) : [];
            let sublabels_text = this.partial? labelset_to_string(sublabels,this.problem.map_label_text) : null;
            call_api_generating_problem(this.stuff,{type:"fixpoint-basic",sub : sublabels_text},fixpoint_basic,[this.problem,this.partial,this.triviality_only, sublabels]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Basic</button>
    `
})

Vue.component('re-fixpoint-loop',{
    props: ['problem','stuff','partial','table','triviality_only'],
    methods: {
        on_fixpoint() {
            let sublabels = this.partial? this.table.filter(x => x[3]).map(x => x[0]) : [];
            let sublabels_text = this.partial? labelset_to_string(sublabels,this.problem.map_label_text) : null;
            call_api_generating_problem(this.stuff,{type:"fixpoint-loop",sub : sublabels_text},fixpoint_loop,[this.problem,this.partial,this.triviality_only, sublabels]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Loop</button>
    `
})

Vue.component('re-fixpoint-custom',{
    props: ['problem','stuff','partial','table','triviality_only'],
    data: function(){ 
        if( this.problem.fixpoint_diagram != null ){
            return {
                text : this.problem.fixpoint_diagram[1].text,
            } 
        } else {
            let s = "# mapping from original labels to diagram labels\n";
            for( let node of this.problem.diagram_direct[0] ){
                for( let label of node[1] ){
                    s += this.problem.map_label_text[label] + " = " + this.problem.map_label_text[label] + "\n";
                }            
            }
            s += "# diagram edges\n";
            for( let edge of this.problem.diagram_direct[1] ){
                s += this.problem.map_label_text[edge[0]] + " -> " + this.problem.map_label_text[edge[1]] + "\n";
            }
            return {
                text : s,
            }  
        } 
    },
    watch: { 
        // for some unknown reason, vue updates the template values when the prop "problem" changes, but it does not update the values of the variables contained in "data"
        // this is a workaround
        problem: function(newVal, oldVal) { 
            Object.assign(this.$data, this.$options.data.apply(this))
        }
    },
    methods: {
        on_fixpoint() {
            let sublabels = this.partial? this.table.filter(x => x[3]).map(x => x[0]) : [];
            let sublabels_text = this.partial? labelset_to_string(sublabels,this.problem.map_label_text) : null;
            call_api_generating_problem(this.stuff,{type:"fixpoint-custom",sub : sublabels_text, diagram: this.text},fixpoint_custom,[this.problem,this.text,this.partial,this.triviality_only, sublabels]);
        }
    },
    template: `
        <re-card show="true" title="With Custom Diagram" subtitle="(you can edit the diagram)">
            <textarea rows="4" cols="30" class="form-control m-1" v-model="text"></textarea>
            <button type="button" class="btn btn-primary m-1" v-on:click="on_fixpoint">Generate Fixed Point</button>
        </re-card>
    `
})


Vue.component('re-fixpoint-dup',{
    props: ['problem','stuff','partial','table','triviality_only'],
    data : function(){ return {
        dups : [],  
    }},
    methods: {
        on_delete(index) {
            this.dups.splice(index, 1);
        },
        on_from_diagram(){
            let selected = this.problem.fixpoint_diagram[1].selected;
            if( selected != null && selected.length > 0 ){
                this.dups.push(selected)
            }
        },
        on_fixpoint() {
            let sublabels = this.partial? this.table.filter(x => x[3]).map(x => x[0]) : [];
            let sublabels_text = this.partial? labelset_to_string(sublabels,this.problem.map_label_text) : null;
            call_api_generating_problem(this.stuff,{type:"fixpoint-dup", sub : sublabels_text, dups: "["+this.dups.map(x => "["+this.convert(x)+"]").join(",")+"]"},fixpoint_dup,[this.problem, this.dups, this.partial, this.triviality_only, sublabels]);
        },
        convert(x){
            return labelset_to_string(x,this.problem.fixpoint_diagram[1].map_label_text,", ")
        }
    },
    template: `
        <re-card show="true" title="With Label Duplication" subtitle="(choose groups of labels to duplicate)">
            <re-diagram :problem="problem.fixpoint_diagram[1]"></re-diagram>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_from_diagram">Add from diagram selection</button>
            <button type="button" class="btn btn-secondary m-1" data-toggle="tooltip" data-placement="top" title="A good strategy could be the following. First run the basic procedure. Then take the lines that contribute to the 0 round solvability. Take each label L appearing there. L is a set of original labels. For each l ∊ L, take the set of labels that are edge-compatible with it. Take the intersection of the obtained sets. This gives a label L' of the diagram. Take all labels in the shortest path between L and L' and create a group of labels to duplicate.">?</button>
            <table class="table">
                <tr v-for="(group, index) in dups" :key="index">
                    <td><div class="m-1"><span>{{ convert(group) }}</span></div></td>
                    <td>
                        <button type="button" class="btn btn-primary btn-sm" v-on:click="on_delete(index)">Delete</button>
                    </td>
                </tr>
            </table>
            <button type="button" class="btn btn-primary m-1" v-on:click="on_fixpoint">Generate Fixed Point</button>
        </re-card>
    `
})


Vue.component('re-zero-input',{
    props: ['problem','stuff'],
    data: function(){ return {
            active : "",
            passive : ""
        }    
    },
    methods: {
        on_zero(){
            call_api_generating_problem(
                this.stuff,
                {type:"zerowithinput", active:this.active,passive:this.passive, reverse : false},
                check_zero_with_input,[this.problem, this.active,this.passive, false]
            );
        },
        on_zero_reverse(){
            call_api_generating_problem(
                this.stuff,
                {type:"zerowithinput", active:this.active,passive:this.passive, reverse : true},
                check_zero_with_input,[this.problem, this.active,this.passive, true]
            );
        },
    },
    template: `
        <re-card title="Zero-Round Solvability with Input" subtitle="(check if the given input makes the problem trivial)">
            <div class="m-1">
                <h4>Active</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="active"></textarea>
            </div>
            <div class="m-1">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="passive"></textarea>
            </div>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_zero">Check</button>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_zero_reverse">Reverse Check</button>
        </re-card>
    `
})


Vue.component('re-dual',{
    props: ['problem','stuff'],
    data: function(){ return {
            dual_fp_active : "",
            dual_fp_passive : "",
            doubledual_fp_active : "",
            doubledual_fp_passive : "",  
            doubledual_fp_diagram : "",
            input_active : "",
            input_passive : "",
        }    
    },
    methods: {
        on_dual(){
            call_api_generating_problem(
                this.stuff,
                {type:"dual", active:this.dual_fp_active,passive:this.dual_fp_passive},
                dual,[this.problem, this.dual_fp_active,this.dual_fp_passive]
            );
        },
        on_doubledual(){
            call_api_generating_problem(
                this.stuff,
                {type:"doubledual", active:this.dual_fp_active,passive:this.dual_fp_passive},
                doubledual,[this.problem, this.dual_fp_active,this.dual_fp_passive]
            );
        },
        on_doubledual2(){
            call_api_generating_problem(
                this.stuff,
                {type:"doubledual2", active:this.doubledual_fp_active,passive:this.doubledual_fp_passive,diagram:this.doubledual_fp_diagram,input_active:this.input_active, input_passive:this.input_passive},
                doubledual2,[this.problem, this.doubledual_fp_active,this.doubledual_fp_passive,this.doubledual_fp_diagram,this.input_active, this.input_passive]
            );
        },
    },
    template: `
        <re-card title="Dual" subtitle="(compute dual and double dual)">
            The dual is computed according to the definition.<br/>
            The double dual is computed by using the fp procedure.<br/>
            Dual (or double dual) w.r.t. the following fixed point.
            <div class="m-1">
                <h4>Active</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="dual_fp_active"></textarea>
            </div>
            <div class="m-1">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="dual_fp_passive"></textarea>
            </div>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_dual">Dual</button>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_doubledual">Double Dual</button>
            <hr/>
            Double dual computed by using the fp procedure (the dual computation is skipped).<br/> You need to provide the fixed point, or the diagram of the fixed point. 
            <div class="m-1">
                <h4>Active</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="doubledual_fp_active"></textarea>
            </div>
            <div class="m-1">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="doubledual_fp_passive"></textarea>
            </div>
            <div class="m-1">
                <h4>Fixed Point Diagram</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="doubledual_fp_diagram"></textarea>
            </div>
            Optional: custom input.
            <div class="m-1">
                <h4>Active</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="input_active"></textarea>
            </div>
            <div class="m-1">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" style="resize: both" v-model="input_passive"></textarea>
            </div>
            <button type="button" class="btn btn-primary ml-1" v-on:click="on_doubledual2">Double Dual</button>
        </re-card>
    `
})


// https://stackoverflow.com/questions/400212/how-do-i-copy-to-the-clipboard-in-javascript
// navigator.clipboard.writeText(link); only works with HTTPS
function copyToClipboard(text) {
    var textArea = document.createElement("textarea");
    textArea.value = text;    
    textArea.style.top = "0";
    textArea.style.left = "0";
    textArea.style.position = "fixed";
  
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    document.execCommand('copy');
    document.body.removeChild(textArea);
}

Vue.component('re-export', {
    props: ["stuff","active","passive"],
    methods: {
        generate_export_data : function() {
            let redata = {active:this.active,passive:this.passive,stuff:this.stuff};
            let data = {v:version, d : redata};
            let json = JSON.stringify(data);
            let compressed = pako.deflateRaw(json,{level:9});
            //console.log("original length: " +json.length);
            //console.log("compressed length: " +compressed.length);
            //console.log(compressed);
            // https://github.com/WebReflection/uint8-to-base64/blob/master/index.js
            var v = [];
            for (var i = 0, length = compressed.length; i < length; i++) {
                v.push(String.fromCharCode(compressed[i]));
            }
            let s = v.join("");
            let b64 =  btoa(s);
            return b64;
        },
        on_share : function() {
            let text = this.generate_export_data();
            //console.log(s);
            
            let uri = window.location.href.split("#")[0];
            let link = uri + '#' + text;
            //navigator.clipboard.writeText(link);
            copyToClipboard(link);
        },
        on_save : function() {
            let text = this.generate_export_data();
            var element = document.createElement('a');
            element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(text));
            element.setAttribute('download', "problem.re");
            element.style.display = 'none';
            document.body.appendChild(element);
            element.click();
            document.body.removeChild(element);
        },
        on_load : function() {
            this.file = this.$refs.doc.files[0];
            const reader = new FileReader();
            reader.onload = (res) => {
                this.content = res.target.result;
                //console.log(this.content);
                this.$root.$emit('event_load',this.content);
            };
            reader.onerror = (err) => alert("The file appears to be invalid");
            reader.readAsText(this.file);
        },
        trigger_load : function() {
            document.getElementById("upfile").click();
        }
    },
    template: `
        <div class="mt-1">
            <button type="button" class="btn btn-primary" v-on:click="on_share">Export Link To Clipboard</button>
            <button type="button" class="btn btn-primary" v-on:click="on_save">Save</button>
            <button type="button" class="btn btn-primary" v-on:click="trigger_load">Load</button>
            <div style='height: 0px;width:0px; overflow:hidden;'><input type="file" accept=".re" id="upfile" ref="doc" @change="on_load()"/></div>            
        </div>
    `
})

Vue.component('re-stuff', {
    props: ["stuff","handle","supstuff"],
    methods: {
        on_close() {
            let idx = this.supstuff.indexOf(this.handle);
            this.supstuff.splice(idx,1);
        }
    },
    template: `
        <div>
            <div class="card bg-light pr-4 m-2" v-if="this.supstuff != null">
                <button type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
                <div v-for="elem in this.stuff">
                    <re-performed-action :stuff="stuff" :action='elem.data' v-if='elem.type == "performed"'  :handle="elem"/></re-performed-action>
                    <re-computing :action='elem.data' v-if='elem.type == "computing"'  :handle="elem"/></re-computing>
                    <re-error :stuff="stuff" :error='elem.data' :warning='elem.warning' v-if='elem.type == "error"'  :handle="elem"/></re-error>
                    <re-problem :problem='elem.data' :stuff='stuff' v-if='elem.type == "problem"' :handle="elem"></re-problem>
                    <re-stuff :supstuff='stuff' :stuff='elem.data' v-if='elem.type == "sub"' :handle="elem"></re-stuff>
                </div>
            </div>
            <div v-if="this.supstuff == null">
                <button type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
                <div v-for="elem in this.stuff">
                    <re-performed-action :stuff="stuff" :action='elem.data' v-if='elem.type == "performed"'  :handle="elem"/></re-performed-action>
                    <re-computing :action='elem.data' v-if='elem.type == "computing"'  :handle="elem"/></re-computing>
                    <re-error :stuff="stuff" :error='elem.data' :warning='elem.warning' v-if='elem.type == "error"'  :handle="elem"/></re-error>
                    <re-problem :problem='elem.data' :stuff='stuff' v-if='elem.type == "problem"' :handle="elem"></re-problem>
                    <re-stuff :supstuff='stuff' :stuff='elem.data' v-if='elem.type == "sub"' :handle="elem"></re-stuff>
                </div>
            </div>
        </div>
    `
})


function init_data(x) {
    if( window.location.hash && x == undefined ) {
        x =  window.location.hash.substring(1);
    }
    if( x !== undefined ) {
        try{
            let b64 = x;
            let s = atob(b64);
            let compressed = Uint8Array.from(s, c => c.charCodeAt(0));
            let json = pako.inflateRaw(compressed, { to: 'string' });
            let data = JSON.parse(json);
            if( data.v != version ){
                alert("Sorry, the link comes from a different version of REtor");
                return def;
            }
            return data.d;
        }catch(error){
            alert("The file appears to be invalid");
        }
    }
    let def = {active:"M U^9\nP^10",passive:"M UP^9\nU^10",stuff:[]};
    return def;
}

var app = new Vue({
    el: '#vueapp',
    data : {
        all : init_data(),
    },
    mounted: function(){
        this.$root.$on('event_load', x => {
            this.all = init_data(x);
        })
    },
    template: `
        <div>
            <re-begin :all="all"></re-begin>
            <re-stuff :stuff="all.stuff"></re-stuff>
        </div>
    `
})


