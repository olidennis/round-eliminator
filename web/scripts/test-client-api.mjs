import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import ts from 'typescript'

const moduleURL = code => 'data:text/javascript;base64,' + Buffer.from(code).toString('base64')
const transpile = source => ts.transpileModule(source, { compilerOptions: {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
} }).outputText
const diagram = moduleURL(transpile(await readFile(new URL('../src/diagram.ts', import.meta.url), 'utf8')))
const source = transpile(await readFile(new URL('../src/api.ts', import.meta.url), 'utf8'))
  .replace(/from ['"]\.\/diagram['"]/, 'from ' + JSON.stringify(diagram))
const { sendForProblem } = await import(moduleURL(source))
const empty = () => ({ maximization: null, zero_round: null, coloring: [], diagrams: [] })
const state = { problem: { marker: 'immutable source' }, analyses: empty() }
const before = structuredClone(state.problem)
const requests = []
const pending = new Map()
const stream = events => new Response(events.map(event => `data: ${JSON.stringify(event)}\n\n`).join(''),
  { headers: { 'content-type': 'text/event-stream' } })
globalThis.fetch = (_url, options) => new Promise(resolve => {
  const request = JSON.parse(options.body)
  requests.push(request)
  pending.set(request.data.operation.type, events => resolve(stream(events)))
})
const maximum = { kind: 'output_only', constraint: { degrees: [] } }
const diagramResult = { options: { kind: 'output_only', output: 'exact', input_output: 'cheap' },
  diagram: { nodes: [], successors: [], components: [], edges: [], complete: true } }
const a = sendForProblem(state, { type: 'maximize' }, 'server')
const b = sendForProblem(state, { type: 'compute_diagram', data: { options: diagramResult.options } }, 'server')
assert.equal(requests.length, 2)
assert.ok(requests.every(request => request.type === 'compute'))
assert.ok(requests.every(request => request.data.state.analyses.maximization === null))
pending.get('compute_diagram')([{ type: 'response', data: {
  response: { type: 'diagram', data: diagramResult }, cache: { ...empty(), diagrams: [diagramResult] },
} }])
await b
pending.get('maximize')([{ type: 'response', data: {
  response: { type: 'maximization', data: maximum }, cache: { ...empty(), maximization: maximum },
} }])
await a
assert.deepEqual(state.analyses.diagrams, [diagramResult], 'A late completion must not overwrite another computation.')
assert.deepEqual(state.analyses.maximization, maximum)
assert.deepEqual(state.problem, before)

// Completed source entries survive a later cancellation. This is independent
// of progress/proof handling and uses the real client stream parser.
const cancellation = new AbortController()
const sourceState = { problem: before, analyses: empty() }
const cancelled = sendForProblem(sourceState, { type: 'apply_re' }, 'server', {
  signal: cancellation.signal, onCache: cache => { if (cache.maximization) cancellation.abort() },
})
pending.get('apply_re')([
  { type: 'cache_update', data: { ...empty(), maximization: maximum } },
  { type: 'response', data: { response: { type: 'cancelled' }, cache: empty() } },
])
await assert.rejects(cancelled, error => error.name === 'AbortError')
assert.deepEqual(sourceState.analyses.maximization, maximum)
assert.deepEqual(sourceState.problem, before)
console.log('Client API checks passed: shared state envelope, concurrent cache merging, immutable source and completed entries retained on cancellation.')
