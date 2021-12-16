

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

function label_mapping(problem){
    return Object.assign({}, ...problem.mapping_label_text.map((x) => ({[x[0]]: x[1]})));
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
        <div class="card bg-primary text-white m-2 p-2">{{ actionview }}</div>
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
        <div class="card card-body m-2 bg-light">
            <div class="spinner-border" role="status"></div>
            {{ state.msg }}
            <div v-if="state.bar" class="progress">
                <div class="progress-bar" role="progressbar" :style="'width : ' + Math.floor(state.cur *100 / state.max) + '%'">
            </div>
        </div>
        </div>
        
    `
})


Vue.component('re-problem-info', {
    props: ['problem'],
    computed: {
        label_mapping: function() {
            return label_mapping(this.problem);
        },
        info: function() {
            let problem = this.problem;
            let numlabels = problem.mapping_label_text.length;
            let is_zero = problem.trivial_sets != null && problem.trivial_sets.length > 0;
            let is_nonzero = problem.trivial_sets != null && problem.trivial_sets.length == 0;
            let numcolors = problem.coloring_sets != null ? problem.coloring_sets.length : 0;
            let zerosets = !is_zero ? [] : problem.trivial_sets.map(x => labelset_to_string(x,this.label_mapping));
            let coloringsets = numcolors < 2 ? [] : problem.coloring_sets.map(x => labelset_to_string(x,this.label_mapping));
            let mergeable = (this.problem.diagram_direct ?? [[]])[0].filter(x => x[1].length > 1); 
            let is_mergeable = mergeable.length > 0;
            let mergesets = !is_mergeable ? [] : mergeable.map(x => labelset_to_string(x[1],this.label_mapping));
            return { numlabels : numlabels, is_zero : is_zero, is_nonzero : is_nonzero, numcolors : numcolors, zerosets : zerosets, coloringsets : coloringsets, is_mergeable : is_mergeable, mergesets : mergesets };
        }
    },
    template: `
        <div>
            <div class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>{{ this.info.numlabels }} Labels.</div>
                </div>
            </div>
            <div v-if="this.info.is_zero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem IS zero round solvable.</div>
                    <div>The following sets allow zero round solvability:
                        <span v-for="set in this.info.zerosets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.info.is_nonzero" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is NOT zero round solvable</div>
                </div>
            </div>
            <div v-if="this.info.numcolors >= 2" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The problem is solvable in zero round given a {{ this.info.numcolors }} coloring.</div>
                    <div>The following sets are colors:
                        <span v-for="set in this.info.coloringsets">{{ set }} </span>
                    </div>
                </div>
            </div>
            <div v-if="this.info.is_mergeable" class="col-auto m-2 p-0">
                <div class="card card-body m-0 p-2">
                    <div>The following labels can be merged:
                        <span v-for="set in this.info.mergesets">{{ set }} </span>
                    </div>
                </div>
            </div>
        </div>
    `
})


Vue.component('re-constraint', {
    props: ['problem','side','oldlabels'],
    computed: {
        label_mapping: function() {
            return label_mapping(this.problem);
        },
        table : function() {
            let problem = this.problem;
            let constraint = this.side == "active" ? problem.active : problem.passive;
            return constraint.lines.map(row => row.parts.map(elem => {
                let r = {  label : labelset_to_string(elem.group,this.label_mapping) };
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
                    {{ elem.label }}<sup v-if="elem.rep">{{ elem.rep }}</sup><span v-if="elem.star">*</span>
                </td>
            </tr>
        </table>
    `   
})


var app = new Vue({
    el: '#vueapp',
    data: {
        problem1 : problem1,
        problem2 : problem2,
        problem3 : problem3,
        problem4 : problem4
    },
    methods: {}
})



