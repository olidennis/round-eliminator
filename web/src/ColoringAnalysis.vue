<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { send, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { Progress } from './generated/Progress'
import type { ColoringKind } from './generated/ColoringKind'
import type { ColoringBounds } from './generated/ColoringBounds'
import type { ColoringResult } from './generated/ColoringResult'
import ComputationProgress from './ComputationProgress.vue'
import ColoringView from './ColoringView.vue'

const props = defineProps<{ problem: Problem; backend: Backend }>()
const kind = ref<ColoringKind>('vertex')
const search = ref<'maximum' | 'fixed'>('maximum')
const colors = ref(3)
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>('idle')
const progress = shallowRef<Progress | null>(null)
const bounds = shallowRef<ColoringBounds | null>(null)
const result = shallowRef<ColoringResult | null>(null)
const error = ref('')
let controller: AbortController | null = null
onBeforeUnmount(() => { controller?.abort(); controller = null })

const minimum = computed(() => kind.value === 'distance_two'
  ? Math.max(0, ...props.problem.graph_class.active) + 1
  : Math.max(1, ...(
  kind.value === 'vertex' ? props.problem.graph_class.passive :
  kind.value === 'graph_edges' ? props.problem.graph_class.active :
  [...props.problem.graph_class.active, ...props.problem.graph_class.passive]
)))
const ordinaryGraph = computed(() => props.problem.graph_class.passive.every(degree => degree === 2))
watch(kind, value => {
  if (value === 'distance_two') {
    colors.value = Math.max(colors.value, minimum.value)
  }
})
const witness = computed(() => {
  const r = result.value
  return r && 'witness' in r ? r.witness : bounds.value?.witness
})

// Bounds certify one particular query; do not display them for changed settings.
watch([kind, search, colors], () => {
  if (state.value !== 'running') {
    state.value = 'idle'
    result.value = null
    bounds.value = null
    error.value = ''
  }
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
  bounds.value = null
  result.value = null
  error.value = ''
  try {
    const response = await send({ type: 'check_coloring', data: {
      problem: props.problem, kind: kind.value, passive_mode: 'general',
      search: search.value === 'fixed' ? { kind: 'fixed', colors: colors.value } : { kind: 'maximum' },
    } }, props.backend, {
      signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
      onPartialResult: update => {
        if (controller === current && update.type === 'coloring_bounds') bounds.value = update.data
      },
    })
    if (controller !== current) return
    if (response.type === 'coloring') {
      result.value = response.data
      state.value = 'done'
    } else if (response.type === 'cancelled') {
      state.value = 'cancelled'
    } else {
      throw new Error(response.type === 'error' ? response.data.message : 'Unexpected coloring response.')
    }
  } catch (cause) {
    if (controller !== current) return
    if (current.signal.aborted) state.value = 'cancelled'
    else {
      error.value = cause instanceof Error ? cause.message : String(cause)
      state.value = 'error'
    }
  } finally {
    if (controller === current) controller = null
  }
}
</script>

<template>
  <section class="analysis" aria-label="Coloring solvability" :aria-busy="state === 'running'">
    <h3>Coloring solvability</h3>
    <fieldset class="form-fields analysis-options" :disabled="state === 'running'">
      <label>Given coloring
        <select v-model="kind">
          <option value="vertex">Vertex coloring (strong on hypergraphs)</option>
          <option value="distance_two" :disabled="!ordinaryGraph">Distance-two vertex coloring</option>
          <option value="graph_edges">Graph edge coloring</option>
          <option value="incidence_edges">Incidence edge coloring</option>
        </select>
      </label>
      <label>Palette search
        <select v-model="search">
          <option value="maximum">Find the maximum palette</option>
          <option value="fixed">Check a given palette</option>
        </select>
      </label>
      <label v-if="search === 'fixed'">Number of colors
        <input v-model.number="colors" type="number" :min="minimum" step="1" />
      </label>
    </fieldset>
    <p v-if="kind === 'distance_two'" class="hint">Each node knows its own color, all neighboring
      colors, and all original local inputs. Colors in each closed neighborhood are distinct.
      This check requires graphs with passive degree 2.
      At least {{ minimum }} colors are required locally.</p>
    <p v-else class="hint">The strategy can use the given colors and all original inputs.
      Vertex colors differ around each passive node; graph edge colors agree across
      a degree-two passive edge and differ at active nodes; incidence edge colors
      differ at both sides. At least {{ minimum }} colors are required locally.</p>
    <ComputationProgress v-if="state === 'running'" :progress="progress"
      label="Coloring computation in progress" @cancel="cancel" />
    <div aria-live="polite" aria-label="Coloring result">
      <template v-if="result">
        <p v-if="result.status === 'maximum'">Maximum solvable palette: {{ result.colors }} colors.</p>
        <p v-else-if="result.status === 'unbounded'">Every palette with at least {{ result.minimum }} colors is solvable.</p>
        <p v-else-if="result.status === 'no_palette'">No locally feasible palette is solvable.</p>
        <p v-else-if="result.status === 'solvable'">Solvable with {{ result.colors }} colors.</p>
        <p v-else>Unsolvable with {{ result.colors }} colors.</p>
      </template>
      <template v-else-if="bounds">
        <p v-if="state === 'running' && bounds.testing !== null">Checking {{ bounds.testing }} colors…</p>
        <p v-if="bounds.lower !== null">Proved solvable with {{ bounds.lower }} colors.</p>
        <p v-if="bounds.upper !== null">Proved upper bound: {{ bounds.upper }} colors.</p>
      </template>
      <p v-if="state === 'cancelled'" role="status">Computation cancelled. Proved bounds are retained.</p>
      <p v-if="state === 'error'" class="error" role="alert">{{ error }}</p>
    </div>
    <ColoringView v-if="witness" :problem="problem" :witness="witness" :repeating="result?.status === 'unbounded'" />
    <button v-if="state !== 'running' && state !== 'done'" type="button" class="analysis-button" @click="run">
      {{ state === 'idle' ? 'Check coloring solvability' : 'Retry coloring check' }}
    </button>
  </section>
</template>
