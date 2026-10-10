import type { Request } from './generated/Request'
import type { Response } from './generated/Response'
import type { ComputationEvent } from './generated/ComputationEvent'
import type { Progress } from './generated/Progress'

export type Backend = 'server' | 'wasm'
type Options = { signal?: AbortSignal; onProgress?: (progress: Progress) => void }

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
      } else {
        cleanup()
        resolve(event.data.data)
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
        if (event.type === 'response') return event.data
        options.onProgress?.(event.data)
      }
    }
  } finally {
    // Closing the body drops the server stream and cancels its CPU work.
    await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}
