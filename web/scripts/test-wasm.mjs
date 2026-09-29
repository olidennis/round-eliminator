import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json } from '../public/wasm/round_eliminator_3_wasm.js'

const bytes = await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url))
await init({ module_or_path: bytes })

// Optionally pass a running server's /api URL to check exact transport parity.
const serverUrl = process.argv[2]
async function execute(data) {
  const request = JSON.stringify({ type: 'parse_problem', data })
  const response = JSON.parse(execute_json(request))
  if (serverUrl) {
    const http = await fetch(serverUrl, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: request,
    })
    assert.equal(http.status, 200)
    assert.deepEqual(await http.json(), response)
  }
  return response
}

const constraints = (active, passive) => ({ active, passive })
const plain = await execute({ kind: 'plain', constraints: constraints('A\nA^3', 'A^2') })
assert.equal(plain.type, 'problem')
assert.deepEqual(plain.data.data.graph_class, { active: [1, 3], passive: [2] })

const empty = await execute({ kind: 'plain', constraints: constraints('', ''), degrees: { active: '3', passive: '2' } })
assert.equal(empty.type, 'problem')
assert.deepEqual(empty.data.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(empty.data.data.output.active.degrees, [])

const independent = await execute({
  kind: 'independent', input: constraints('a^3', 'b^2'), output: constraints('b', 'a'),
})
assert.equal(independent.type, 'problem')
assert.deepEqual(independent.data.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(independent.data.data.input.labels, ['a', 'b'])
assert.deepEqual(independent.data.data.output.labels, ['b', 'a'])

const paired = await execute({
  kind: 'paired', constraints: constraints('(red,X)(red,Y)^2\n(blue,Y)', '(red,X) (blue,Y)'),
})
assert.equal(paired.type, 'problem')
assert.equal(paired.data.kind, 'paired')
assert.deepEqual(paired.data.data.graph_class, { active: [1, 2], passive: [2] })
assert.deepEqual(paired.data.data.input.labels, ['red', 'blue'])
assert.deepEqual(paired.data.data.constraints.input_labels, paired.data.data.input.labels)
assert.deepEqual(paired.data.data.constraints.output_labels, ['X', 'Y'])
assert.deepEqual(paired.data.data.constraints.active.degrees[1].configurations[0].parts[0].labels, [
  { input: 0, output: 0 }, { input: 0, output: 1 },
])
assert.deepEqual(paired.data.data.input.active.degrees[1].configurations[0].parts[0], { labels: [0], multiplicity: 2 })

const mapped = await execute({
  kind: 'mapped', input: constraints('a^3', 'b^2'), output: constraints('A', 'B'), mapping: 'a -> AB\nb ->',
})
assert.equal(mapped.type, 'problem')
assert.deepEqual(mapped.data.data.graph_class, { active: [3], passive: [2] })
assert.deepEqual(mapped.data.data.allowed_outputs, [[0, 1], []])

const zero = await execute({ kind: 'plain', constraints: constraints('()', '()') })
assert.deepEqual(zero.data.data.graph_class, { active: [0], passive: [0] })
assert.deepEqual(zero.data.data.output.active.degrees[0].configurations[0].parts, [])

const ranged = await execute({ kind: 'plain', constraints: constraints('AB^1..2 C^2..3', 'A^0..2') })
assert.equal(ranged.type, 'problem')
assert.deepEqual(ranged.data.data.graph_class, { active: [3, 4, 5], passive: [0, 1, 2] })
assert.equal(ranged.data.data.output.active.degrees.flatMap(group => group.configurations).length, 4)
assert.deepEqual(ranged.data.data.output.passive.degrees[0].configurations[0].parts, [])

const pairRange = await execute({ kind: 'paired', constraints: constraints('(a,X)(a,Y)^0..2', '(a,X)') })
assert.equal(pairRange.type, 'problem')
assert.deepEqual(pairRange.data.data.input.active.degrees.map(group => group.degree), [0, 1, 2])
assert.equal(pairRange.data.data.constraints.active.degrees[2].configurations[0].parts[0].labels.length, 2)

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
assert.deepEqual(explicitPairDegrees.data.data.graph_class, { active: [3, 4], passive: [2] })
assert.deepEqual(explicitPairDegrees.data.data.input.active.degrees.map(group => group.degree), [1, 2, 3])

const invalidPairDegrees = await execute({
  kind: 'paired', constraints: constraints('(a,A)', '(a,A)'), degrees: { passive: '-1' },
})
assert.equal(invalidPairDegrees.type, 'error')
assert.deepEqual(invalidPairDegrees.data.location, { field: 'passive_degrees', line: null })

console.log(serverUrl ? 'All four variants passed wasm/server parity checks.' : 'All four variants passed wasm protocol checks.')
