import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import init, { execute_json, execute_with_events } from '../public/wasm/round_eliminator_3_wasm.js'
import { problemPresets } from '../src/presets.ts'
import { diagramKinds, reuseOutputOnlyDiagram } from '../src/diagram.ts'

await init({ module_or_path: await readFile(new URL('../public/wasm/round_eliminator_3_wasm_bg.wasm', import.meta.url)) })
const server = process.argv[2]
let checks = 0
const plain = passive => ({ kind: 'plain', constraints: { active: 'A B C', passive } })
const edge = (graph, input, output, target) => {
  const source = graph.nodes.findIndex(node => node.input === input && node.output === output)
  const destination = graph.nodes.findIndex(node => node.input === input && node.output === target)
  return source >= 0 && destination >= 0 && graph.successors[source].includes(destination)
}
async function check(definition, options, verify = () => {}) {
  const parsed = JSON.parse(execute_json(JSON.stringify({ type: 'parse_problem', data: definition })))
  assert.equal(parsed.type, 'problem')
  const body = JSON.stringify({ type: 'compute_diagram', data: { problem: parsed.data, options } })
  const events = []
  const response = JSON.parse(execute_with_events(body, raw => events.push(JSON.parse(raw))))
  assert.equal(response.type, 'diagram')
  assert.ok(events.some(event => event.type === 'progress'))
  assert.equal(events.at(-1).data.phase, 'reducing')
  assert.ok(events.every(event => event.type !== 'progress' || event.data.phase !== 'parsing'))
  assert.equal(response.data.problem, undefined)
  assert.deepEqual(Object.keys(response.data).sort(), ['diagram', 'options'])
  assert.equal(response.data.options.kind, options.kind)
  const graph = response.data.diagram
  assert.deepEqual(graph.components.flat().sort((a,b) => a-b), graph.nodes.map((_,i) => i))
  graph.successors.forEach((row,i) => assert.ok(row.includes(i)))
  assert.ok(graph.edges.every(([a,b]) => a !== b && graph.components[a] && graph.components[b]))
  if (options.kind === 'input_output') {
    graph.successors.forEach((row,i) => row.forEach(j => assert.equal(graph.nodes[i].input,graph.nodes[j].input)))
  }
  verify(graph, parsed.data)
  if (server) {
    const http = await fetch(`${server}/stream`, { method: 'POST', headers: {'content-type':'application/json'}, body })
    assert.equal(http.status, 200)
    const native = (await http.text()).split('\n\n').filter(frame => frame.includes('data:'))
      .map(frame => JSON.parse(frame.split('\n').find(line => line.startsWith('data:')).slice(5)))
    assert.equal(native[0].data.phase, 'queued')
    assert.deepEqual(native.at(-1).data,response)
  }
  checks++
  return graph
}
async function checkAll(definition, options, verify = () => {}) {
  const result = {}
  for (const kind of ['output_only', 'input_output', 'input_aware']) {
    result[kind] = await check(definition, { ...options, kind })
  }
  verify(result)
  return result
}
for (const preset of problemPresets) {
  for (const input_output of ['cheap','exact']) {
    const graphs = await checkAll(preset.definition, {output:'exact',input_output})
    if (preset.name === 'Input-aware label replacement (mapped)') {
      const parsed = JSON.parse(execute_json(JSON.stringify({type:'parse_problem',data:preset.definition}))).data
      assert.deepEqual(diagramKinds(parsed),['output_only','input_output','input_aware'])
      const output = name => parsed.output.labels.indexOf(name)
      const input = parsed.input.data.labels.indexOf('i')
      assert.ok(graphs.input_output.edges.length > 0, 'The displayed pair graph must contain an arrow after SCC grouping.')
      assert.ok(graphs.input_aware.edges.length > 0, 'The displayed input-aware graph must contain an arrow after SCC grouping.')
      assert.equal(edge(graphs.output_only,null,output('A'),output('B')),true)
      assert.equal(edge(graphs.output_only,null,output('B'),output('C')),true)
      assert.equal(edge(graphs.input_output,input,output('A'),output('B')),true)
      assert.equal(edge(graphs.input_aware,null,output('A'),output('B')),true)
      assert.equal(edge(graphs.input_output,input,output('B'),output('C')),false)
      assert.equal(edge(graphs.input_aware,null,output('B'),output('C')),false)
      assert.equal(edge(graphs.input_output,input,output('B'),output('A')),false)
      assert.equal(edge(graphs.input_aware,null,output('B'),output('A')),false)
      assert.equal(graphs.input_output.edges.length,1)
      assert.equal(graphs.input_aware.edges.length,1)
    }
  }
}
for (const output of ['partial','maximized','exact']) {
  for (const input_output of ['cheap','exact']) {
    await checkAll(plain('ABC^3'),{output,input_output}, result => {
      assert.deepEqual(result.output_only.components,[[0,1,2]])
      assert.deepEqual(result.output_only.edges,[])
      assert.equal(result.output_only.complete,output !== 'partial')
    })
  }
}
const mapped = {kind:'mapped', input:{active:'i j k',passive:'i j\nj k'},
  output:{active:'A B X Y',passive:'A X\nB X\nA Y'},mapping:'i -> AB\nj -> X\nk -> Y'}
for (const input_output of ['cheap','exact']) await checkAll(mapped,{output:'exact',input_output},result => {
  assert.equal(edge(result.output_only,null,0,1),false)
  assert.equal(edge(result.input_output,0,0,1),input_output === 'exact')
  assert.equal(edge(result.input_aware,null,0,1),input_output === 'exact')
})
await checkAll({kind:'mapped',input:{active:'i j',passive:'ij^2'},output:{active:'A B',passive:'AB^2'},mapping:'i -> AB\nj -> A'},
  {output:'exact',input_output:'exact'},result => {
    assert.equal(edge(result.input_output,0,0,1),true)
    assert.equal(edge(result.input_aware,null,0,1),false)
  })
await checkAll({kind:'plain',constraints:{active:'()',passive:'()\nA B\nB^2'},degrees:{active:'0',passive:'0 2'}},
  {output:'exact',input_output:'exact'})
await checkAll({kind:'plain',constraints:{active:'',passive:''}}, {output:'exact',input_output:'exact'}, result => {
  assert.deepEqual(result.output_only.components,[])
})
// Visibility must preserve distinctions caused by nonregular input promises.
for (const [input, output, degrees, kinds] of [
  [{active:'i j',passive:'i j'}, {active:'A B',passive:'A B\nB^2'}, undefined, ['output_only']],
  [{active:'i j',passive:'i\nj^2'}, {active:'A B',passive:'B\nAB^2'}, undefined, ['output_only','input_output']],
  [{active:'i j',passive:'ij^2'}, {active:'A B',passive:'B\nAB^2'}, {active:'2',passive:'1 2'}, ['output_only','input_output','input_aware']],
  [{active:'i j',passive:'()'}, {active:'A B',passive:'()'}, undefined, ['output_only']],
]) {
  const definition = {kind:'independent',input,output,...(degrees ? {degrees} : {})}
  const parsed = JSON.parse(execute_json(JSON.stringify({type:'parse_problem',data:definition}))).data
  assert.deepEqual(diagramKinds(parsed),kinds)
  for (const method of ['partial','exact']) {
    const graphs = await checkAll(definition,{output:method,input_output:'cheap'})
    for (const kind of ['input_output','input_aware'].filter(kind => !kinds.includes(kind))) {
      const reused = reuseOutputOnlyDiagram(parsed,{options:{kind,output:method,input_output:'cheap'},diagram:graphs[kind]})
      assert.deepEqual(reused.diagram,graphs.output_only, 'Cached independent diagrams project to the output-only graph.')
    }
  }
  const exact = await checkAll(definition,{output:'partial',input_output:'exact'})
  const outputOnly = await check(definition,{kind:'output_only',output:'exact',input_output:'cheap'})
  for (const kind of ['input_output','input_aware'].filter(kind => !kinds.includes(kind))) {
    const reused = reuseOutputOnlyDiagram(parsed,{options:{kind,output:'partial',input_output:'exact'},diagram:exact[kind]})
    assert.equal(reused.options.output,'exact')
    assert.deepEqual(reused.diagram,outputOnly)
  }
  if (kinds.includes('input_output')) assert.equal(edge(exact.input_output,1,1,0),true)
  if (kinds.includes('input_aware')) {
    assert.equal(edge(exact.input_aware,null,1,0),true)
    assert.equal(edge(outputOnly,null,1,0),false)
  }
}
console.log(`Diagram checks passed: ${checks} fixtures on wasm${server ? ' and server' : ''}, one diagram per request, all methods, input promises, SCCs, input-aware permissions, empty/nonregular graphs, and progress.`)
