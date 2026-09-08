// Run with: node --test www/gui-fixpoint-normalize.test.cjs
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

function gui(native) {
    const components = new Map();
    function Vue() {}
    Vue.component = (name, value) => components.set(name, value);
    const requests = [];
    const context = {
        Vue, window: { location: { hash: '' } },
        api: {
            supports_lattice_normalization: () => native,
            request: (...args) => { requests.push(args); return 'cancel'; },
        },
    };
    vm.createContext(context);
    vm.runInContext(readFileSync(path.join(__dirname, 'gui.js'), 'utf8')
        .replace(/^import \* as api from "\.\/api\.js"\s*$/m, ''), context);
    return { context, components, requests };
}

test('normalization button is native-only and applies to the whole input', () => {
    const { context, components } = gui(true);
    const component = components.get('re-fixpoint');
    assert.equal(component.computed.native_normalization.call({}), true);
    assert.equal(gui(false).components.get('re-fixpoint').computed.native_normalization.call({}), false);
    assert.match(component.template, /v-if="native_normalization"/);
    assert.match(component.template, /v-on:click="on_normalize"/);
    const calls = [];
    context.call_api_generating_problem = (...args) => calls.push(args);
    const problem = { marker: 'original' };
    const stuff = {};
    component.methods.on_normalize.call({ problem, stuff, partial: true, triviality_only: true });
    assert.equal(calls.length, 1);
    assert.equal(calls[0][0], stuff);
    assert.equal(calls[0][1].type, 'fixpoint-normalize');
    assert.equal(calls[0][2], context.fixpoint_normalize);
    assert.deepEqual(Array.from(calls[0][3]), [problem]);
});

test('wrapper sends FixpointNormalize and forwards result and error callbacks', () => {
    const { context, requests } = gui(true);
    const problem = { marker: 'original' };
    const seen = [];
    context.handle_result = (...args) => seen.push(args);
    const callbacks = [() => {}, () => {}, () => {}];
    assert.equal(context.fixpoint_normalize(problem, ...callbacks), 'cancel');
    assert.equal(requests[0][0].FixpointNormalize, problem);
    requests[0][1]({ E: 'inconclusive' });
    assert.deepEqual(seen[0].slice(1), callbacks);
    assert.equal(seen[0][0].E, 'inconclusive');
});

test('API availability follows the same server switch as ordinary requests', () => {
    const source = readFileSync(path.join(__dirname, 'api.js'), 'utf8').replace(/export function/g, 'function');
    for (const [href, expected] of [['http://localhost/server', true], ['https://example.org/', false]]) {
        const context = { window: { location: { href } } };
        vm.createContext(context);
        vm.runInContext(source, context);
        assert.equal(context.supports_lattice_normalization(), expected);
    }
});
