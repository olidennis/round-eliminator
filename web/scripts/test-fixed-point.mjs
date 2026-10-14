import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'

await init({ module_or_path: await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const server = process.argv[2]
let checks = 0

async function check(definition, supported = true) {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  assert.equal(parsed.type, 'problem')
  const before = structuredClone(parsed.data)
  const body = JSON.stringify({ type: 'basic_fixed_point', data: { problem: before } })
  const events = []
  const response = JSON.parse(execute_with_events(body, raw => events.push(JSON.parse(raw))))
  assert.equal(response.type, supported ? 'problem' : 'error', JSON.stringify(response))
  assert.deepEqual(parsed.data, before)
  if (supported) {
    assert.deepEqual(response.data.input, before.input)
    assert.deepEqual(response.data.graph_class, before.graph_class)
    assert.equal(response.data.mapping.kind, 'unrestricted')
  }
  for (const event of events) if (event.type === 'progress') {
    assert.notEqual(event.data.phase, 'parsing')
    const work = event.data.work
    if (work) assert.ok(work.completed <= work.total)
  }
  if (server) {
    const http = await fetch(`${server}/stream`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    })
    assert.equal(http.status, 200)
    const updates = (await http.text()).split('\n\n').filter(frame => frame.includes('data:'))
      .map(frame => JSON.parse(frame.split('\n').find(line => line.startsWith('data:')).slice(5)))
    assert.deepEqual(updates.at(-1).data, response)
  }
  checks++
  return response
}

const singleton = await check({ kind: 'plain', constraints: { active: 'A^2', passive: 'A^2' } })
assert.deepEqual(singleton.data.output.labels, ['A'])
for (const side of ['active', 'passive']) assert.deepEqual(singleton.data.output[side].degrees, [
  { degree: 2, configurations: [{ parts: [{ labels: [0], multiplicity: 2 }] }] },
])
await check({ kind: 'plain', constraints: { active: 'A B', passive: 'AB^2' } })
await check({ kind: 'plain', constraints: { active: '()\nA', passive: '()\nA^2' },
  degrees: { active: '0 1 3', passive: '0 2 4' } })
await check({ kind: 'independent', input: { active: 'i', passive: 'i^2' },
  output: { active: 'A', passive: 'A^2' } })
await check({ kind: 'mapped', input: { active: 'i', passive: 'i^2' },
  output: { active: 'A', passive: 'A^2' }, mapping: 'i -> A' }, false)
await check({ kind: 'plain', constraints: {
  active: 'A^5 X^2\nB^5 Y^2\nC^5 Z^2', passive: 'AX BYCZ\nBY CZ\nXYZ^2',
} })
console.log(`${checks} Basic FP fixtures passed through wasm${server ? ' and server' : ''}, including the custom-lattice benchmark problem.`)
