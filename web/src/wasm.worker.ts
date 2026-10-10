import type { Response } from './generated/Response'
import type { Progress } from './generated/Progress'
import type { ComputationEvent } from './generated/ComputationEvent'

type WasmModule = {
  default: () => Promise<unknown>
  execute_with_progress: (request: string, onProgress: (json: string) => void) => string
}

function post(event: ComputationEvent) { self.postMessage(event) }

self.onmessage = async (event: MessageEvent<string>) => {
  try {
    const moduleUrl = '/wasm/round_eliminator_3_wasm.js'
    const wasm = (await import(/* @vite-ignore */ moduleUrl)) as WasmModule
    await wasm.default()
    const response = wasm.execute_with_progress(event.data, json => {
      post({ type: 'progress', data: JSON.parse(json) as Progress })
    })
    post({ type: 'response', data: JSON.parse(response) as Response })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    post({ type: 'response', data: { type: 'error', data: { message, location: null } } })
  }
}
