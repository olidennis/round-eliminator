

let serialized1 = '{"active":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":"Star","group":[1]}]},{"parts":[{"gtype":"Star","group":[2]}]}],"is_maximized":false,"degree":"Star"},"passive":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":"Star","group":[1,2]}]},{"parts":[{"gtype":"One","group":[0,1]},{"gtype":"Star","group":[1]}]}],"is_maximized":true,"degree":"Star"},"mapping_label_text":[[1,"U"],[2,"P"],[0,"M"]],"mapping_label_oldlabels":null,"mapping_oldlabel_text":null,"trivial_sets":[],"coloring_sets":null,"diagram_indirect":[[0,0],[1,1],[2,1],[2,2]],"diagram_indirect_old":null,"diagram_direct":[[[0,[0]],[1,[1]],[2,[2]]],[[2,1]]]}';
let problem1 = JSON.parse(serialized1);
console.log(problem1);

let serialized2 = '{"active":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":{"Many":2},"group":[1]}]},{"parts":[{"gtype":"One","group":[2]},{"gtype":{"Many":2},"group":[3]}]}],"is_maximized":false,"degree":{"Finite":3}},"passive":{"lines":[{"parts":[{"gtype":{"Many":2},"group":[0,1]}]},{"parts":[{"gtype":{"Many":2},"group":[2,3]}]}],"is_maximized":true,"degree":{"Finite":2}},"mapping_label_text":[[3,"D"],[0,"A"],[2,"C"],[1,"B"]],"mapping_label_oldlabels":null,"mapping_oldlabel_text":null,"trivial_sets":[[0,1],[2,3]],"coloring_sets":null,"diagram_indirect":[[0,0],[0,1],[1,0],[1,1],[2,2],[2,3],[3,2],[3,3]],"diagram_indirect_old":null,"diagram_direct":[[[0,[0,1]],[2,[2,3]]],[]]}';
let problem2 = JSON.parse(serialized2);
console.log(problem2);

let serialized3 = '{"active":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":{"Many":2},"group":[1]}]},{"parts":[{"gtype":"One","group":[2]},{"gtype":{"Many":2},"group":[3]}]}],"is_maximized":false,"degree":{"Finite":3}},"passive":{"lines":[{"parts":[{"gtype":"One","group":[0,1]},{"gtype":"One","group":[2,3]}]}],"is_maximized":true,"degree":{"Finite":2}},"mapping_label_text":[[1,"B"],[3,"D"],[2,"C"],[0,"A"]],"mapping_label_oldlabels":null,"mapping_oldlabel_text":null,"trivial_sets":[],"coloring_sets":[[0,1],[2,3]],"diagram_indirect":[[0,0],[0,1],[1,0],[1,1],[2,2],[2,3],[3,2],[3,3]],"diagram_indirect_old":null,"diagram_direct":[[[0,[0,1]],[2,[2,3]]],[]]}';
let problem3 = JSON.parse(serialized3);
console.log(problem3);

let serialized4 = '{"active":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":"One","group":[0,1]},{"gtype":"One","group":[1]},{"gtype":"One","group":[2]}]}],"is_maximized":false,"degree":{"Finite":4}},"passive":{"lines":[{"parts":[{"gtype":{"Many":2},"group":[0,1]}]},{"parts":[{"gtype":{"Many":2},"group":[2]}]}],"is_maximized":true,"degree":{"Finite":2}},"mapping_label_text":[[1,"B"],[0,"A"],[2,"C"]],"mapping_label_oldlabels":null,"mapping_oldlabel_text":null,"trivial_sets":[],"coloring_sets":[],"diagram_indirect":[[0,0],[0,1],[1,0],[1,1],[2,2]],"diagram_indirect_old":null,"diagram_direct":[[[0,[0,1]],[2,[2]]],[]]}';
let problem4 = JSON.parse(serialized4);
console.log(problem4);

let serialized5 = '{"active":{"lines":[{"parts":[{"gtype":"One","group":[0]},{"gtype":"Star","group":[1]}]},{"parts":[{"gtype":"One","group":[2]},{"gtype":"Star","group":[3]}]}],"is_maximized":false,"degree":"Star"},"passive":{"lines":[{"parts":[{"gtype":"One","group":[0,2]},{"gtype":"Star","group":[1,2,3]}]},{"parts":[{"gtype":"One","group":[0,1,2,3]},{"gtype":"One","group":[2]},{"gtype":"Star","group":[1,2,3]}]},{"parts":[{"gtype":"One","group":[0,1,2]},{"gtype":"Star","group":[1,2]}]}],"is_maximized":true,"degree":"Star"},"mapping_label_text":[[0,"A"],[1,"B"],[2,"C"],[3,"D"]],"mapping_label_oldlabels":[[0,[0]],[1,[1,2]],[2,[0,1]],[3,[1]]],"mapping_oldlabel_text":[[0,"M"],[2,"P"],[1,"U"]],"trivial_sets":[],"coloring_sets":null,"diagram_indirect":[[0,0],[0,2],[1,1],[1,2],[2,2],[3,1],[3,2],[3,3]],"diagram_indirect_old":null,"diagram_direct":[[[0,[0]],[1,[1]],[2,[2]],[3,[3]]],[[0,2],[1,2],[3,1]]]}';
let problem5 = JSON.parse(serialized5);
console.log(problem5);

function fix_problem(p) {
    p.map_label_text = vec_to_map(p.mapping_label_text);
    p.map_label_oldlabels = vec_to_map(p.mapping_label_oldlabels) ?? null;
    p.map_oldlabel_text = vec_to_map(p.mapping_oldlabel_text) ?? null;
    p.labels = p.mapping_label_text.map(x => x[0]);
    let problem = p;
    let numlabels = problem.mapping_label_text.length;
    let is_zero = problem.trivial_sets != null && problem.trivial_sets.length > 0;
    let is_nonzero = problem.trivial_sets != null && problem.trivial_sets.length == 0;
    let numcolors = problem.coloring_sets != null ? problem.coloring_sets.length : 0;
    let zerosets = !is_zero ? [] : problem.trivial_sets.map(x => labelset_to_string(x,problem.map_label_text));
    let coloringsets = numcolors < 2 ? [] : problem.coloring_sets.map(x => labelset_to_string(x,problem.map_label_text));
    let mergeable = (problem.diagram_direct ?? [[]])[0].filter(x => x[1].length > 1); 
    let is_mergeable = mergeable.length > 0;
    let mergesets = !is_mergeable ? [] : mergeable.map(x => labelset_to_string(x[1],problem.map_label_text));
    p.info = { numlabels : numlabels, is_zero : is_zero, is_nonzero : is_nonzero, numcolors : numcolors, zerosets : zerosets, coloringsets : coloringsets, is_mergeable : is_mergeable, mergesets : mergesets };
}

fix_problem(problem1);
fix_problem(problem2);
fix_problem(problem3);
fix_problem(problem4);
fix_problem(problem5);


function vec_to_map(v){
    if( v == null ){
        return null;
    }
    return Object.assign({}, ...v.map((x) => ({[x[0]]: x[1]})));
}

function labelset_to_string(v, mapping) {
    return v.map(x => mapping[x]).join("");
}


Vue.component('re-container', {
    template: `
        <span>test</span>
    `
})


Vue.component('re-performed-action', {
    props: ['action'],
    computed: {
        actionview: function() {
            switch( this.action.type ) {
                case "initial":
                    return "Initial problem";
                case "mergeequal":
                    return "Merged equivalent labels.";
                case "simplificationmerge":
                    return "Performed Simplification: Merge " + this.action.from + "→" + this.action.to;
                case "simplificationaddarrow":
                    return "Performed Simplification: Add Arrow " + this.action.from + "→" + this.action.to;
                case "hardenkeep":
                    return "Performed Hardening: Keep Label Set " + this.action.labels.join("");
                case "hardenremove":
                    return "Performed Hardening: Remove Label " + this.action.label;
                case "speedup":
                    return "Performed speedup";
                default:
                    return "Unknown " + this.action.type
            }
        }
    },
    template: `
        <div class="card bg-primary text-white m-2 p-2" :id="'current'+this._uid">
            <span>
                {{ actionview }}
                <button data-dismiss="alert" :data-target="'#current'+this._uid" type="button" class="close" aria-label="Close">
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
                case "combining line pairs":
                    return {bar : true, msg: "Maximizing, combining line pairs", max : this.action.max, cur : this.action.cur };
                case "triviality":
                    return {bar : true, msg: "Computing triviality", max : this.action.max, cur : this.action.cur };
                default:
                    return "Unknown " + this.action.type
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
            <button data-dismiss="alert" :data-target="'#current'+this._uid" type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close">
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
                    <div>The problem is solvable in zero round given a {{ this.problem.info.numcolors }} coloring.</div>
                    <div>The following sets are colors:
                        <span v-for="set in this.problem.info.coloringsets">{{ set }} </span>
                    </div>
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
                if( elem.gtype == "One" ){
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
    props: ['title','subtitle','show','id'],
    template : `
        <div class="card m-2">
            <div class="card-header p-0">
                <button class="btn btn-link text-left" data-toggle="collapse" :data-target="'.collapse'+this.id">
                    {{ this.title }}<br/>
                    <small v-if="this.subtitle!=''">{{ this.subtitle }}</small>
                </button>
            </div>
            <div :class="'collapse'+this.id + ' collapse ' + (this.show?'show':'')">
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


Vue.component('re-diagram', {
    props: ["problem","id"],
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
        }
    },
    mounted: function() {
        let id = "diagram" + this.id;
        let network = new vis.Network(document.getElementById(id), this.visdata, {});
    },
    template: `
        <div class="panel-resizable" style="width: 300px; height: 300px;" :id="'diagram'+this.id" onmouseover="document.body.style.overflow='hidden';"  onmouseout="document.body.style.overflow='auto';">
        </div>
    `
})


Vue.component('re-speedup',{
    props: ['problem'],
    template: `
        <button type="button" class="btn btn-primary ml-2">Speedup</button>
    `
})


Vue.component('re-merge',{
    props: ['problem'],
    template: `
        <button type="button" class="btn btn-primary ml-2" v-if="this.problem.info.is_mergeable">Merge</button>
    `
})

Vue.component('re-edit',{
    props: ['problem'],
    template: `
        <button type="button" class="btn btn-primary m-2">Edit</button>
    `
})

Vue.component('re-simplify-merge',{
    props: ['problem'],
    template: `
        <re-card title="Simplify" subtitle="(by merging)" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-simplify-addarrow',{
    props: ['problem'],
    template: `
        <re-card title="Simplify" subtitle="(by adding arrows)" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-harden-keep',{
    props: ['problem'],
    template: `
        <re-card title="Harden" subtitle="(by keeping labels)" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-harden-remove',{
    props: ['problem'],
    template: `
        <re-card title="Harden" subtitle="(by removing labels)" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-auto-lb',{
    props: ['problem'],
    template: `
        <re-card title="Automatic Lower Bound" subtitle="" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-auto-ub',{
    props: ['problem'],
    template: `
        <re-card title="Automatic Upper Bound" subtitle="" :id="'group'+this._uid">
        </re-card>
    `
})

Vue.component('re-tools', {
    props: ["problem"],
    computed: {
        
    },
    template: `
        <div>
            <re-speedup :problem="problem"></re-speedup>
            <re-merge :problem="problem"></re-merge>
            <re-edit :problem="problem"></re-edit>
            <re-simplify-merge :problem="problem"></re-simplify-merge>
            <re-simplify-addarrow :problem="problem"></re-simplify-addarrow>
            <re-harden-keep :problem="problem"></re-harden-keep>
            <re-harden-remove :problem="problem"></re-harden-remove>
            <re-auto-lb :problem="problem"></re-auto-lb>
            <re-auto-ub :problem="problem"></re-auto-ub>
        </div>
    `
})


Vue.component('re-problem', {
    props: ["problem"],
    data: function() {
        return {
            mode : "renamed"
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
                <button data-dismiss="alert" :data-target="'#problem'+this._uid" type="button" class="close position-absolute top-0 end-0 p-2" aria-label="Close">
                    <span aria-hidden="true">&times;</span>
                </button>
            </div>
            <re-problem-info :problem="this.problem"></re-problem-info>
            <div class="row p-0 m-2">
                <re-card title="Active" subtitle="Any choice satisfies previous Passive" :id="'group1'+this._uid" show="true">
                    <re-constraint side="active" :mode="this.mode" :problem="this.problem"></re-constraint>
                </re-card>
                <re-card title="Passive" subtitle="Exists choice satisfying previous Active" :id="'group1'+this._uid" show="true">
                    <re-constraint side="passive" :mode="this.mode" :problem="this.problem"></re-constraint>
                </re-card>
                <re-card title="Renaming" subtitle="Old and new labels" :id="'group1'+this._uid" show="true" v-if="this.problem.mapping_label_oldlabels != null">
                    <re-renaming :problem="problem"></re-renaming>
                </re-card>
                <re-card title="Diagram" subtitle="Strength of passive labels" :id="'group1'+this._uid" show="true" v-if="this.problem.diagram_direct != null">
                    <re-diagram :problem="problem" :id="'diag'+this._uid" ></re-diagram>
                </re-card>
                <re-card title="Tools" subtitle="Speedup, edit, simplifications, ..." :id="'group1'+this._uid" show="true">
                    <re-tools :problem="problem"></re-tools>
                </re-card>
            </div>
        </div>
    `
})

var app = new Vue({
    el: '#vueapp',
    data: {
        problem1 : problem1,
        problem2 : problem2,
        problem3 : problem3,
        problem4 : problem4,
        problem5 : problem5
    },
    methods: {}
})


