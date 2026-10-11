import type { Response } from './generated/Response'
import type { ComputationEvent } from './generated/ComputationEvent'

type WasmModule = {
  default: () => Promise<unknown>
  execute_with_events: (request: string, onEvent: (json: string) => void) => string
}

function post(event: ComputationEvent) { self.postMessage(event) }

self.onmessage = async (event: MessageEvent<string>) => {
  try {
    const moduleUrl = '/wasm/round_eliminator_3_wasm.js'
    const wasm = (await import(/* @vite-ignore */ moduleUrl)) as WasmModule
    await wasm.default()
    const response = wasm.execute_with_events(event.data, json => {
      post(JSON.parse(json) as ComputationEvent)
    })
    post({ type: 'response', data: JSON.parse(response) as Response })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    post({ type: 'response', data: { type: 'error', data: { message, location: null } } })
  }
}
