// Pass wasm package directories to compare snapshots with alternating samples.
// With no arguments, measure the application's current release wasm package.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const directories = process.argv.slice(2)
const modesOption = directories.indexOf('--compare-modes')
const modes = modesOption === -1 ? ['general'] : ['general', 'maximized']
if (modesOption !== -1) directories.splice(modesOption, 1)
const casesOption = directories.indexOf('--cases')
const casesFile = casesOption === -1
  ? new URL('./support/zero_round_cases.json', import.meta.url)
  : directories.splice(casesOption, 2)[1]
assert.ok(casesFile, '--cases requires a fixture file')
const cases = JSON.parse(await readFile(casesFile))
if (directories.length === 0) directories.push('web/public/wasm')
const implementations = []
for (const directory of directories) {
  const path = resolve(directory, 'round_eliminator_3_wasm.js')
  const { default: init, execute_json } = await import(pathToFileURL(path))
  await init({ module_or_path: await readFile(resolve(directory, 'round_eliminator_3_wasm_bg.wasm')) })
  for (const mode of modes) {
    implementations.push({ name: `${directory}:${mode}`, mode, execute: execute_json })
  }
}
console.log('case,implementation,median_ms_per_request')
for (const fixture of cases) {
  const requests = implementations.map(implementation => {
    const parsed = JSON.parse(implementation.execute(JSON.stringify({ type: 'parse_problem', data: fixture.definition })))
    assert.equal(parsed.type, 'problem', fixture.name)
    assert.ok(implementation.mode !== 'maximized' || fixture.passive_maximized,
      'Maximized mode requires checked fixtures')
    return JSON.stringify({ type: 'check_zero_round',
      data: { problem: parsed.data, passive_mode: implementation.mode } })
  })
  const samples = implementations.map(() => [])
  // Warm up each implementation and check agreement before timing it.
  implementations.forEach((implementation, index) => {
    for (let iteration = 0; iteration < 3; iteration++) {
      const response = JSON.parse(implementation.execute(requests[index]))
      assert.equal(response.type, 'zero_round', fixture.name)
      assert.equal(response.data.status, fixture.expected, fixture.name)
    }
  })
  for (let sample = 0; sample < 5; sample++) {
    const order = implementations.map((_, index) => index)
    if (sample % 2) order.reverse()
    for (const index of order) {
      const started = performance.now()
      let iterations = 0
      do {
        implementations[index].execute(requests[index])
        iterations++
      } while (performance.now() - started < 100)
      samples[index].push((performance.now() - started) / iterations)
    }
  }
  implementations.forEach((implementation, index) => {
    samples[index].sort((a, b) => a - b)
    console.log(`"${fixture.name}","${implementation.name}",${samples[index][2].toFixed(6)}`)
  })
}
