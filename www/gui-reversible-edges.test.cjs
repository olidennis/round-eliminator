const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

function gui(native = true) {
    const components = new Map(), requests = [];
    let stopped = 0;
    function Vue() {}
    Vue.component = (n,c) => components.set(n,c);
    const context = { Vue, window: { location: { hash: '' } }, api: {
        supports_reversible_edges: () => native,
        request: (...args) => { requests.push(args); return () => stopped++; }
    } };
    vm.createContext(context);
    vm.runInContext(readFileSync(path.join(__dirname, 'gui.js'), 'utf8')
        .replace(/^import \* as api from "\.\/api\.js"\s*$/m, ''), context);
    return { context, components, requests, stopped: () => stopped };
}

test('new native button leaves the existing merge buttons in place', () => {
    const { components } = gui();
    const c = components.get('re-demisifiable');
    assert.equal(c.computed.native_edges(), true);
    assert.equal(gui(false).components.get('re-demisifiable').computed.native_edges(), false);
    assert.match(c.template, /Logstar Reversible Edge Additions/);
    assert.match(c.template, /v-if="native_edges"/);
    assert.match(c.template, /on_demisifiable_old/);
    assert.match(components.get('re-stuff').template, /re-edge-additions/g);
});

test('streaming reports update one result card; STOP retains verified results', () => {
    const g = gui(), stuff = [], problem = { marker: 'original' };
    g.context.start_reversible_edges(stuff, problem, '7');
    assert.equal(g.requests.length, 1);
    assert.equal(g.requests[0][0].ReversibleEdges[0], problem);
    assert.equal(g.requests[0][0].ReversibleEdges[1].seconds, 7);
    const entry = stuff.find(x => x.type === 'edgeadditions');
    const progress = stuff.find(x => x.type === 'computing');
    const report = { certificates: [{ added: [[0,1]] }], complete: false, message: 'Verified' };
    g.requests[0][1]({ ReversibleEdges: report });
    g.requests[0][1]({ Event: ['Reversible edges: testing A B', 1, 3] });
    assert.equal(entry.data, report);
    assert.equal(progress.data.cur, 1);
    progress.data.onstop();
    assert.equal(g.stopped(), 1);
    assert.equal(stuff.includes(progress), false);
    assert.equal(entry.data.certificates.length, 1);
    assert.match(entry.data.message, /Stopped/);
});

test('completion and errors remove progress without discarding earlier certificates', () => {
    for (const fail of [false, true]) {
        const g = gui(), stuff = [];
        g.context.start_reversible_edges(stuff, {}, 10);
        if (fail) g.requests[0][1]({ E: 'budget configuration invalid' });
        else g.requests[0][2]();
        assert.equal(stuff.some(x => x.type === 'computing'), false);
        assert.equal(stuff.filter(x => x.type === 'edgeadditions').length, 1);
    }
});

test('apply submits the specific certificate and original input for re-verification', () => {
    const g = gui(), calls = [];
    g.context.call_api_generating_problem = (...a) => calls.push(a);
    const c = g.components.get('re-edge-additions'), original = {}, certificate = { added: [[0,1]] };
    c.methods.apply.call({ stuff: [], report: { original }, edges: () => 'A B' }, certificate);
    assert.equal(calls[0][2], g.context.apply_reversible_edges);
    assert.equal(calls[0][3][0], original);
    assert.equal(calls[0][3][1], certificate);
    assert.match(c.template, /Do not combine rows/);
    assert.match(c.template, /v-if="shown === i"/);
});

test('invalid time inputs never reach the server deserializer', () => {
    for (const value of [0,-1,'',NaN,2.5,86401]) {
        const g = gui(), stuff = [];
        g.context.start_reversible_edges(stuff, {}, value);
        assert.equal(g.requests.length, 0);
        assert.equal(stuff[0].type, 'error');
    }
});

test('native availability follows the server selector', () => {
    const source = readFileSync(path.join(__dirname,'api.js'),'utf8').replace(/export function/g,'function');
    for (const [href, native] of [['http://localhost/server',true],['http://localhost/',false]]) {
        const c = { window: { location: { href } } };
        vm.createContext(c); vm.runInContext(source,c);
        assert.equal(c.supports_reversible_edges(), native);
    }
});

test('worker controls are validated and transmitted', () => {
    const g = gui();
    g.context.start_reversible_edges([], {}, 10, '3');
    assert.equal(g.requests[0][0].ReversibleEdges[1].threads, 3);
    for (const threads of [-1, 1.5, 33, NaN]) {
        const g = gui(), stuff = [];
        g.context.start_reversible_edges(stuff, {}, 10, threads);
        assert.equal(g.requests.length, 0);
        assert.equal(stuff[0].type, 'error');
    }
});

test('all stronger recipe kinds render with the original labels', () => {
    const { components } = gui();
    const component = components.get('re-edge-additions');
    const graph = { Pairs: [[0,1]] };
    const recipe = ['Prune','NodeContext', { RepairPairs: [[0,1]] }, { Matching: graph },
        { GreedyColoring: graph }, { RulingSet: graph },
        { PriorityMis: { graph, order: [[0,0], [1,1]] } }];
    const text = component.methods.recipe.call({names:{0:'A',1:'B'}}, {recipe});
    for (const expected of ['impossible','whole node','oriented maximal matching',
        'greedy coloring','distance-two ruling set','priority MIS','A B','(A A) then (B B)', 'noninterfering edge-color phases']) {
        assert(text.includes(expected), expected);
    }
});
