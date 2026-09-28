import type { Request } from './generated/Request'
import type { Response } from './generated/Response'

export type Backend = 'server' | 'wasm'

/** The UI sends one Rust-defined request through either transport. */
export async function send(request: Request, backend: Backend): Promise<Response> {
  const json = JSON.stringify(request)
  if (backend === 'server') {
    const response = await fetch('/api', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: json,
    })
    if (!response.ok) throw new Error(`Server request failed (${response.status}).`)
    return response.json() as Promise<Response>
  }

  return new Promise<Response>((resolve, reject) => {
    const worker = new Worker(new URL('./wasm.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = (event: MessageEvent<Response>) => {
      worker.terminate()
      resolve(event.data)
    }
    worker.onerror = (event) => {
      worker.terminate()
      reject(new Error(event.message || 'WebAssembly worker failed.'))
    }
    worker.postMessage(json)
  })
}
