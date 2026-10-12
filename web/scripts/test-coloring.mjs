import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'

await init({ module_or_path: await readFile(new URL(
  '../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const serverUrl = process.argv[2]
const constraints = (active, passive) => ({ active, passive })
const plain = (active, passive) => ({ kind: 'plain', constraints: constraints(active, passive) })
let checks = 0

function checkEvents(events, response, kind) {
  assert.ok(events.some(event => event.type === 'progress'))
  assert.ok(events.every(event => event.type !== 'progress' || event.data.phase !== 'parsing'))
  let lower = 0
  let upper = Infinity
  for (const event of events) {
    if (event.type !== 'partial_result') continue
    assert.equal(event.data.type, 'coloring_bounds')
    const bounds = event.data.data
    if (bounds.lower !== null) {
      assert.ok(bounds.lower >= lower)
      assert.equal(bounds.witness.kind, kind === 'distance_two' ? 'neighbor_colors' : 'local_colors')
      assert.equal(bounds.witness.behaviors.length, bounds.lower)
      lower = bounds.lower
    } else assert.equal(bounds.witness, null)
    if (bounds.upper !== null) { assert.ok(bounds.upper <= upper); upper = bounds.upper }
    assert.ok(lower <= upper)
  }
  if (response.data.status === 'maximum') {
    assert.equal(lower, response.data.colors)
    assert.equal(upper, lower)
    const queries = events.filter(event => event.type === 'partial_result')
      .map(event => event.data.data.testing).filter(colors => colors !== null)
    // Transports can skip superseded snapshots, including query announcements.
    // The core tests check that every palette is queried consecutively.
    assert.ok(queries.every((colors, index) => index === 0 || colors > queries[index - 1]))
  }
}

// Read the server stream incrementally, as the UI does. Bounds are protocol
// events of their own and precede the final response, rather than being nested
// in the completed result. Native and wasm solvers may select different witnesses.
async function nativeEvents(body) {
  const http = await fetch(`${serverUrl}/stream`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body,
  })
  assert.equal(http.status, 200)
  const events = []
  let buffer = ''
  const decoder = new TextDecoder()
  for await (const chunk of http.body) {
    buffer += decoder.decode(chunk, { stream: true })
    let boundary
    while ((boundary = buffer.indexOf('\n\n')) !== -1) {
      const frame = buffer.slice(0, boundary)
      buffer = buffer.slice(boundary + 2)
      const data = frame.split('\n').filter(line => line.startsWith('data:'))
        .map(line => line.slice(5).trimStart()).join('\n')
      if (data) events.push(JSON.parse(data))
    }
  }
  assert.equal(events[0].data.phase, 'queued')
  assert.equal(events.at(-1).type, 'response')
  return { events: events.slice(0, -1), response: events.at(-1).data }
}

async function check(definition, kind, search, expected, mode = 'general') {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  assert.equal(parsed.type, 'problem')
  const body = JSON.stringify({ type: 'check_coloring', data: {
    problem: parsed.data, kind, search, passive_mode: mode,
  } })
  const events = []
  const response = JSON.parse(execute_with_events(body, json => events.push(JSON.parse(json))))
  assert.equal(response.type, 'coloring', JSON.stringify(response))
  assert.equal(response.data.status, expected.status)
  if ('colors' in expected) assert.equal(response.data.colors, expected.colors)
  assert.equal('problem' in response.data, false)
  if (response.data.status === 'solvable') {
    const witness = response.data.witness
    assert.equal(witness.kind, kind === 'distance_two' ? 'neighbor_colors' : 'local_colors')
    assert.equal(witness.behaviors.length, search.colors)
    if (kind === 'distance_two') {
      for (const [own, behavior] of witness.behaviors.entries()) {
        assert.equal(behavior.outputs_by_neighbor.length, search.colors)
        assert.ok(behavior.outputs_by_neighbor[own].outputs_by_input.every(outputs => outputs.length === 0))
      }
    }
  }
  if (kind === 'distance_two' && response.data.status === 'unbounded') {
    assert.equal(response.data.witness.kind, 'color_order')
    assert.ok(response.data.witness.smaller.outputs_by_input)
    assert.ok(response.data.witness.larger.outputs_by_input)
  }
  checkEvents(events, response, kind)
  if (serverUrl) {
    const native = await nativeEvents(body)
    assert.equal(native.response.type, response.type)
    assert.equal(native.response.data.status, response.data.status)
    assert.equal(native.response.data.colors, response.data.colors)
    checkEvents(native.events, native.response, kind)
  }
  checks++
  return events
}

const coloring = plain('A^3\nB^3\nC^3', 'A BC\nB C')
const events = await check(coloring, 'vertex', { kind: 'maximum' }, { status: 'maximum', colors: 3 })
assert.equal(events.filter(event => event.type === 'partial_result').at(-1).data.data.lower, 3)
for (const [colors, status] of [[2, 'solvable'], [3, 'solvable'], [4, 'unsolvable']]) {
  await check(coloring, 'vertex', { kind: 'fixed', colors }, { status, colors })
}
await check(plain('A B\nA C\nB C', 'A A\nB B\nC C'), 'graph_edges', { kind: 'maximum' },
  { status: 'maximum', colors: 3 })
await check(plain('A\nB\nC', 'A B C'), 'incidence_edges', { kind: 'maximum' },
  { status: 'maximum', colors: 3 })
const matchingEvents = await check(plain('M U^9\nP^10', 'M UP^9\nU^10'), 'incidence_edges',
  { kind: 'maximum' }, { status: 'maximum', colors: 10 })
assert.equal(matchingEvents.filter(event => event.type === 'partial_result').at(-1).data.data.lower, 10)

// Both the original inputs and colors are necessary: a and b have disjoint
// output lists, while an arbitrary proper coloring still has to be reduced.
const mapped = { kind: 'mapped', input: constraints('a^2\nb^2', 'ab^2'),
  output: constraints('A^2\nB^2\nC^2\nD^2', 'A BCD\nB CD\nC D'), mapping: 'a -> AB\nb -> CD' }
for (const [colors, status] of [[2, 'solvable'], [3, 'unsolvable']]) {
  await check(mapped, 'vertex', { kind: 'fixed', colors }, { status, colors })
}
await check({ kind: 'independent', input: constraints('a^2', 'a^2'),
  output: coloring.constraints }, 'vertex', { kind: 'maximum' }, { status: 'no_palette' })
await check({ kind: 'paired', constraints: constraints('(a,A)^2\n(a,B)^2', '(a,A) (a,B)') },
  'vertex', { kind: 'maximum' }, { status: 'maximum', colors: 2 })
await check(plain('()\nA\nA^3', '()\nA^2'), 'incidence_edges', { kind: 'maximum' }, { status: 'unbounded' })

// These passive constraints are manually maximized. No maximization is run.
for (const mode of ['general', 'maximized']) {
  const distanceTwoEvents = await check(plain('O I', 'O I'), 'distance_two', { kind: 'maximum' },
    { status: 'maximum', colors: 3 }, mode)
  assert.equal(distanceTwoEvents.filter(event => event.type === 'partial_result').at(-1).data.data.lower, 3)
  await check(plain('OI^2', 'O I'), 'distance_two', { kind: 'maximum' },
    { status: 'unbounded' }, mode)
  await check(plain('O I', 'O I'), 'distance_two', { kind: 'fixed', colors: 3 },
    { status: 'solvable', colors: 3 }, mode)
  await check(plain('O I', 'O I'), 'distance_two', { kind: 'fixed', colors: 4 },
    { status: 'unsolvable', colors: 4 }, mode)
  await check(plain('A^2\nB^2', 'A B'), 'vertex', { kind: 'maximum' }, { status: 'maximum', colors: 2 }, mode)
  await check(plain('A B', 'AB^2'), 'incidence_edges', { kind: 'maximum' }, { status: 'unbounded' }, mode)
  await check(plain('A B\nA C\nB C', 'A A\nB B\nC C'), 'graph_edges', { kind: 'maximum' },
    { status: 'maximum', colors: 3 }, mode)
}

await check(plain('A B C', 'A A\nB B\nC C'), 'distance_two', { kind: 'fixed', colors: 4 },
  { status: 'solvable', colors: 4 })
await check(plain('A B C', 'A A\nB B\nC C'), 'distance_two', { kind: 'fixed', colors: 5 },
  { status: 'unsolvable', colors: 5 })
await check({ kind: 'mapped', input: constraints('a b', 'a b'),
  output: constraints('O I', 'O I'), mapping: 'a -> O\nb -> I' },
  'distance_two', { kind: 'fixed', colors: 3 }, { status: 'solvable', colors: 3 })
await check({ kind: 'paired', constraints: constraints('(a,O) (b,I)', '(a,O) (b,I)') },
  'distance_two', { kind: 'fixed', colors: 3 }, { status: 'solvable', colors: 3 })
await check(plain('()\nA\nA B', 'AB^2'), 'distance_two', { kind: 'fixed', colors: 3 },
  { status: 'solvable', colors: 3 })
await check(plain('()', 'O I'), 'distance_two', { kind: 'fixed', colors: 1 },
  { status: 'solvable', colors: 1 })
await check(plain('()\nOI\nOI^3', 'O I'), 'distance_two', { kind: 'maximum' },
  { status: 'unbounded' })
await check({ kind: 'mapped', input: constraints('a^2\nb^2', 'ab^2'),
  output: constraints('AB^2\nCD^2', 'AC BD'), mapping: 'a -> AB\nb -> CD' },
  'distance_two', { kind: 'maximum' }, { status: 'unbounded' })
await check({ kind: 'paired', constraints: constraints('(a,O)(a,I)^2\n(b,O)(b,I)^2', '(a,O)(b,O) (a,I)(b,I)') },
  'distance_two', { kind: 'maximum' }, { status: 'unbounded' })
await check({ kind: 'plain', constraints: constraints('A^2', ''), degrees: { active: '2', passive: '2' } },
  'distance_two', { kind: 'maximum' }, { status: 'no_palette' })

for (const [definition, search] of [
  [plain('O I', 'O I'), { kind: 'fixed', colors: 2 }],
  [plain('A', 'A^3'), { kind: 'fixed', colors: 3 }],
]) {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  const body = JSON.stringify({ type: 'check_coloring', data: {
    problem: parsed.data, kind: 'distance_two', search,
  } })
  const response = JSON.parse(execute_json(body))
  assert.equal(response.type, 'error')
  if (serverUrl) {
    const native = await nativeEvents(body)
    assert.equal(native.response.type, 'error')
    assert.equal(native.response.data.message, response.data.message)
  }
  checks++
}

console.log(`${checks} coloring checks passed ${serverUrl ? 'wasm/server' : 'wasm'} checks, including streamed bounds.`)
