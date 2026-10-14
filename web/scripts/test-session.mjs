import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import ts from 'typescript'

// This codec uses browser-standard streams, also available in Node. Transpile
// its type-only imports so the checks don't require a TypeScript runtime loader.
const source = await readFile(new URL('../src/session.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } })
const { newSavedProblem, serializeSession, deserializeSession, sessionLink, sessionFromHash, coloringQuery, coloringKey } =
  await import('data:text/javascript;base64,' + Buffer.from(outputText).toString('base64'))

const problem = {
  graph_class: { active: [0], passive: [2] }, input: { kind: 'trivial' },
  mapping: { kind: 'unrestricted' }, output: {
    labels: ['α', 'blue 💙'], active: { degrees: [{ degree: 0, configurations: [{ parts: [] }] }] },
    passive: { degrees: [] },
  },
}
const saved = newSavedProblem(problem)
assert.equal(saved.diagram.layout, 'physics')
assert.equal(saved.state.analyses.coloring.length, 0)
assert.equal(saved.state.analyses.maximization, null)
assert.equal(coloringKey(coloringQuery(saved.coloring)), coloringKey(coloringQuery({ ...saved.coloring, colors: 10 })))
assert.notEqual(coloringKey({ kind: 'vertex', search: { kind: 'fixed', colors: 3 } }),
  coloringKey({ kind: 'vertex', search: { kind: 'fixed', colors: 4 } }))
assert.notEqual(coloringKey({ kind: 'vertex', search: { kind: 'maximum' } }),
  coloringKey({ kind: 'distance_two', search: { kind: 'maximum' } }))
saved.collapsed = true
saved.collapsedSections = { output: true, outputPassive: true, coloring: true }
saved.state.analyses.zero_round = { status: 'unsolvable' }
saved.state.analyses.maximization = { kind: 'input_output', constraint: { degrees: [
  { degree: 2, configurations: [{ parts: [{ labels: [{ input: null, output: 0 }], multiplicity: 2 }] }] },
] } }
saved.collapsedSections.maximization = true
saved.diagram.options = { kind: 'input_output', output: 'partial', input_output: 'exact' }
saved.diagram.layout = 'hierarchical'
const diagram = { nodes: [{ input: 0, output: 1 }], successors: [[0]], components: [[0]], edges: [], complete: true }
saved.state.analyses.diagrams = [{ options: saved.diagram.options, diagram }]
saved.collapsedSections.diagram = true
saved.coloring.kind = 'distance_two'
saved.coloring.bounds = { minimum: 1, lower: 3, upper: null, testing: 6,
  witness: { kind: 'color_order', smaller: { outputs_by_input: [[0]] }, larger: { outputs_by_input: [[1]] } },
}
const session = { editor: { kind: 'plain', constraints: { active: '()', passive: '(α) (blue 💙)' }, degrees: { active: '', passive: '' } },
  problems: [saved, newSavedProblem(problem)],
}
saved.state.analyses.coloring = [
  { kind: 'vertex', search: { kind: 'maximum' }, result: { status: 'maximum', colors: 3,
      witness: { kind: 'local_colors', behaviors: [] } } },
  { kind: 'distance_two', search: { kind: 'fixed', colors: 4 }, result: { status: 'unsolvable', colors: 4 } },
]
const expected = structuredClone(session)
expected.problems[0].coloring.bounds.testing = null
assert.deepEqual(deserializeSession(serializeSession(session)), expected)
const freeSession = structuredClone(expected)
freeSession.problems[0].diagram.layout = 'free'
assert.deepEqual(deserializeSession(serializeSession(freeSession)), freeSession)
const graphSectionSession = JSON.parse(serializeSession(session))
graphSectionSession.session.problems[0].collapsedSections.graph = true
assert.deepEqual(deserializeSession(JSON.stringify(graphSectionSession)), expected,
  'Legacy graph-class folding must not hide header metadata or remain in saved state.')
assert.equal(saved.coloring.bounds.testing, 6, 'Saving must not mutate a live query.')
// Version-one caches lived beside the controls. Migrate every entry and
// preserve the original problem independently of those artifacts.
const oldFile = JSON.parse(serializeSession(session))
oldFile.version = 1
for (const entry of oldFile.session.problems) {
  const { problem, analyses } = entry.state
  entry.problem = problem
  entry.zeroRound = analyses.zero_round
  entry.maximization = analyses.maximization
  entry.coloring.results = analyses.coloring
  entry.diagram.results = analyses.diagrams
  delete entry.state
}
assert.deepEqual(deserializeSession(JSON.stringify(oldFile)), expected)
const oldColoring = structuredClone(oldFile)
const current = oldColoring.session.problems[0].coloring
current.result = current.results[1].result
current.kind = 'distance_two'; current.search = 'fixed'; current.colors = 4
current.results = undefined
const migrated = deserializeSession(JSON.stringify(oldColoring)).problems[0]
assert.deepEqual(migrated.state.analyses.coloring,
  [{ kind: 'distance_two', search: { kind: 'fixed', colors: 4 }, result: current.result }])
assert.equal('result' in migrated.coloring, false)
assert.equal('results' in migrated.coloring, false)
const legacy = structuredClone(oldFile)
delete legacy.session.problems[0].diagram
delete legacy.session.problems[0].maximization
assert.deepEqual(deserializeSession(JSON.stringify(legacy)).problems[0].state.analyses.diagrams, [])
assert.equal(deserializeSession(JSON.stringify(legacy)).problems[0].state.analyses.maximization, null)
const olderDiagrams = structuredClone(oldFile)
olderDiagrams.session.problems[0].diagram = {
  options: { output: 'partial', input_output: 'exact' }, view: 'input_specific', layout: 'physics',
  result: { options: { output: 'partial', input_output: 'exact' }, output: diagram, input_specific: diagram, uniform: diagram },
}
const restored = deserializeSession(JSON.stringify(olderDiagrams)).problems[0]
assert.equal(restored.diagram.options.kind, 'input_output')
assert.deepEqual(restored.state.analyses.diagrams.map(result => result.options.kind), ['output_only', 'input_output', 'input_aware'])
assert.ok(restored.state.analyses.diagrams.every(result => result.diagram.nodes[0].output === 1))
const olderCache = structuredClone(oldFile)
olderCache.session.problems[0].diagram.options.kind = 'uniform'
olderCache.session.problems[0].diagram.results[0].options.kind = 'uniform'
const renamed = deserializeSession(JSON.stringify(olderCache)).problems[0]
assert.equal(renamed.diagram.options.kind, 'input_aware')
assert.equal(renamed.state.analyses.diagrams[0].options.kind, 'input_aware')

for (const editor of [
  { kind: 'independent', input: session.editor.constraints, output: session.editor.constraints, degrees: session.editor.degrees },
  { kind: 'mapped', input: session.editor.constraints, output: session.editor.constraints, mapping: '(α) -> (blue 💙)', degrees: session.editor.degrees },
  { kind: 'paired', constraints: { active: '(α,A)^2', passive: '(α,A) (α,A)' }, degrees: session.editor.degrees },
]) {
  assert.deepEqual(deserializeSession(serializeSession({ ...session, editor })).editor, editor)
}

globalThis.location = { href: 'https://example.org/re3/?backend=wasm#previous' }
const url = new URL(await sessionLink(session))
assert.equal(url.origin + url.pathname, 'https://example.org/re3/')
assert.equal(url.search, '?backend=wasm')
assert.match(url.hash, /^#re3=[A-Za-z0-9_-]+$/)
assert.deepEqual(await sessionFromHash(url.hash), expected)
const empty = { editor: session.editor, problems: [] }
assert.deepEqual(await sessionFromHash(new URL(await sessionLink(empty)).hash), empty)
// Use a payload larger than argument-spread limits, including Unicode names.
const large = { ...session, problems: Array.from({ length: 1000 }, () => saved) }
assert.equal((await sessionFromHash(new URL(await sessionLink(large)).hash)).problems.length, 1000)
assert.throws(() => deserializeSession('{'), SyntaxError)
assert.throws(() => deserializeSession(JSON.stringify({ v: 'old', d: {} })), /not a Round Eliminator 3 session/)
assert.throws(() => deserializeSession(JSON.stringify({ format: 'round-eliminator-3', version: 3 })), /Unsupported/)
assert.throws(() => deserializeSession(JSON.stringify({ format: 'round-eliminator-3', version: 1, session: {} })), /missing/)
await assert.rejects(sessionFromHash('#old-link'), /not a Round Eliminator 3/)
await assert.rejects(sessionFromHash('#re3=invalid'))
console.log('Session checks passed: files, compressed links, Unicode, multiple/empty sessions, large payloads, retained proofs, and invalid formats.')
