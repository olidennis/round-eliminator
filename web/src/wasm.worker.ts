import type { Response } from './generated/Response'

type WasmModule = {
  default: () => Promise<unknown>
  execute_json: (request: string) => string
}

self.onmessage = async (event: MessageEvent<string>) => {
  try {
    // wasm-pack writes this ES module into Vite's public directory.
    const moduleUrl = '/wasm/round_eliminator_3_wasm.js'
    const wasm = (await import(/* @vite-ignore */ moduleUrl)) as WasmModule
    await wasm.default()
    self.postMessage(JSON.parse(wasm.execute_json(event.data)) as Response)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    const response: Response = { type: 'error', data: { message, location: null } }
    self.postMessage(response)
  }
}
