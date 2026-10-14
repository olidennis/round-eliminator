<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { sendForProblem, type Backend } from './api'
import type { ProblemState } from './generated/ProblemState'
import type { Progress } from './generated/Progress'
import type { ColoringKind } from './generated/ColoringKind'
import type { ColoringBounds } from './generated/ColoringBounds'
import { coloringKey, coloringQuery, type SavedColoring } from './session'
import ComputationProgress from './ComputationProgress.vue'
import ColoringView from './ColoringView.vue'
import CollapsibleSection from './CollapsibleSection.vue'

const props = defineProps<{ state: ProblemState; backend: Backend; saved: SavedColoring; collapsed?: boolean; collapsedTarget?: HTMLElement | null }>()
const problem = computed(() => props.state.problem)
const emit = defineEmits<{ update: [analysis: SavedColoring]; collapse: [collapsed: boolean] }>()
const ordinaryGraph = computed(() => props.state.problem.graph_class.passive.every(degree => degree === 2))
const savedKindSupported = ordinaryGraph.value || !['graph_edges', 'distance_two'].includes(props.saved.kind)
const kind = ref<ColoringKind>(savedKindSupported ? props.saved.kind : 'vertex')
const search = ref<'maximum' | 'fixed'>(props.saved.search)
const colors = ref(props.saved.colors)
const query = computed(() => coloringQuery({ kind: kind.value, search: search.value, colors: colors.value }))
const results = computed(() => props.state.analyses.coloring.filter(entry => ordinaryGraph.value
  || !['graph_edges', 'distance_two'].includes(entry.kind)))
const result = computed(() => results.value.find(entry => coloringKey(entry) === coloringKey(query.value))?.result ?? null)
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>(
  result.value ? 'done' : savedKindSupported && props.saved.bounds ? 'cancelled' : 'idle',
)
const progress = shallowRef<Progress | null>(null)
const bounds = shallowRef<ColoringBounds | null>(savedKindSupported && props.saved.bounds ? { ...props.saved.bounds, testing: null } : null)
const error = ref('')
let controller: AbortController | null = null
onBeforeUnmount(() => { controller?.abort(); controller = null })

const minimum = computed(() => kind.value === 'distance_two'
  ? Math.max(0, ...props.state.problem.graph_class.active) + 1
  : Math.max(1, ...(
  kind.value === 'vertex' ? props.state.problem.graph_class.passive :
  kind.value === 'graph_edges' ? props.state.problem.graph_class.active :
  [...props.state.problem.graph_class.active, ...props.state.problem.graph_class.passive]
)))
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
    state.value = result.value ? 'done' : 'idle'
    bounds.value = null
    error.value = ''
  }
})

// Only stable settings and mathematical results belong in a saved session.
// Progress, workers, controllers and running state remain local to this card.
watch([kind, search, colors, bounds, results], () => emit('update', {
  kind: kind.value, search: search.value, colors: colors.value,
  bounds: bounds.value,
}), { immediate: true })

function cancel() {
  controller?.abort()
  controller = null
  state.value = 'cancelled'
}

async function run() {
  if (state.value === 'running' || result.value) return
  const requested = query.value
  const current = new AbortController()
  controller = current
  state.value = 'running'
  progress.value = null
  bounds.value = null
  error.value = ''
  try {
    const response = await sendForProblem(props.state, { type: 'check_coloring', data: {
      ...requested, passive_mode: 'general',
    } }, props.backend, {
      signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
      onPartialResult: update => {
        if (controller === current && update.type === 'coloring_bounds') bounds.value = update.data
      },
    })
    if (controller !== current) return
    if (response.type === 'coloring') {
      bounds.value = null
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
  <CollapsibleSection class="analysis coloring-analysis" title="Coloring solvability" :aria-busy="state === 'running'"
    :collapsed="collapsed" :collapsed-target="collapsedTarget" @update:collapsed="emit('collapse', $event)">
    <fieldset class="form-fields analysis-options" :disabled="state === 'running'">
      <label>Given coloring
        <select v-model="kind">
          <option value="vertex">Vertex coloring (strong on hypergraphs)</option>
          <option v-if="ordinaryGraph" value="distance_two">Distance-two vertex coloring</option>
          <option v-if="ordinaryGraph" value="graph_edges">Graph edge coloring</option>
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
      <button v-if="state !== 'running' && state !== 'done'" type="button" @click="run">
        {{ state === 'idle' ? 'Check coloring solvability' : 'Retry coloring check' }}
      </button>
    </fieldset>
    <details class="help-details analysis-help">
    <summary>Coloring assumptions</summary>
    <p v-if="kind === 'distance_two'" class="hint">Each node knows its own color, all neighboring
      colors, and all original local inputs. Colors in each closed neighborhood are distinct.
      This check requires graphs with passive degree 2.
      At least {{ minimum }} colors are required locally.</p>
    <p v-else class="hint">The strategy can use the given colors and all original inputs.
      Vertex colors differ around each passive node; graph edge colors agree across
      a degree-two passive edge and differ at active nodes; incidence edge colors
      differ at both sides. At least {{ minimum }} colors are required locally.</p>
    </details>
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
  </CollapsibleSection>
</template>
