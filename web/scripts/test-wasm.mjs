import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json } from '../public/wasm/round_eliminator_3_wasm.js'

const bytes = await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url))
await init({ module_or_path: bytes })

const request = (active, passive) => JSON.stringify({
  type: 'parse_problem',
  data: { active, passive },
})

const valid = JSON.parse(execute_json(request('A\nA^3', 'A^2')))
assert.equal(valid.type, 'problem')
assert.deepEqual(valid.data.active.degrees.map((group) => group.degree), [1, 3])
assert.equal(valid.data.passive.degrees[0].degree, 2)

const invalid = JSON.parse(execute_json(request('A*', 'A')))
assert.equal(invalid.type, 'error')
assert.equal(invalid.data.location.side, 'active')
assert.match(invalid.data.message, /not supported/)

console.log('Wasm protocol smoke test passed.')
