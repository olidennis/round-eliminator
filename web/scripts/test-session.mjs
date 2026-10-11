import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import ts from 'typescript'

// This codec uses browser-standard streams, also available in Node. Transpile
// its type-only imports so the checks don't require a TypeScript runtime loader.
const source = await readFile(new URL('../src/session.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } })
const { newSavedProblem, serializeSession, deserializeSession, sessionLink, sessionFromHash } =
  await import('data:text/javascript;base64,' + Buffer.from(outputText).toString('base64'))

const problem = {
  graph_class: { active: [0], passive: [2] }, input: { kind: 'trivial' },
  mapping: { kind: 'unrestricted' }, output: {
    labels: ['α', 'blue 💙'], active: { degrees: [{ degree: 0, configurations: [{ parts: [] }] }] },
    passive: { degrees: [] },
  },
}
const saved = newSavedProblem(problem)
saved.collapsed = true
saved.collapsedSections = { output: true, outputPassive: true, coloring: true }
saved.zeroRound = { status: 'unsolvable' }
saved.coloring.kind = 'distance_two'
saved.coloring.bounds = { minimum: 1, lower: 3, upper: null, testing: 6,
  witness: { kind: 'color_order', smaller: { outputs_by_input: [[0]] }, larger: { outputs_by_input: [[1]] } },
}
const session = { editor: { kind: 'plain', constraints: { active: '()', passive: '(α) (blue 💙)' }, degrees: { active: '', passive: '' } },
  problems: [saved, newSavedProblem(problem)],
}
const expected = structuredClone(session)
expected.problems[0].coloring.bounds.testing = null
assert.deepEqual(deserializeSession(serializeSession(session)), expected)
assert.equal(saved.coloring.bounds.testing, 6, 'Saving must not mutate a live query.')

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
assert.throws(() => deserializeSession(JSON.stringify({ format: 'round-eliminator-3', version: 2 })), /Unsupported/)
assert.throws(() => deserializeSession(JSON.stringify({ format: 'round-eliminator-3', version: 1, session: {} })), /missing/)
await assert.rejects(sessionFromHash('#old-link'), /not a Round Eliminator 3/)
await assert.rejects(sessionFromHash('#re3=invalid'))
console.log('Session checks passed: files, compressed links, Unicode, multiple/empty sessions, large payloads, retained proofs, and invalid formats.')
