import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'
import { problemPresets } from '../src/presets.ts'

await init({ module_or_path: await readFile(new URL(
  '../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const server = process.argv[2]
let fixtures = 0

async function execute(problem, maximum) {
  const body = JSON.stringify({ type: 'apply_re', data: { problem, maximum } })
  const events = []
  const response = JSON.parse(execute_with_events(body, raw => events.push(JSON.parse(raw))))
  assert.equal(response.type, 'problem', JSON.stringify(response))
  assert.ok(events.every(event => event.type !== 'progress' || event.data.phase !== 'parsing'))
  const snapshots = events.filter(event => event.type === 'partial_result').map(event => event.data)
  assert.equal(snapshots.length, maximum ? 0 : 1)
  if (!maximum) assert.equal(snapshots[0].type, 'maximization')
  else assert.ok(events.every(event => event.type !== 'progress' || event.data.phase !== 'maximizing'))
  if (server) {
    const http = await fetch(`${server}/stream`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    })
    assert.equal(http.status, 200)
    const updates = (await http.text()).split('\n\n').filter(frame => frame.includes('data:'))
      .map(frame => JSON.parse(frame.split('\n').find(line => line.startsWith('data:')).slice(5)))
    assert.deepEqual(updates.at(-1).data, response)
    assert.deepEqual(updates.filter(event => event.type === 'partial_result').map(event => event.data), snapshots)
    assert.ok(updates.every(event => event.type !== 'progress' || event.data.phase !== 'parsing'))
    if (maximum) assert.ok(updates.every(event => event.type !== 'progress' || event.data.phase !== 'maximizing'))
  }
  return { problem: response.data, maximum: maximum ?? snapshots[0].data }
}

async function check(definition, verify = () => {}) {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  assert.equal(parsed.type, 'problem')
  const source = parsed.data
  const before = structuredClone(source)
  const result = await execute(source, null)
  assert.deepEqual((await execute(source, result.maximum)).problem, result.problem)
  assert.deepEqual(source, before)
  assert.deepEqual(result.problem.graph_class, {
    active: source.graph_class.passive, passive: source.graph_class.active,
  })
  assert.deepEqual(result.problem.input, source.input.kind === 'trivial' ? source.input : {
    kind: 'explicit', data: {
      labels: source.input.data.labels,
      active: source.input.data.passive,
      passive: source.input.data.active,
    },
  })
  assert.equal(result.problem.mapping.kind, source.mapping.kind)
  for (const side of ['active', 'passive']) {
    for (const group of result.problem.output[side].degrees) {
      assert.ok(result.problem.graph_class[side].includes(group.degree))
      for (const configuration of group.configurations) {
        assert.equal(configuration.parts.reduce((sum, part) => sum + part.multiplicity, 0), group.degree)
        assert.ok(configuration.parts.every(part => part.labels.length > 0 && part.multiplicity > 0))
      }
    }
  }
  verify(result.problem)
  fixtures++
  return result.problem
}

for (const preset of problemPresets) await check(preset.definition)
const matching = await check({ kind: 'plain', constraints: {
  active: 'M U^3\nP^4', passive: 'M^2\nU UP',
} }, result => {
  const expected = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: {
    kind: 'plain', constraints: { active: 'A^2\nB C', passive: 'A BC^3\nC^4' },
  } })))
  assert.deepEqual(result, expected.data)
})
const repeated = (await execute(matching, null)).problem
assert.deepEqual(repeated.graph_class, { active: [4], passive: [2] })
await check({ kind: 'plain', constraints: { active: '()\nA', passive: '()\nA^2' },
  degrees: { active: '0 1 3', passive: '0 2 4' },
})
await check({ kind: 'mapped', input: { active: 'i', passive: 'i^2' },
  output: { active: 'A', passive: 'B^2' }, mapping: 'i -> A',
}, result => assert.equal(result.output.labels.length, 0))
await check({ kind: 'mapped', input: { active: 'ij^100000', passive: 'ij^100000' },
  output: { active: 'A^100000', passive: 'A^100000' }, mapping: 'i -> A\nj -> A',
}, result => {
  assert.deepEqual(result.output.labels, ['A'])
  for (const side of ['active', 'passive']) {
    assert.deepEqual(result.output[side].degrees, [{ degree: 100000,
      configurations: [{ parts: [{ labels: [0], multiplicity: 100000 }] }],
    }])
  }
})
console.log(`${fixtures} RE fixtures passed through wasm${server ? ' and server' : ''}, with cache reuse, source-promise preservation, nonregular/zero degrees, high counts, and repeated RE.`)
