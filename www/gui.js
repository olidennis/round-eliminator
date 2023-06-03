
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

function fixpoint_gendefault(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ DefaultDiagram : problem }, ondata , function(){});
}

function fixpoint_basic(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointBasic : problem }, ondata , function(){});
}

function fixpoint_loop(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointLoop : problem }, ondata , function(){});
}

function fixpoint_custom(problem, diagram, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointCustom : [problem, diagram] }, ondata , function(){});
}

function fixpoint_dup(problem, dups, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ FixpointDup : [problem, dups] }, ondata , function(){});
}

function give_orientation(problem, outdegree, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Orientation : [problem,parseInt(outdegree)] }, ondata , function(){});
}

function inverse_speedup(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ InverseSpeedup : problem }, ondata , function(){});
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

function renamegenerators(problem, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ RenameGenerators : problem }, ondata , function(){});
}

function rename(problem, renaming, onresult, onerror, progress){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ Rename : [problem,renaming] }, ondata , function(){});
}

function autoub(problem, max_labels, branching, max_steps, allow_discard_old, onresult, onerror, progress, oncomplete){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AutoUb : [problem, parseInt(max_labels), parseInt(branching), parseInt(max_steps), allow_discard_old] }, ondata, oncomplete);
}

function autoautoub(problem, allow_discard_old, onresult, onerror, progress, oncomplete){
    let ondata = x => handle_result(x, onresult, onerror, progress);
    return api.request({ AutoAutoUb : [problem, allow_discard_old] }, ondata, oncomplete);
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
    if( p.fixpoint_diagram !== null ){
        p.fixpoint_diagram.map_label_text = vec_to_map(p.fixpoint_diagram.mapping_newlabel_text);

    }
    p.info = { orientation_coloringsets:orientation_coloringsets, orientation_numcolors:orientation_numcolors, orientation_zerosets:orientation_zerosets,orientation_is_zero:orientation_is_zero, orientation_is_nonzero:orientation_is_nonzero, numlabels : numlabels, is_zero : is_zero, is_nonzero : is_nonzero, numcolors : numcolors, zerosets : zerosets, coloringsets : coloringsets, is_mergeable : is_mergeable, mergesets : mergesets };
}



function on_new_what(stuff, action, progress, p, what, removeprogress = true){
    let idx = stuff.indexOf(progress);
    if( removeprogress ){
        stuff.splice(idx,1);
    }
    if( what == "problem" ){
        stuff.push({ type : "performed", data: action });
        stuff.push({ type : "problem", data : p });
    }else if( what == "sequence" ){
        let len = p[0];
        let sequence = p[1];
        let substuff = [];
        action.len = len;
        substuff.push({ type : "performed", data: action });
        for( var step of sequence ){
            let operation = step[0];
            if( operation == "Initial" ){
                substuff.push({ type : "performed", data: {type:"initial"} });
            } else if( operation == "Speedup" ){
                substuff.push({ type : "performed", data: {type:"speedup"} });
            } else if( operation.Harden != null) {
                substuff.push({ type : "performed", data: {type:"hardenkeep", labels:operation.Harden.map(x => step[1].map_label_text[x])} });
            }
            substuff.push({ type : "problem", data : step[1] });
        }
        stuff.push({ type : "sub", data : substuff });
    }
}


function call_api_generating_problem(stuff, action, f, params, removeprogress = true) {
    return call_api_generating_what(stuff, action, f, params, "problem", removeprogress);
}

function call_api_generating_sequence(stuff, action, f, params, removeprogress = true) {
    return call_api_generating_what(stuff, action, f, params, "sequence", removeprogress);
}


function call_api_generating_what(stuff, action, f, params, what, removeprogress = true) {
    let progress = { type : "computing", data: {type : "empty", cur : 1, max : 1, onstop : function(){}} };
    stuff.push(progress);
    let remove_progress_bar = function() {
        console.log("removing progress bar");
        let idx = stuff.indexOf(progress);
        if(idx != -1)stuff.splice(idx,1);
    }
    let termination_handle = removeprogress?
        f(...params, p => on_new_what(stuff, action, progress, p, what, removeprogress),e =>  { remove_progress_bar() ; stuff.push({ type : "error", data : e });} ,progress.data) :
        f(...params, p => on_new_what(stuff, action, progress, p, what, removeprogress),e =>  { remove_progress_bar() ; stuff.push({ type : "error", data : e });} ,progress.data, function(){
            remove_progress_bar();
        });

    progress.data.onstop = function() {
        remove_progress_bar();
        console.log("killing worker");
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
                case "hardenremove":
                    return "Performed Hardening: Removed Label " + this.action.label;
                case "orientation":
                    return "Gave input orientation. Outdegree = " + this.action.outdegree;
                case "speedup":
                    return "Performed speedup";
                case "fixpoint-basic":
                    return "Generated Fixed Point with Default Diagram.";
                case "fixpoint-gendefault":
                    return "Generated Default Diagram";
                case "fixpoint-loop":
                    return "Generated Fixed Point with Automatic Diagram Fixing.";
                case "fixpoint-custom":
                    return "Generated Fixed Point with Custom Diagram:\n" + this.action.diagram;
                case "fixpoint-dup":
                    return "Generated Fixed Point With Label Duplication: "+ this.action.dups;
                case "inversespeedup":
                    return "Performed inverse speedup";
                case "speedupmaximize":
                    return "Performed speedup and maximized";
                case "speedupmaximizerenamegen":
                    return "Performed speedup, maximized, and renamed by generators";
                case "maximize":
                    return "Maximized passive side";
                case "renamegenerators":
                    return "Renamed by generators";
                case "rename":
                    return "Renamed";
                case "autoub":
                    return "Automatic Upper Bound (max labels: "+ this.action.max_labels + ", branching: "+ this.action.branching + ", max steps: " + this.action.max_steps + "). Obtained Upper Bound of " + this.action.len + " Rounds.";
                case "autoautoub":
                    return "Automatic Upper Bound with Automatic Parameters. Obtained Upper Bound of " + this.action.len + " Rounds.";
                default:
                    return "Unknown " + this.action.type
            }
        }
    },
    template: `
        <div class="card bg-primary text-white m-2 p-2" :id="'current'+this._uid">
            <span style="white-space: break-spaces;">{{ actionview }}<button type="button" class="close" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
            </span>
        </div>
    `
})

Vue.component('re-error', {
    props: ['error','handle','stuff'],
    methods: {
        on_close() {
            let idx = this.stuff.indexOf(this.handle);
            this.stuff.splice(idx,1);
        }
    },
    template: `
        <div class="card bg-danger text-white m-2 p-2" :id="'current'+this._uid">
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
                case "autoub":
                    return {bar : false, msg: "Computing an Upper Bound Automatically"}; 
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
                case "combining line pairs":
                    return {bar : true, msg: "Maximizing, combining lines ("+this.action.max+")", max : this.action.max, cur : this.action.cur };
                case "triviality":
                    return {bar : true, msg: "Computing triviality", max : this.action.max, cur : this.action.cur };
                case "orientationtriviality":
                    return {bar : true, msg: "Computing orientation triviality", max : this.action.max, cur : this.action.cur };
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
        <div class="card card-body m-2 bg-light" :id="'current'+this._uid">
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
        </div>
    `
})


Vue.component('re-constraint', {
    props: ['problem','side','mode'],
    computed: {
        table : function() {
            let problem = this.problem;
            let constraint = this.side == "active" ? problem.active : problem.passive;
            return constraint.lines.map(row => row.parts.map(elem => {
                let renamed = labelset_to_string(elem.group,this.problem.map_label_text);
                let original = problem.mapping_label_oldlabels == null ? null : elem.group.map(x => labelset_to_string(this.problem.map_label_oldlabels[x],this.problem.map_oldlabel_text));

                let r = {  renamed : renamed, original : original};
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
    props: ["problem","id"],
    data : function() {
        return {
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
                    enabled: this.physics
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
            };
        }
    },
    watch : {
        'physics' : function() {
            if(this.network[0] != null){
                this.network[0].setOptions(this.options);
            }
        }
    },
    mounted: function() {
        let id = "diagram" + this.id;
        let network = new vis.Network(document.getElementById(id), this.visdata, {});
        network.setOptions(this.options);
        let p = this.problem;
        network.on("select", function() {
            p.selected = network.getSelectedNodes();
        });
        network.on("selectEdge", function() {
            p.selectedEdges = network.getSelectedEdges().map(x => network.getConnectedNodes(x));
        });
        //prevent vue from adding getters and setters, otherwise some things of vis break
        this.network[0] = network;
    },
    template: `
        <div>
            <div class="panel-resizable" style="width: 300px; height: 300px;" :id="'diagram'+this.id">
            </div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="physics"><p class="form-control-static custom-control-label">Physics</p></label>
            </div>
        </div>
    `
})


Vue.component('re-orientation-give',{
    props: ['problem','stuff'],
    data: function() {
        return {
            outdegree : 1
        }
    },
    methods: {
        on_orientation() {
            console.log(this.outdegree)
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
    methods: {
        on_rename() {
            call_api_generating_problem(this.stuff,{type:"rename"},rename,[this.problem,this.table.map(x => [x[0],x[3]])]);
        }
    },
    template: `
    <re-card title="New renaming" subtitle="(manually rename labels)" :id="'group'+this._uid">
        <table class="table">
            <tr v-for="(row,index) in this.table">
                <td class="align-middle" v-if="row[2]!=''"><span class="rounded m-1 labelborder">{{ row[2] }}</span></td>
                <td class="align-middle">{{ row[1] }}</td>
                <td class="align-middle"><input class="form-control" v-model="table[index][3]"></input></td>
            </tr>
        </table>
        <button type="button" class="btn btn-primary m-1" v-on:click="on_rename">Rename</button>
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
        <re-card title="Simplify" subtitle="(by merging or adding arrows)" :id="'group'+this._uid">
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
    data: function() {return {
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
        <re-card title="Harden" subtitle="(by removing labels)" :id="'group'+this._uid">
            <re-label-picker :problem="this.problem" v-model="label"></re-label-picker>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="keep_predecessors"><p class="form-control-static custom-control-label">Replace With Predecessors</p></label>
            </div>
            <button type="button" class="btn btn-primary ml-2" v-on:click="on_remove">Remove</button>
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
        <re-card title="Group Simplify" subtitle="(choose a group of labels)" :id="'group'+this._uid">
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
        <re-card title="Group Harden" subtitle="(choose a group of labels)" :id="'group'+this._uid">
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
    template: `
        <re-card title="Automatic Lower Bound" subtitle="" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-auto-ub',{
    props: ['problem','stuff'],
    data: function() {
        return {
            max_labels : this.problem.labels.length + 4,
            branching : 4,
            max_steps : 8,
            allow_discard_old : false
        }
    },
    methods: {
        on_autoub() {
            call_api_generating_sequence(this.stuff,{type:"autoub", max_labels : this.max_labels, branching : this.branching, max_steps : this.max_steps},autoub,[this.problem, this.max_labels, this.branching, this.max_steps, this.allow_discard_old], false);
        },
        on_autoautoub() {
            call_api_generating_sequence(this.stuff,{type:"autoautoub"},autoautoub,[this.problem, this.allow_discard_old], false);
        }
    },
    template: `
        <re-card title="Automatic Upper Bound" subtitle="" :id="'group'+this._uid">
            <div>Max Labels: <input class="form-control m-2" type="number" v-model="max_labels"></div>
            <div>Branching: <input class="form-control m-2" type="number" v-model="branching"></div>
            <div>Max Steps: <input class="form-control m-2" type="number" v-model="max_steps"></div>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="allow_discard_old"><p class="form-control-static custom-control-label">Allow Discarding Old Labels</p></label>
            </div>
            <button type="button" class="btn btn-primary m-2" v-on:click="on_autoub">Automatic Upper Bound</button>
            <hr/>
            <div class="custom-control custom-switch m-2">
                <label><input type="checkbox" class="custom-control-input" v-model="allow_discard_old"><p class="form-control-static custom-control-label">Allow Discarding Old Labels</p></label>
            </div>
            <button type="button" class="btn btn-primary m-2" v-on:click="on_autoautoub">Automatic Upper Bound with Automatic Parameters</button>
        </re-card>
    `
})

Vue.component('re-operations',{
    props: ['problem','stuff'],
    template: `
        <re-card title="Operations" subtitle="(speedup, maximize, edit, gen renaming, merge)" :id="'group'+this._uid">
            <div class="m-2"><re-speedup :problem="problem" :stuff="stuff"></re-speedup> apply round elimination</div>
            <div class="m-2"><re-maximize :problem="problem" :stuff="stuff"></re-maximize> maximize passive side (and compute full diagram, triviality, ...)</div>
            <div class="m-2" v-if="this.problem.info.is_mergeable"><re-merge :problem="problem" :stuff="stuff"></re-merge>merge equivalent labels</div>
            <div class="m-2"><re-edit :problem="problem" :stuff="stuff"></re-edit>copy problem up</div>
            <div class="m-2"><re-inverse-speedup :problem="problem" :stuff="stuff"></re-inverse-speedup> apply inverse round elimination</div>
            <div class="m-2" v-if="this.problem.mapping_label_oldlabels != null"><re-rename-generators :problem="problem" :stuff="stuff"></re-rename-generators>rename by using diagram generators</div>
            <div class="m-2"><re-speedup-maximize :problem="problem" :stuff="stuff"></re-speedup-maximize><re-speedup-maximize-rename :problem="problem" :stuff="stuff"></re-speedup-maximize-rename></div>
            <re-orientation-give :problem="problem" :stuff="stuff"></re-orientation-give>
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
            <re-harden-remove :problem="problem" :stuff="stuff"></re-harden-remove>
            <re-group-harden :problem="problem" :stuff="stuff"></re-group-harden>
            <re-rename :problem="problem" :stuff="stuff"></re-rename>
            <re-fixpoint :problem="problem" :stuff="stuff"></re-fixpoint>
            <re-auto-ub :problem="problem" :stuff="stuff"></re-auto-ub>
        </div>
    `
})



Vue.component('re-problem', {
    props: ["problem","stuff","handle"],
    data: function() {
        return {
            mode : "renamed"
        }
    },
    methods: {
        on_close() {
            let idx = this.stuff.indexOf(this.handle);
            this.stuff.splice(idx,1);
        }
    },
    template: `
        <div class="card card-body m-2 p-2 bg-light position-relative" :id="'problem'+this._uid">
            <div class="row p-0 m-0 justify-content-between">
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
                    <re-constraint side="passive" :mode="this.mode" :problem="this.problem"></re-constraint>
                </re-card>
                <re-card title="Renaming" subtitle="Old and new labels" show="true" v-if="this.problem.mapping_label_oldlabels != null">
                    <re-renaming :problem="problem"></re-renaming>
                </re-card>
                <re-card title="Renaming" subtitle="Old and new labels" show="true" v-if="this.problem.mapping_oldlabel_labels != null">
                    <re-inverse-renaming :problem="problem"></re-inverse-renaming>
                </re-card>
                <re-card :title="this.problem.passive.is_maximized ? 'Diagram' : 'Partial Diagram'" subtitle="Strength of passive labels" show="true" v-if="this.problem.diagram_direct != null">
                    <re-diagram :problem="problem" :id="'diag'+this._uid" ></re-diagram>
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
        data : function() {
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
    data: function(){ return {
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
                <textarea rows="4" cols="30" class="form-control" v-model="active"></textarea>
            </div>
            <div class="col-md">
                <h4>Passive</h4>
                <textarea rows="4" cols="30" class="form-control" v-model="passive"></textarea>
            </div>
            <div class="col-sm mt-auto text-right">
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
    template: `
        <re-card :show="this.problem.fixpoint_diagram !== null" title="Generate Fixed Point" subtitle="(automatic procedure for fixed point generation)" :id="'group'+this._uid">
            <div class="m-2"><re-fixpoint-basic :problem="problem" :stuff="stuff"></re-fixpoint-basic> (with default diagram)</div>
            <div class="m-2"><re-fixpoint-loop :problem="problem" :stuff="stuff"></re-fixpoint-loop> (with default diagram, automatic fixing)</div>
            <div v-if="this.problem.fixpoint_diagram === null" class="m-2">
                <re-fixpoint-gendefault :problem="problem" :stuff="stuff"></re-fixpoint-gendefault> for additional options, click here
            </div>
            <div v-else class="m-2">
                <re-fixpoint-dup :problem="problem" :stuff="stuff"></re-fixpoint-dup>
                <re-fixpoint-custom :problem="problem" :stuff="stuff"></re-fixpoint-custom>
            </div>
        </re-card>
    `
})

Vue.component('re-fixpoint-gendefault',{
    props: ['problem','stuff'],
    methods: {
        on_fixpoint() {
            call_api_generating_problem(this.stuff,{type:"fixpoint-gendefault"},fixpoint_gendefault,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Generate Default Diagram</button>
    `
})

Vue.component('re-fixpoint-basic',{
    props: ['problem','stuff'],
    methods: {
        on_fixpoint() {
            call_api_generating_problem(this.stuff,{type:"fixpoint-basic"},fixpoint_basic,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Basic</button>
    `
})

Vue.component('re-fixpoint-loop',{
    props: ['problem','stuff'],
    methods: {
        on_fixpoint() {
            call_api_generating_problem(this.stuff,{type:"fixpoint-loop"},fixpoint_loop,[this.problem]);
        }
    },
    template: `
        <button type="button" class="btn btn-primary m-2" v-on:click="on_fixpoint">Loop</button>
    `
})

Vue.component('re-fixpoint-custom',{
    props: ['problem','stuff'],
    data: function(){ return {
            text : this.problem.fixpoint_diagram.text,
        }    
    },
    methods: {
        on_fixpoint() {
            call_api_generating_problem(this.stuff,{type:"fixpoint-custom", diagram: this.text},fixpoint_custom,[this.problem, this.text]);
        }
    },
    template: `
        <re-card show="true" title="With Custom Diagram" subtitle="(you can edit the diagram)" :id="'group'+this._uid">
            <textarea rows="4" cols="30" class="form-control m-1" v-model="text"></textarea>
            <button type="button" class="btn btn-primary m-1" v-on:click="on_fixpoint">Generate Fixed Point</button>
        </re-card>
    `
})


Vue.component('re-fixpoint-dup',{
    props: ['problem','stuff'],
    data: function(){ return {
        dups : [],  
    }},
    methods: {
        on_delete(index) {
            this.dups.splice(index, 1);
        },
        on_from_diagram(){
            let selected = this.problem.fixpoint_diagram.selected;
            if( selected != null && selected.length > 0 ){
                this.dups.push(selected)
            }
        },
        on_fixpoint() {
            call_api_generating_problem(this.stuff,{type:"fixpoint-dup", dups: "["+this.dups.map(x => "["+this.convert(x)+"]").join(",")+"]"},fixpoint_dup,[this.problem, this.dups]);
        },
        convert(x){
            return labelset_to_string(x,this.problem.fixpoint_diagram.map_label_text,", ")
        }
    },
    template: `
        <re-card show="true" title="With Label Duplication" subtitle="(choose groups of labels to duplicate)" :id="'group'+this._uid">
            <re-diagram :problem="problem.fixpoint_diagram" :id="'diag'+this._uid" ></re-diagram>
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
        on_share : function() {
            let redata = {active:this.active,passive:this.passive,stuff:this.stuff};
            let data = {v:version, d : redata};
            let json = JSON.stringify(data);
            let compressed = pako.deflateRaw(json,{level:9});
            console.log("original length: " +json.length);
            console.log("compressed length: " +compressed.length);
            //console.log(compressed);
            // https://github.com/WebReflection/uint8-to-base64/blob/master/index.js
            var v = [];
            for (var i = 0, length = compressed.length; i < length; i++) {
                v.push(String.fromCharCode(compressed[i]));
            }
            let s = v.join("");
            //console.log(s);
            let b64 =  btoa(s);
            let uri = window.location.href.split("#")[0];
            let link = uri + '#' + b64;
            //navigator.clipboard.writeText(link);
            copyToClipboard(link);
        }
    },
    template: `
        <button type="button" class="btn btn-primary" v-on:click="on_share">Export Link To Clipboard</button>
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
            <div class="card bg-light pr-4 m-2" :id="'current'+this._uid" v-if="this.supstuff != null">
                <button type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close" v-on:click="on_close">
                    <span aria-hidden="true">&times;</span>
                </button>
                <div v-for="elem in this.stuff">
                    <re-performed-action :stuff="stuff" :action='elem.data' v-if='elem.type == "performed"'  :handle="elem"/></re-performed-action>
                    <re-computing :action='elem.data' v-if='elem.type == "computing"'  :handle="elem"/></re-computing>
                    <re-error :stuff="stuff" :error='elem.data' v-if='elem.type == "error"'  :handle="elem"/></re-computing>
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
                    <re-error :stuff="stuff" :error='elem.data' v-if='elem.type == "error"'  :handle="elem"/></re-computing>
                    <re-problem :problem='elem.data' :stuff='stuff' v-if='elem.type == "problem"' :handle="elem"></re-problem>
                    <re-stuff :supstuff='stuff' :stuff='elem.data' v-if='elem.type == "sub"' :handle="elem"></re-stuff>
                </div>
            </div>
        </div>
    `
})


function init_data() {
    let def = {active:"M U^9\nP^10",passive:"M UP^9\nU^10",stuff:[]};
    if( window.location.hash ) {
        let b64 = window.location.hash.substring(1);
        let s = atob(b64);
        let compressed = Uint8Array.from(s, c => c.charCodeAt(0));
        let json = pako.inflateRaw(compressed, { to: 'string' });
        let data = JSON.parse(json);
        if( data.v != version ){
            alert("Sorry, the link comes from a different version of REtor");
            return def;
        }
        return data.d;
    } else return def;
}

var app = new Vue({
    el: '#vueapp',
    data: {
        all : init_data(),
    },
    template: `
        <div>
            <re-begin :all="all"></re-begin>
            <re-stuff :stuff="all.stuff"></re-stuff>
        </div>
    `
})


