const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

function Vue() {}
Vue.component = () => {};
const context = { Vue, window: { location: { hash: '' } } };
vm.createContext(context);
vm.runInContext(readFileSync(path.join(__dirname, 'gui.js'), 'utf8')
    .replace(/^import \* as api from "\.\/api\.js"\s*$/m, ''), context);

test('basic works is advisory: no result and subsequent progress still updates', () => {
    const warnings = [];
    const results = [];
    const progress = {};
    const handle = x => context.handle_result(x, p => results.push(p),
        (message, warning) => warnings.push({ message, warning }), progress);
    handle({ W: 'The basic fixed-point procedure works (16-node diagram). Loop is still searching for a minimum-size good diagram.' });
    assert.equal(warnings.length, 1);
    assert.equal(warnings[0].warning, true);
    assert.equal(results.length, 0);
    handle({ Event: ['SAT: searching 12-node diagrams', 27, 0] });
    assert.equal(progress.type, 'SAT: searching 12-node diagrams');
    assert.equal(progress.cur, 27);
});
