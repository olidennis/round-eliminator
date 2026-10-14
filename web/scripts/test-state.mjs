import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'

await init({ module_or_path: await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const server = process.argv[2]
const empty = () => ({ maximization: null, zero_round: null, diagrams: [], coloring_bounds: [], coloring: [] })
const plain = (active, passive) => ({ kind: 'plain', constraints: { active, passive } })
let checks = 0

async function compute(state, operation, backend) {
  const body = JSON.stringify({ type: 'compute', data: { state, operation } })
  const events = []
  let packet
  if (backend === 'wasm') packet = JSON.parse(execute_with_events(body, raw => events.push(JSON.parse(raw))))
  else {
    const http = await fetch(`${server}/stream`, { method: 'POST', headers: { 'content-type': 'application/json' }, body })
    assert.equal(http.status, 200)
    for (const frame of (await http.text()).split('\n\n')) {
      const data = frame.split('\n').find(line => line.startsWith('data:'))
      if (data) events.push(JSON.parse(data.slice(5)))
    }
    packet = events.pop().data
  }
  assert.equal(packet.cache.problem, undefined)
  assert.ok(events.every(e => e.type !== 'progress' || e.data.phase !== 'parsing'))
  return { ...packet, events }
}
const coloring = (kind, search) => ({ type: 'check_coloring', data: { kind, search, passive_mode: 'general' } })
const diagram = kind => ({ type: 'compute_diagram', data: { options: { kind, output: 'exact', input_output: 'exact' } } })

for (const backend of server ? ['wasm', 'server'] : ['wasm']) {
  {
    const problem = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data:
      plain('A B', 'A^3\nA^2 B\nA B^2\nB^3') }))).response.data
    const state = { problem, analyses: empty() }
    const operation = { type: 'compute_diagram', data: { options:
      { kind: 'output_only', output: 'partial', input_output: 'cheap' } } }
    const partial = await compute(state, operation, backend)
    assert.equal(partial.response.data.diagram.complete, false)
    state.analyses = partial.cache
    const maximum = await compute(state, { type: 'maximize' }, backend)
    state.analyses.maximization = maximum.cache.maximization
    const upgraded = await compute(state, operation, backend)
    assert.equal(upgraded.response.data.diagram.complete, true)
    assert.equal(upgraded.response.data.options.output, 'exact')
    state.analyses.diagrams.push(upgraded.response.data)
    const exact = await compute(state, diagram('output_only'), backend)
    assert.deepEqual(exact.response.data.diagram, upgraded.response.data.diagram)
    assert.ok(!exact.events.some(e => e.type === 'progress' && e.data.phase === 'comparing'))
    checks++
  }
  for (const definition of [plain('A B', 'A^3\nA^2 B\nA B^2\nB^3'),
    { kind: 'mapped', input: { active: 'i^3\nj^3', passive: 'ij^2' },
      output: { active: 'A^3\nB^3\nC^3', passive: 'A C\nBC^2' }, mapping: 'i -> AB\nj -> C' },
  ]) {
    const problem = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition }))).response.data
    const state = { problem, analyses: empty() }
    const before = structuredClone(problem)
    const maximum = await compute(state, { type: 'maximize' }, backend)
    state.analyses.maximization = maximum.cache.maximization
    assert.deepEqual(maximum.response.data, state.analyses.maximization)
    const re = await compute(state, { type: 'apply_re' }, backend)
    assert.equal(re.response.type, 'problem')
    assert.deepEqual(re.cache, empty())
    assert.ok(!re.events.some(e => e.type === 'progress' && e.data.phase === 'maximizing'))
    for (const kind of ['output_only', 'input_output', 'input_aware']) {
      const operation = diagram(kind)
      const cached = await compute(state, operation, backend)
      const ordinary = await compute({ problem, analyses: empty() }, operation, backend)
      assert.deepEqual(cached.response, ordinary.response)
      state.analyses.diagrams.push(cached.response.data)
      const repeated = await compute(state, operation, backend)
      assert.deepEqual(repeated.response, cached.response)
      assert.deepEqual(repeated.cache, empty())
      assert.ok(!repeated.events.some(e => e.type === 'progress' && ['comparing','reducing'].includes(e.data.phase)))
      checks++
    }
    const zero = await compute(state, { type: 'check_zero_round', data: { passive_mode: 'general' } }, backend)
    state.analyses.zero_round = zero.cache.zero_round
    const repeated = await compute(state, { type: 'check_zero_round', data: { passive_mode: 'general' } }, backend)
    assert.deepEqual(repeated.response, zero.response)
    assert.deepEqual(repeated.cache, empty())
    assert.ok(!repeated.events.some(e => e.type === 'progress' && e.data.phase === 'solving'))
    assert.deepEqual(problem, before)
    checks++
  }
  for (const [definition, kind, expected] of [[plain('A^3\nB^3\nC^3', 'A BC\nB C'), 'vertex', 'maximum'],
    [plain('O I', 'O I'), 'distance_two', 'maximum'], [plain('OI^2','O I'), 'distance_two', 'unbounded']]) {
    const problem = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition }))).response.data
    const state = { problem, analyses: empty() }
    const maximum = await compute(state, coloring(kind, { kind: 'maximum' }), backend)
    assert.equal(maximum.response.data.status, expected)
    state.analyses = maximum.cache
    for (const colors of [3,4]) {
      const operation = coloring(kind, { kind: 'fixed', colors })
      const derived = await compute(state, operation, backend)
      const ordinary = await compute({ problem, analyses: empty() }, operation, backend)
      assert.equal(derived.response.data.status, ordinary.response.data.status)
      assert.ok(!derived.events.some(e => e.type === 'progress' && e.data.phase === 'solving'))
      if (derived.response.data.status === 'solvable') {
        const witness = derived.response.data.witness
        assert.equal(witness.behaviors.length, colors)
        if (kind === 'distance_two') for (const [own, row] of witness.behaviors.entries()) {
          assert.equal(row.outputs_by_neighbor.length, colors)
          assert.ok(row.outputs_by_neighbor[own].outputs_by_input.every(labels => labels.length === 0))
        }
      }
      checks++
    }
  }
}
console.log(`${checks} stateless-cache checks passed ${server ? 'wasm/server' : 'wasm'}: maxima, all diagram relations, saved proofs, palette restriction, and source preservation.`)
