import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json } from '../public/wasm/round_eliminator_3_wasm.js'
import { problemPresets } from '../src/presets.ts'

await init({ module_or_path: await readFile(new URL(
  '../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const serverUrl = process.argv[2]

async function execute(request) {
  const body = JSON.stringify(request)
  const response = JSON.parse(execute_json(body))
  if (serverUrl) {
    const http = await fetch(serverUrl, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    })
    assert.equal(http.status, 200)
    const native = await http.json()
    if (response.type === 'zero_round') {
      assert.equal(native.type, response.type)
      assert.equal(native.data.status, response.data.status)
    } else {
      assert.deepEqual(native, response)
    }
  }
  return response
}

const parsed = new Map()
for (const preset of problemPresets) {
  const response = await execute({ type: 'parse_problem', data: preset.definition })
  assert.equal(response.type, 'problem', `${preset.name}: ${JSON.stringify(response)}`)
  assert.deepEqual(response.data.graph_class, {
    active: [preset.name === '5-coloring with outdegree 2 (degree 5)' ? 5 : 3],
    passive: [preset.name === 'Bipartite maximal matching' ? 3 : 2],
  }, preset.name)
  parsed.set(preset.name, response.data)
  const result = await execute({ type: 'check_zero_round', data: { problem: response.data } })
  assert.equal(result.type, 'zero_round', preset.name)
  const expected = preset.name.startsWith('Edge grabbing from sinkless') ? 'solvable' : 'unsolvable'
  assert.equal(result.data.status, expected, preset.name)
}

function* tuples(alphabet, degree, prefix = []) {
  if (degree === 0) { yield prefix; return }
  for (const label of alphabet) yield* tuples(alphabet, degree - 1, [...prefix, label])
}

// Independent exhaustive membership for these small degree-two/three examples.
// Try bijections between concrete occurrences and the condensed allowed slots.
function accepts(constraint, names, labels) {
  const ids = labels.map(label => names.indexOf(label))
  if (ids.includes(-1)) return false
  return (constraint.degrees.find(group => group.degree === ids.length)?.configurations ?? [])
    .some(configuration => {
      const slots = configuration.parts.flatMap(part => Array(part.multiplicity).fill(part.labels))
      function match(position, remaining) {
        if (position === ids.length) return true
        return remaining.some((slot, index) => slot.includes(ids[position]) &&
          match(position + 1, remaining.filter((_, other) => other !== index)))
      }
      return match(0, slots)
    })
}

// Check the two list-coloring entries agree on every local input/output tuple,
// including forbidden pairs and inputs that violate the uniform-vertex promise.
const mapped = parsed.get('List coloring (mapped)')
const paired = parsed.get('List coloring (paired)')
assert.equal(paired.input.data.active.degrees[0].configurations.length, 3)
assert.equal(paired.output.active.degrees[0].configurations.length, 6)
const alphabet = ['r', 'g', 'b'].flatMap(input => ['A', 'B', 'C'].map(output => [input, output]))
for (const [side, degree] of [['active', 3], ['passive', 2]]) {
  for (const tuple of tuples(alphabet, degree)) {
    const inputs = tuple.map(([input]) => input)
    const outputs = tuple.map(([, output]) => output)
    const allowed = accepts(mapped.input.data[side], mapped.input.data.labels, inputs) &&
      accepts(mapped.output[side], mapped.output.labels, outputs) &&
      tuple.every(([input, output]) => mapped.mapping.data[mapped.input.data.labels.indexOf(input)]
        .includes(mapped.output.labels.indexOf(output)))
    assert.equal(accepts(paired.output[side], paired.output.labels,
      tuple.map(([input, output]) => `${input},${output}`)), allowed, `${side}: ${JSON.stringify(tuple)}`)
  }
  for (const input of tuples(['r', 'g', 'b'], degree)) {
    assert.equal(accepts(paired.input.data[side], paired.input.data.labels, input),
      accepts(mapped.input.data[side], mapped.input.data.labels, input))
  }
}

console.log(`${problemPresets.length} presets passed ${serverUrl ? 'wasm/server' : 'wasm'} checks; mapped/paired list coloring is locally equivalent.`)
