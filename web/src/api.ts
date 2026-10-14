import type { Operation } from './generated/Operation'
import type { ProblemState } from './generated/ProblemState'
import type { AnalysisCache } from './generated/AnalysisCache'
import type { ResponsePacket } from './generated/ResponsePacket'
import { diagramKey } from './diagram'
import type { Request } from './generated/Request'
import type { Response } from './generated/Response'
import type { ComputationEvent } from './generated/ComputationEvent'
import type { Progress } from './generated/Progress'

export type Backend = 'server' | 'wasm'

export function defaultBackend(url: URL): Backend {
  const requested = url.searchParams.get('backend')
  if (requested === 'server' || requested === 'wasm') return requested
  return url.hostname === 'localhost' || url.hostname === '127.0.0.1' ? 'server' : 'wasm'
}
type Options = { onCache?: (cache: AnalysisCache) => void; signal?: AbortSignal; onProgress?: (progress: Progress) => void; onPartialResult?: (response: Response) => void }

/** Components request an operation; this adapter owns snapshot/cache transport.
 * Merge against the live cache so concurrent analyses retain each other's results.
 */
export function sendForProblem(state: ProblemState, operation: Operation, backend: Backend, options: Options = {}): Promise<Response> {
  return send({ type: 'compute', data: { state, operation } }, backend, {
    ...options,
    onPartialResult: update => {
      if (operation.type === 'check_coloring' && update.type === 'coloring_bounds'
        && (update.data.lower !== null || update.data.upper !== null)) {
        state.analyses = mergeAnalyses(state.analyses, { maximization: null, zero_round: null,
          coloring: [], diagrams: [], coloring_bounds: [{ kind: operation.data.kind,
            bounds: { ...update.data, testing: null } }] })
      }
      options.onPartialResult?.(update)
    },
    onCache: update => { state.analyses = mergeAnalyses(state.analyses, update); options.onCache?.(update) },
  })
}

export function mergeAnalyses(current: AnalysisCache, update: AnalysisCache): AnalysisCache {
  const merge = <T>(old: T[], added: T[], key: (value: T) => string): T[] =>
    Array.from(new Map([...old, ...added].map(value => [key(value), value])).values())
  const coloring = merge(current.coloring, update.coloring, value => value.search.kind === 'fixed'
    ? `${value.kind}:fixed:${value.search.colors}` : `${value.kind}:maximum`)
  const bounds = new Map((current.coloring_bounds ?? []).map(entry => [entry.kind, entry]))
  for (const entry of update.coloring_bounds ?? []) {
    const previous = bounds.get(entry.kind)
    const lower = previous?.bounds.lower ?? null
    const upper = previous?.bounds.upper ?? null
    const stronger = entry.bounds.lower !== null && (lower === null || entry.bounds.lower > lower)
    bounds.set(entry.kind, { kind: entry.kind, bounds: { ...entry.bounds, testing: null,
      lower: stronger ? entry.bounds.lower : lower,
      witness: stronger ? entry.bounds.witness : previous?.bounds.witness ?? null,
      upper: entry.bounds.upper === null ? upper : upper === null ? entry.bounds.upper : Math.min(upper, entry.bounds.upper),
    } })
  }
  return {
    maximization: update.maximization ?? current.maximization,
    zero_round: update.zero_round ?? current.zero_round,
    coloring,
    coloring_bounds: [...bounds.values()].filter(entry => !coloring.some(proof => proof.kind === entry.kind
      && ['maximum', 'unbounded', 'no_palette'].includes(proof.result.status))),
    diagrams: merge(current.diagrams, update.diagrams, value => diagramKey(value.options)),
  }
}

function completed(packet: ResponsePacket, options: Options): Response {
  options.onCache?.(packet.cache)
  return packet.response
}

/** The same Rust-defined events arrive through HTTP streaming or a worker. */
export async function send(request: Request, backend: Backend, options: Options = {}): Promise<Response> {
  options.signal?.throwIfAborted()
  const json = JSON.stringify(request)
  if (backend === 'server') return sendToServer(json, options)

  return new Promise<Response>((resolve, reject) => {
    const worker = new Worker(new URL('./wasm.worker.ts', import.meta.url), { type: 'module' })
    const cleanup = () => {
      options.signal?.removeEventListener('abort', abort)
      worker.terminate()
    }
    const abort = () => {
      cleanup()
      reject(new DOMException('Computation cancelled.', 'AbortError'))
    }
    options.signal?.addEventListener('abort', abort, { once: true })
    worker.onmessage = (event: MessageEvent<ComputationEvent>) => {
      if (event.data.type === 'progress') {
        options.onProgress?.(event.data.data)
      } else if (event.data.type === 'cache_update') {
        options.onCache?.(event.data.data)
      } else if (event.data.type === 'partial_result') {
        options.onPartialResult?.(event.data.data)
      } else {
        cleanup()
        resolve(completed(event.data.data, options))
      }
    }
    worker.onerror = (event) => {
      cleanup()
      reject(new Error(event.message || 'WebAssembly worker failed.'))
    }
    worker.postMessage(json)
  })
}

async function sendToServer(json: string, options: Options): Promise<Response> {
  const response = await fetch('/api/stream', {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'text/event-stream' },
    body: json,
    signal: options.signal,
  })
  if (!response.ok) throw new Error(`Server request failed (${response.status}).`)
  if (!response.body) throw new Error('Server returned no computation stream.')
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  try {
    while (true) {
      const { value, done } = await reader.read()
      if (done) throw new Error('Server stream ended before a result was received.')
      buffer += decoder.decode(value, { stream: true })
      // Axum emits LF-delimited SSE frames. Network chunks may split anywhere,
      // including inside UTF-8 characters; the decoder and buffer retain them.
      let boundary: number
      while ((boundary = buffer.indexOf('\n\n')) !== -1) {
        const frame = buffer.slice(0, boundary)
        buffer = buffer.slice(boundary + 2)
        const data = frame.split('\n').filter(line => line.startsWith('data:'))
          .map(line => line.slice(5).trimStart()).join('\n')
        if (!data) continue // SSE keep-alive comments carry no protocol event.
        options.signal?.throwIfAborted()
        const event = JSON.parse(data) as ComputationEvent
        if (event.type === 'response') return completed(event.data, options)
        if (event.type === 'cache_update') { options.onCache?.(event.data); continue }
        if (event.type === 'partial_result') options.onPartialResult?.(event.data)
        else options.onProgress?.(event.data)
      }
    }
  } finally {
    // Closing the body drops the server stream and cancels its CPU work.
    await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}
