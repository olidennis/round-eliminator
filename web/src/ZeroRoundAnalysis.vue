<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { send, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { Progress } from './generated/Progress'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'
import ZeroRoundView from './ZeroRoundView.vue'
import ComputationProgress from './ComputationProgress.vue'
import CollapsibleSection from './CollapsibleSection.vue'

const props = defineProps<{ problem: Problem; backend: Backend; saved: ZeroRoundResult | null; collapsed?: boolean }>()
const emit = defineEmits<{ update: [result: ZeroRoundResult | null]; collapse: [collapsed: boolean] }>()
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>(props.saved ? 'done' : 'idle')
const progress = ref<Progress | null>(null)
const result = ref<ZeroRoundResult | null>(props.saved)
watch(result, value => emit('update', value), { flush: 'sync' })
const error = ref('')
let controller: AbortController | null = null
onBeforeUnmount(() => {
  controller?.abort()
  controller = null
})

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
    const response = await send({ type: 'check_zero_round', data: { problem: props.problem, passive_mode: 'general' } }, props.backend, {
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
  <CollapsibleSection class="analysis" title="Zero-round solvability" :aria-busy="state === 'running'"
    :collapsed="collapsed" @update:collapsed="emit('collapse', $event)">
    <ComputationProgress v-if="state === 'running'" :progress="progress"
      label="Zero-round computation in progress" @cancel="cancel" />
    <ZeroRoundView v-else-if="state === 'done' && result" :problem="problem" :result="result" />
    <template v-else>
      <p v-if="state === 'cancelled'" role="status">Computation cancelled.</p>
      <p v-if="state === 'error'" class="error" role="alert">{{ error }}</p>
      <button type="button" @click="run">{{ state === 'idle' ? 'Check zero-round solvability' : 'Retry zero-round check' }}</button>
    </template>
  </CollapsibleSection>
</template>
