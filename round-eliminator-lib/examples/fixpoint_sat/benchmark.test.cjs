const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { checkModel } = require('./benchmark.cjs');

test('actual-DIMACS checker rejects false, incomplete, and conflicting models', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'fixpoint-cnf-check-'));
  t.after(() => fs.rmSync(directory, { recursive: true }));
  const cnf = path.join(directory, 'instance.cnf');
  const model = path.join(directory, 'assignment.model');
  fs.writeFileSync(cnf, 'c small test\np cnf 3 3\n1 0\n-1 2 0\n-2 3 0\n');
  fs.writeFileSync(model, 's SATISFIABLE\nv 1 2\nv 3 0\n');
  assert.deepEqual(await checkModel(cnf, model), { variables: 3, clauses: 3 });
  for (const text of [
    's SATISFIABLE\nv -1 2 3 0\n',
    's SATISFIABLE\nv 1 2 0\n',
    's SATISFIABLE\nv 1 -1 2 3 0\n',
    's SATISFIABLE\nv 1 2 3 4 0\n',
    's UNKNOWN\nv 1 2 3 0\n',
  ]) {
    fs.writeFileSync(model, text);
    await assert.rejects(checkModel(cnf, model));
  }
  fs.writeFileSync(model, 's SATISFIABLE\nv 1 2 3 0\n');
  fs.writeFileSync(cnf, 'p cnf 3 2\n1 0\n');
  await assert.rejects(checkModel(cnf, model), /Clause count mismatch/);
});
