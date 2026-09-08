// Run with: node --test www/gui-label-mapping.test.cjs
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const components = new Map();
function Vue() {}
Vue.component = (name, component) => components.set(name, component);
const source = readFileSync(path.join(__dirname, 'gui.js'), 'utf8')
    .replace(/^import \* as api from "\.\/api\.js"\s*$/m, '');
vm.runInNewContext(source, { Vue, window: { location: { hash: '' } } });

const component = components.get('re-inverse-renaming');
function rows(problem) {
    // Normalize the VM realm's object prototypes for strict comparison.
    return JSON.parse(JSON.stringify(component.computed.table.call({ problem })));
}

test('shows each original label mapping to an automatically named merger', () => {
    const problem = {
        mapping_oldlabel_labels: [[10, [3]], [20, [3]], [30, [7]]],
        map_oldlabel_labels: { 10: [3], 20: [3], 30: [7] },
        map_oldlabel_text: { 10: 'A', 20: 'B', 30: '(long_name)' },
        map_label_text: { 3: '(A=B)', 7: '(long_name)' },
    };
    assert.deepEqual(rows(problem), [
        { old: 'A', cur: '(A=B)', removed: false },
        { old: 'B', cur: '(A=B)', removed: false },
        { old: '(long_name)', cur: '(long_name)', removed: false },
    ]);
    problem.map_label_text[3] = 'X';
    assert.deepEqual(rows(problem).slice(0, 2), [
        { old: 'A', cur: 'X', removed: false },
        { old: 'B', cur: 'X', removed: false },
    ]);
});

test('marks removed targets and keeps other old-to-new mappings readable', () => {
    assert.deepEqual(rows({
        mapping_oldlabel_labels: [[1, []], [2, [4, 5]]],
        map_oldlabel_labels: { 1: [], 2: [4, 5] },
        map_oldlabel_text: { 1: 'A', 2: 'B' },
        map_label_text: { 4: '(B_1)', 5: '(B_2)' },
    }), [
        { old: 'A', cur: '', removed: true },
        { old: 'B', cur: '(B_1), (B_2)', removed: false },
    ]);
    assert.match(component.template, /Removed during simplification/);
});

test('the mapping table is shown automatically, with no new button', () => {
    assert.match(components.get('re-problem').template,
        /title="Label mapping"[^>]*show="true"[^>]*mapping_oldlabel_labels != null/);
    assert.match(component.template, /Original label/);
    assert.match(component.template, /Result label\(s\)/);
    assert.doesNotMatch(component.template, /<button/);
});
