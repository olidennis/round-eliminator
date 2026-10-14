<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { sendForProblem, type Backend } from './api'
import type { ProblemState } from './generated/ProblemState'
import type { Progress } from './generated/Progress'
import type { InputOutputLabel } from './generated/InputOutputLabel'
import CollapsibleSection from './CollapsibleSection.vue'
import ComputationProgress from './ComputationProgress.vue'
import ConstraintTable from './ConstraintTable.vue'

const props = defineProps<{
  state: ProblemState; backend: Backend
  collapsed?: boolean; collapsedTarget?: HTMLElement | null
  disabled?: boolean
}>()
const problem = computed(() => props.state.problem)
const emit = defineEmits<{ collapse: [collapsed: boolean]; busy: [running: boolean] }>()
const result = computed(() => props.state.analyses.maximization)
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>(result.value ? 'done' : 'idle')
const progress = shallowRef<Progress | null>(null)
const error = ref('')
const count = computed(() => result.value?.constraint.degrees.reduce((sum, group) => sum + group.configurations.length, 0) ?? 0)
const originalCount = computed(() => problem.value.output.passive.degrees.reduce((sum, group) => sum + group.configurations.length, 0))
const defaultCollapsed = computed(() => !!result.value && count.value * 2 > originalCount.value * 3)

function outputLabel(id: number): string {
  const name = problem.value.output.labels[id]
  return [...name].length === 1 && !'()^*'.includes(name) ? name : `(${name})`
}
function pairLabel(pair: InputOutputLabel): string {
  const input = pair.input === null ? 'Trivial input' : problem.value.input.kind === 'explicit'
    ? problem.value.input.data.labels[pair.input] : 'Trivial input'
  const quote = (name: string) => /[,()\s]/u.test(name) ? JSON.stringify(name) : name
  return `(${quote(input)},${quote(problem.value.output.labels[pair.output])})`
}
let controller: AbortController | null = null

// Apply RE can complete the source maximum through the same saved snapshot.
watch(result, value => { if (value) { state.value = 'done' } })
// Persist the automatic choice so section grouping and session exports agree.
// Explicit saved choices, including collapsed=false, take precedence.
watch(result, value => {
  if (value && props.collapsed === undefined) emit('collapse', defaultCollapsed.value)
}, { immediate: true })
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
    const response = await sendForProblem(props.state, { type: 'maximize' }, props.backend, {
      signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
    })
    if (controller !== current) return
    if (response.type === 'maximization') { state.value = 'done' }
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
    :collapsed="collapsed ?? defaultCollapsed" :collapsed-target="collapsedTarget" @update:collapsed="emit('collapse', $event)">
    <ComputationProgress v-if="state === 'running'" :progress="progress" label="Maximization in progress"
      iteration-label="configuration pairs processed" @cancel="cancel" />
    <div v-else aria-live="polite">
      <p v-if="result">{{ result.kind === 'input_output' ? 'Input-aware passive maximum ready.' : 'Passive maximum ready.' }}
        <small>{{ count }} maximal {{ count === 1 ? 'configuration' : 'configurations' }}.</small></p>
      <ConstraintTable v-if="result?.kind === 'output_only'" :constraint="result.constraint"
        :degrees="problem.graph_class.passive" :format-label="outputLabel" />
      <ConstraintTable v-else-if="result?.kind === 'input_output'" :constraint="result.constraint"
        :degrees="problem.graph_class.passive" :format-label="pairLabel" />
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
        RE, applicable diagram computations and solvability checks reuse the saved maximum automatically.
        The displayed definition stays unchanged; saved sessions retain the complete result.</p>
    </details>
  </CollapsibleSection>
</template>
