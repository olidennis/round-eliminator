import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'
import { problemPresets } from '../src/presets.ts'

await init({ module_or_path: await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const server = process.argv[2]
let checks = 0
async function check(definition, verify = () => {}) {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  assert.equal(parsed.type, 'problem')
  const before = structuredClone(parsed.data)
  const body = JSON.stringify({ type: 'maximize', data: { problem: parsed.data } })
  const events = []
  const response = JSON.parse(execute_with_events(body, raw => events.push(JSON.parse(raw))))
  assert.equal(response.type, 'maximization')
  assert.equal(response.data.kind, parsed.data.mapping.kind === 'unrestricted' ? 'output_only' : 'input_output')
  assert.ok(events.some(event => event.type === 'progress'))
  assert.ok(events.every(event => event.type !== 'progress' || event.data.phase !== 'parsing'))
  assert.equal(response.data.problem, undefined)
  assert.deepEqual(parsed.data, before)
  for (const group of response.data.constraint.degrees) {
    assert.ok(before.graph_class.passive.includes(group.degree))
    for (const config of group.configurations) {
      assert.equal(config.parts.reduce((n,p) => n + p.multiplicity, 0),group.degree)
      assert.ok(config.parts.every(part => part.labels.length && part.multiplicity > 0))
      if (response.data.kind === 'input_output') {
        for (const part of config.parts) for (const pair of part.labels) {
          const input = pair.input ?? 0
          assert.ok(before.mapping.data[input].includes(pair.output))
        }
      }
    }
  }
  verify(response.data, before)
  if (server) {
    const http = await fetch(`${server}/stream`, {method:'POST',headers:{'content-type':'application/json'},body})
    assert.equal(http.status,200)
    const updates = (await http.text()).split('\n\n').filter(frame => frame.includes('data:'))
      .map(frame => JSON.parse(frame.split('\n').find(line => line.startsWith('data:')).slice(5)))
    assert.equal(updates[0].data.phase,'queued')
    assert.deepEqual(updates.at(-1).data,response)
  }
  checks++
  return response.data
}
for (const preset of problemPresets) await check(preset.definition)
await check({kind:'plain', constraints:{active:'A B',passive:'A^3\nA^2 B\nA B^2\nB^3'}}, result => {
  assert.deepEqual(result.constraint.degrees,[{degree:3,configurations:[{parts:[{labels:[0,1],multiplicity:3}]}]}])
})
await check({kind:'plain',constraints:{active:'A',passive:'()\nA\nB'}}, result => {
  assert.equal(result.constraint.degrees.length,2)
  assert.equal(result.constraint.degrees[0].degree,0)
})
await check({kind:'mapped',input:{active:'i',passive:'i^2'},output:{active:'A',passive:'B^2'},mapping:'i -> A'},result => {
  assert.deepEqual(result.constraint.degrees,[])
})
await check({kind:'mapped',input:{active:'ij^100000',passive:'ij^100000'},output:{active:'A^100000',passive:'A^100000'},mapping:'i -> A\nj -> A'},result => {
  const [configuration] = result.constraint.degrees[0].configurations
  assert.equal(configuration.parts.length,1)
  assert.equal(configuration.parts[0].multiplicity,100000)
  assert.equal(configuration.parts[0].labels.length,2)
})
for (const size of [128, 129, 256, 257, 522]) {
  // Include every label at degree one and force a genuine closure operation
  // across the first/last bits at degree two, including dynamic masks above256.
  const labels = Array.from({length:size}, (_, i) => `(L${i})`)
  const first = labels[0], last = labels.at(-1)
  await check({kind:'plain',constraints:{active:`${first} ${last}`,
    passive:`${labels.join('')}\n${first}^2\n${last}^2\n${first} ${last}`}}, result => {
    const degreeOne = result.constraint.degrees.find(group => group.degree === 1)
    assert.equal(degreeOne.configurations[0].parts[0].labels.length,size)
    const degreeTwo = result.constraint.degrees.find(group => group.degree === 2)
    assert.equal(degreeTwo.configurations.length,1)
    const [part] = degreeTwo.configurations[0].parts
    assert.equal(part.multiplicity,2)
    assert.equal(part.labels.length,2)
  })
}
console.log(`${checks} maximization fixtures passed through wasm${server ? ' and the server' : ''}, including all presets, zero/nonregular degrees, impossible inputs, and condensed high-degree inputs.`)
