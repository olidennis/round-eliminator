<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { send, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { Progress } from './generated/Progress'
import type { Phase } from './generated/Phase'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'
import ZeroRoundView from './ZeroRoundView.vue'

const props = defineProps<{ problem: Problem; backend: Backend }>()
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>('idle')
const progress = ref<Progress | null>(null)
const result = ref<ZeroRoundResult | null>(null)
const error = ref('')
let controller: AbortController | null = null
onBeforeUnmount(() => {
  controller?.abort()
  controller = null
})

const phases: Record<Phase, string> = {
  queued: 'Waiting to start…', parsing: 'Reading the problem…',
  enumerating: 'Enumerating input configurations…', encoding: 'Building the SAT constraints…',
  solving: 'Searching for a strategy…', verifying: 'Checking passive configurations…',
}

function cancel() {
  controller?.abort()
  controller = null
  state.value = 'cancelled'
}

async function run() {
  if (state.value === 'running') return
  const current = new AbortController()
  controller = current
  state.value = 'running'
  progress.value = null
  error.value = ''
  try {
    const response = await send({ type: 'check_zero_round', data: props.problem }, props.backend, {
      signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
    })
    // Cancellation/retry/unmount may have superseded this request.
    if (controller !== current) return
    if (response.type === 'zero_round') {
      result.value = response.data
      state.value = 'done'
    } else if (response.type === 'cancelled') {
      state.value = 'cancelled'
    } else {
      throw new Error(response.type === 'error' ? response.data.message : 'Unexpected analysis response.')
    }
  } catch (cause) {
    if (controller !== current) return
    if (current.signal.aborted) {
      state.value = 'cancelled'
    } else {
      error.value = cause instanceof Error ? cause.message : String(cause)
      state.value = 'error'
    }
  } finally {
    if (controller === current) controller = null
  }
}
</script>

<template>
  <section class="analysis" aria-label="Zero-round solvability" :aria-busy="state === 'running'">
    <h3>Zero-round solvability</h3>
    <div v-if="state === 'running'" class="computation-progress">
      <progress aria-label="Zero-round computation in progress" />
      <div role="status">
        {{ progress ? phases[progress.phase] : 'Starting…' }}
        <small v-if="progress && progress.iterations > 0">{{ progress.iterations }} candidate
          {{ progress.iterations === 1 ? 'strategy examined' : 'strategies examined' }}.</small>
      </div>
      <button type="button" @click="cancel">Cancel</button>
    </div>
    <ZeroRoundView v-else-if="state === 'done' && result" :problem="problem" :result="result" />
    <template v-else>
      <p v-if="state === 'cancelled'" role="status">Computation cancelled.</p>
      <p v-if="state === 'error'" class="error" role="alert">{{ error }}</p>
      <button type="button" @click="run">{{ state === 'idle' ? 'Check zero-round solvability' : 'Retry zero-round check' }}</button>
    </template>
  </section>
</template>
