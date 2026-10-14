<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { send, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { Progress } from './generated/Progress'
import type { MaximizationResult } from './generated/MaximizationResult'
import CollapsibleSection from './CollapsibleSection.vue'
import ComputationProgress from './ComputationProgress.vue'

const props = defineProps<{
  problem: Problem; backend: Backend; saved: MaximizationResult | null
  collapsed?: boolean; collapsedTarget?: HTMLElement | null
  disabled?: boolean
}>()
const emit = defineEmits<{ update: [result: MaximizationResult]; collapse: [collapsed: boolean]; busy: [running: boolean] }>()
const result = shallowRef(props.saved)
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>(result.value ? 'done' : 'idle')
const progress = shallowRef<Progress | null>(null)
const error = ref('')
const count = computed(() => result.value?.constraint.degrees.reduce((sum, group) => sum + group.configurations.length, 0) ?? 0)
let controller: AbortController | null = null
watch(result, value => { if (value) emit('update', value) })
// Apply RE can complete the source maximum through the same saved snapshot.
watch(() => props.saved, value => { if (value) { result.value = value; state.value = 'done' } })
watch(() => state.value === 'running', value => emit('busy', value), { flush: 'sync' })
onBeforeUnmount(() => { controller?.abort(); controller = null })

function cancel() {
  controller?.abort()
  controller = null
  state.value = 'cancelled'
}

async function run() {
  if (controller || result.value || props.disabled) return
  const current = new AbortController()
  controller = current
  state.value = 'running'
  progress.value = null
  error.value = ''
  try {
    const response = await send({ type: 'maximize', data: { problem: props.problem } }, props.backend, {
      signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
    })
    if (controller !== current) return
    if (response.type === 'maximization') { result.value = response.data; state.value = 'done' }
    else if (response.type === 'cancelled') state.value = 'cancelled'
    else throw new Error(response.type === 'error' ? response.data.message : 'Unexpected maximization response.')
  } catch (cause) {
    if (controller !== current) return
    if (current.signal.aborted) state.value = 'cancelled'
    else { error.value = cause instanceof Error ? cause.message : String(cause); state.value = 'error' }
  } finally { if (controller === current) controller = null }
}
</script>

<template>
  <CollapsibleSection class="analysis maximization-analysis" title="Maximization" :aria-busy="state === 'running'"
    :collapsed="collapsed" :collapsed-target="collapsedTarget" @update:collapsed="emit('collapse', $event)">
    <ComputationProgress v-if="state === 'running'" :progress="progress" label="Maximization in progress"
      iteration-label="configuration pairs processed" @cancel="cancel" />
    <div v-else aria-live="polite">
      <p v-if="result">{{ result.kind === 'input_output' ? 'Input-aware passive maximum ready.' : 'Passive maximum ready.' }}
        <small>{{ count }} maximal {{ count === 1 ? 'configuration' : 'configurations' }}.</small></p>
      <template v-else>
        <p v-if="state === 'cancelled'" role="status">Computation cancelled.</p>
        <p v-if="state === 'error'" class="error" role="alert">{{ error }}</p>
        <button type="button" :disabled="disabled" @click="run">{{ state === 'idle' ? 'Maximize' : 'Retry maximization' }}</button>
      </template>
    </div>
    <details class="help-details analysis-help">
      <summary>About maximization</summary>
      <p class="hint">Compute and retain the passive maximum for this problem. With input-dependent output
        validity, maximize valid input-output pairs while keeping input configurations condensed.
        The displayed definition stays unchanged; saved sessions retain the complete result.</p>
    </details>
  </CollapsibleSection>
</template>
