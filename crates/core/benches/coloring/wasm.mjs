import { encode, decode } from '../../../../web/scripts/protocol.mjs'
// Production wasm, isolated in a worker so the deadline can interrupt SAT.
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

if (isMainThread) {
  const [problemFile, palette = 'maximum', seconds = '30', mode = 'general', modulePath] = process.argv.slice(2)
  const kind = process.env.RE3_COLORING_KIND ?? 'incidence_edges'
  const worker = new Worker(new URL(import.meta.url), { workerData: {
    problemFile, palette, mode, modulePath: modulePath ?? process.env.RE3_WASM_MODULE, kind, caseName: process.env.RE3_CASE,
  } })
  let timer
  let latest
  worker.on('message', message => {
    if (message.event === 'started') {
      timer = setTimeout(async () => {
        await worker.terminate()
        console.log(JSON.stringify({ event: 'result', error: 'deadline', seconds: Number(seconds), latest }))
      }, Number(seconds) * 1000)
    }
    latest = message
    console.log(JSON.stringify(message))
  })
  worker.on('exit', () => clearTimeout(timer))
  worker.on('error', error => { console.error(error); process.exitCode = 1 })
} else {
  const moduleURL = workerData.modulePath ? pathToFileURL(workerData.modulePath) :
    new URL('../../../../web/public/wasm/round_eliminator_3_wasm.js', import.meta.url)
  const { default: init, execute_json, execute_with_events } = await import(moduleURL)
  await init({ module_or_path: await readFile(new URL('./round_eliminator_3_wasm_bg.wasm', moduleURL)) })
  let problem = workerData.problemFile === 'bmm' ? decode(execute_json(encode({
    type: 'parse_problem', data: { kind: 'plain', constraints: { active: 'M U^9\nP^10', passive: 'M UP^9\nU^10' } },
  }))).data : JSON.parse(await readFile(workerData.problemFile, 'utf8'))
  if (workerData.caseName) {
    const fixture = JSON.parse(await readFile(new URL('./cases.json', import.meta.url), 'utf8'))[workerData.caseName]
    problem = decode(execute_json(encode({ type: 'parse_problem', data: fixture.definition }))).data
    for (let step = 0; step < (fixture.steps ?? 0); step++) {
      problem = decode(execute_json(encode({ type: 'apply_re', data: { problem } }))).data
    }
    workerData.kind = fixture.kind
    workerData.palette = String(fixture.palette)
  }
  const search = workerData.palette === 'maximum' ? { kind: 'maximum' } :
    { kind: 'fixed', colors: Number(workerData.palette) }
  const request = encode({ type: 'check_coloring', data: {
    problem, kind: workerData.kind, search, passive_mode: workerData.mode,
  } })
  const started = performance.now()
  let printed = started
  let latestProgress
  parentPort.postMessage({ event: 'started' })
  const response = decode(execute_with_events(request, json => {
    const event = JSON.parse(json)
    if (event.type === 'progress') latestProgress = event.data
    const now = performance.now()
    if (event.type !== 'progress' || now - printed >= 1000) {
      parentPort.postMessage({ event: event.type, seconds: (now - started) / 1000, data: event.data })
      printed = now
    }
  }))
  parentPort.postMessage({ event: 'result', seconds: (performance.now() - started) / 1000, latestProgress, response })
}
