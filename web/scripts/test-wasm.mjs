import { encode, decode, decodeEvent, response as unwrap } from './protocol.mjs'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_progress } from '../public/wasm/round_eliminator_3_wasm.js'

const bytes = await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url))
await init({ module_or_path: bytes })

// Optionally pass a running server's /api URL to compare the two backends.
const serverUrl = process.argv[2]
function assertEquivalent(native, wasm) {
  assert.equal(native.type, wasm.type)
  if (wasm.type === 'zero_round') {
    assert.equal(native.data.status, wasm.data.status)
    // CaDiCaL and BatSat may choose different valid strategies. The native Rust
    // tests independently verify witnesses; compare decisions across transports.
    if (native.data.status === 'solvable') {
      assert.ok(Array.isArray(native.data.strategy))
    }
  } else {
    assert.deepEqual(native, wasm)
  }
}
async function execute(data, type = 'parse_problem', passiveMode = 'general') {
  if (type === 'check_zero_round') {
    const parsed = await execute(data)
    assert.equal(parsed.type, 'problem')
    data = { problem: parsed.data, passive_mode: passiveMode }
  }
  const request = encode({ type, data })
  const response = decode(execute_json(request))
  if (serverUrl) {
    const http = await fetch(serverUrl, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: request,
    })
    assert.equal(http.status, 200)
    assertEquivalent(unwrap(await http.json()), response)
  }
  return response
}

const constraints = (active, passive) => ({ active, passive })
const parsedForProgress = await execute({ kind: 'plain', constraints: constraints('A B', 'AB^3') })
const progressRequest = encode({ type: 'check_zero_round', data: { problem: parsedForProgress.data } })
const progress = []
const progressResult = decode(execute_with_progress(progressRequest, json => progress.push(JSON.parse(json))))
assert.deepEqual(progressResult, decode(execute_json(progressRequest)))
assert.equal(progress[0].phase, 'enumerating')
assert.equal(progress.at(-1).phase, 'verifying')
assert.ok(progress.every(update => ['enumerating', 'encoding', 'solving', 'verifying'].includes(update.phase)))
assert.equal(progress.at(-1).iterations, 1)
if (serverUrl) {
  const stream = await fetch(`${serverUrl}/stream`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: progressRequest,
  })
  assert.equal(stream.status, 200)
  assert.match(stream.headers.get('content-type'), /text\/event-stream/)
  const events = (await stream.text()).split('\n\n').flatMap(frame => {
    const data = frame.split('\n').find(line => line.startsWith('data:'))
    return data ? [decodeEvent(data.slice(5))] : []
  })
  assert.equal(events[0].data.phase, 'queued')
  // Scheduling differs; intermediate phases may be coalesced on either backend.
  const lastProgress = events.filter(event => event.type === 'progress').at(-1).data
  assert.equal(lastProgress.phase, 'verifying')
  assert.equal(lastProgress.iterations, 1)
  assert.equal(events.at(-1).type, 'response')
  assertEquivalent(events.at(-1).data, progressResult)
}
for (const [definition, status] of [
  [{ kind: 'plain', constraints: constraints('A B', 'A^3\nA^2 B\nA B^2\nB^3') }, 'solvable'],
  [{ kind: 'plain', constraints: constraints('A B', 'A^2 B\nA B^2') }, 'unsolvable'],
  [{ kind: 'plain', constraints: constraints('()\nA\nA^3', '()\nA^2') }, 'solvable'],
  [{ kind: 'independent', input: constraints('a^2', 'a^3'), output: constraints('A B', 'AB^3') }, 'solvable'],
  [{ kind: 'mapped', input: constraints('a b', 'a b'), output: constraints('A B', 'AB^2'), mapping: 'a -> A\nb -> B' }, 'solvable'],
  [{ kind: 'mapped', input: constraints('a b', 'a b'), output: constraints('A B', 'AB^2'), mapping: 'a -> A\nb -> A' }, 'unsolvable'],
  [{ kind: 'paired', constraints: constraints('(a,A) (b,B)\n(a,B) (c,A)', '(a,A)(a,B)\n(b,B)\n(c,A)') }, 'solvable'],
  [{ kind: 'paired', constraints: constraints('(a,A) (b,B)', '(a,B) (b,A)') }, 'unsolvable'],
]) {
  const result = await execute(definition, 'check_zero_round')
  assert.equal(result.type, 'zero_round')
  assert.equal(result.data.status, status)
  assert.equal('problem' in result.data, false)
  assert.equal('result' in result.data, false)
  if (status === 'solvable') assert.ok(Array.isArray(result.data.strategy))
}

// The same explicit mode reaches both wasm and the optional native server.
const maximizedFixtures = JSON.parse(await readFile(new URL(
  '../../crates/core/benches/support/zero_round_maximized_cases.json', import.meta.url)))
for (const fixture of maximizedFixtures) {
  for (const mode of ['general', 'maximized']) {
    const response = await execute(fixture.definition, 'check_zero_round', mode)
    assert.equal(response.type, 'zero_round', fixture.name)
    assert.equal(response.data.status, fixture.expected, fixture.name)
  }
}

const plain = await execute({ kind: 'plain', constraints: constraints('A\nA^3', 'A^2') })
assert.equal(plain.type, 'problem')
assert.deepEqual(plain.data.graph_class, { active: [1, 3], passive: [2] })

const empty = await execute({ kind: 'plain', constraints: constraints('', ''), degrees: { active: '3', passive: '2' } })
assert.equal(empty.type, 'problem')
assert.deepEqual(empty.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(empty.data.output.active.degrees, [])

const independent = await execute({
  kind: 'independent', input: constraints('a^3', 'b^2'), output: constraints('b', 'a'),
})
assert.equal(independent.type, 'problem')
assert.deepEqual(independent.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(independent.data.input.data.labels, ['a', 'b'])
assert.deepEqual(independent.data.output.labels, ['b', 'a'])

const paired = await execute({
  kind: 'paired', constraints: constraints('(red,X)(red,Y)^2\n(blue,Y)', '(red,X) (blue,Y)'),
})
assert.equal(paired.type, 'problem')
assert.equal(paired.data.input.kind, 'explicit')
assert.deepEqual(paired.data.graph_class, { active: [1, 2], passive: [2] })
assert.deepEqual(paired.data.input.data.labels, ['red', 'blue'])
assert.deepEqual(paired.data.output.labels, ['red,X', 'red,Y', 'blue,Y'])
assert.deepEqual(paired.data.output.active.degrees[1].configurations[0].parts[0].labels, [
  0, 1,
])
assert.deepEqual(paired.data.input.data.active.degrees[1].configurations[0].parts[0], { labels: [0], multiplicity: 2 })

const mapped = await execute({
  kind: 'mapped', input: constraints('a^3', 'b^2'), output: constraints('A', 'B'), mapping: 'a -> AB\nb ->',
})
assert.equal(mapped.type, 'problem')
assert.deepEqual(mapped.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(mapped.data.mapping.data, [[0, 1], []])

const zero = await execute({ kind: 'plain', constraints: constraints('()', '()') })
assert.deepEqual(zero.data.graph_class, { active: [0], passive: [0] })
assert.deepEqual(zero.data.output.active.degrees[0].configurations[0].parts, [])

const ranged = await execute({ kind: 'plain', constraints: constraints('AB^1..2 C^2..3', 'A^0..2') })
assert.equal(ranged.type, 'problem')
assert.deepEqual(ranged.data.graph_class, { active: [3, 4, 5], passive: [0, 1, 2] })
assert.equal(ranged.data.output.active.degrees.flatMap(group => group.configurations).length, 4)
assert.deepEqual(ranged.data.output.passive.degrees[0].configurations[0].parts, [])

const pairRange = await execute({ kind: 'paired', constraints: constraints('(a,X)(a,Y)^0..2', '(a,X)') })
assert.equal(pairRange.type, 'problem')
assert.deepEqual(pairRange.data.input.data.active.degrees.map(group => group.degree), [0, 1, 2])
assert.equal(pairRange.data.output.active.degrees[2].configurations[0].parts[0].labels.length, 2)

const invalidRange = await execute({ kind: 'plain', constraints: constraints('A\nA^3..1', 'A') })
assert.equal(invalidRange.type, 'error')
assert.deepEqual(invalidRange.data.location, { field: 'active', line: 2 })

const invalid = await execute({ kind: 'plain', constraints: constraints('A*', 'A') })
assert.equal(invalid.type, 'error')
assert.deepEqual(invalid.data.location, { field: 'active', line: 1 })
assert.match(invalid.data.message, /not supported/)

const invalidPairs = await execute({ kind: 'paired', constraints: constraints('(a,)', '(a,A)') })
assert.equal(invalidPairs.type, 'error')

const invalidMap = await execute({
  kind: 'mapped', input: constraints('a', 'b'), output: constraints('A', 'B'), mapping: 'a -> A',
})
assert.equal(invalidMap.type, 'error')
assert.equal(invalidMap.data.location.field, 'mapping')

const explicitPairDegrees = await execute({
  kind: 'paired', constraints: constraints('(a,A)^1..3', '(a,A)^2'), degrees: { active: '3, 4 3' },
})
assert.equal(explicitPairDegrees.type, 'problem')
assert.deepEqual(explicitPairDegrees.data.graph_class, { active: [3, 4], passive: [2] })
assert.deepEqual(explicitPairDegrees.data.input.data.active.degrees.map(group => group.degree), [1, 2, 3])

const invalidPairDegrees = await execute({
  kind: 'paired', constraints: constraints('(a,A)', '(a,A)'), degrees: { passive: '-1' },
})
assert.equal(invalidPairDegrees.type, 'error')
assert.deepEqual(invalidPairDegrees.data.location, { field: 'passive_degrees', line: null })



assert.deepEqual(plain.data.input, { kind: 'trivial' })
assert.deepEqual(plain.data.mapping, { kind: 'unrestricted' })
assert.deepEqual(independent.data.mapping, { kind: 'unrestricted' })
assert.deepEqual(paired.data.mapping, { kind: 'restricted', data: [[0, 1], [2]] })
assert.equal('kind' in paired.data, false)
assert.equal('presentation' in paired.data, false)

console.log(serverUrl ? 'All four variants passed wasm/server parity checks.' : 'All four variants passed wasm protocol checks.')
