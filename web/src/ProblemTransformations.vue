<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef } from 'vue'
import { sendForProblem, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { ProblemState } from './generated/ProblemState'
import type { Progress } from './generated/Progress'
import ComputationProgress from './ComputationProgress.vue'

const props = defineProps<{
  state: ProblemState; backend: Backend; disabled: boolean
}>()
const problem = computed(() => props.state.problem)
const emit = defineEmits<{
  created: [problem: Problem]; busy: [running: boolean]
}>()
const running = ref(false)
const progress = shallowRef<Progress | null>(null)
const error = ref('')
const cancelled = ref(false)
const operation = ref<'apply_re' | 'basic_fixed_point'>('apply_re')
let controller: AbortController | null = null
onBeforeUnmount(() => { controller?.abort(); controller = null })

function cancel() {
  controller?.abort()
  controller = null
  running.value = false
  cancelled.value = true
  emit('busy', false)
}

async function apply(action: 'apply_re' | 'basic_fixed_point') {
  if (controller || props.disabled) return
  const current = new AbortController()
  controller = current
  operation.value = action
  running.value = true
  cancelled.value = false
  error.value = ''
  progress.value = null
  emit('busy', true)
  try {
    const response = await sendForProblem(props.state, { type: action }, props.backend, {
      signal: current.signal,
      onProgress: value => { if (controller === current) progress.value = value },
    })
    if (controller !== current) return
    if (response.type === 'problem') emit('created', response.data)
    else if (response.type === 'cancelled') cancelled.value = true
    else throw new Error(response.type === 'error' ? response.data.message : 'Unexpected transformation response.')
  } catch (cause) {
    if (controller !== current) return
    if (current.signal.aborted) cancelled.value = true
    else error.value = cause instanceof Error ? cause.message : String(cause)
  } finally {
    if (controller === current) {
      controller = null
      running.value = false
      emit('busy', false)
    }
  }
}
</script>

<template>
  <div class="problem-transformations" :aria-busy="running" @click.stop>
    <ComputationProgress v-if="running" :progress="progress"
      :label="operation === 'apply_re' ? 'Round elimination in progress' : 'Basic fixed point in progress'"
      :iteration-label="['maximizing', 'closing_active', 'closing_passive'].includes(progress?.phase ?? '') ? 'configuration pairs processed' : progress?.phase === 'transforming' && operation === 'apply_re' ? 'constraint parts processed' : 'items processed'"
      @cancel="cancel" />
    <template v-else>
      <button type="button" :disabled="disabled" @click="apply('apply_re')">Apply RE</button>
      <button v-if="problem.mapping.kind === 'unrestricted'" type="button" :disabled="disabled"
        title="Construct a fixed-point candidate using the default output diagram"
        @click="apply('basic_fixed_point')">Basic FP (temporary)</button>
      <span v-if="cancelled" role="status" class="hint">Cancelled.</span>
      <span v-if="error" role="alert" class="error">{{ error }}</span>
    </template>
  </div>
</template>
