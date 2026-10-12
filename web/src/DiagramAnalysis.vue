<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { send, type Backend } from './api'
import type { Problem } from './generated/Problem'
import type { Progress } from './generated/Progress'
import type { DiagramResult } from './generated/DiagramResult'
import type { InputOutputDiagramMethod } from './generated/InputOutputDiagramMethod'
import type { OutputDiagramMethod } from './generated/OutputDiagramMethod'
import type { SavedDiagram } from './session'
import { diagramKey, diagramKinds, reuseOutputOnlyDiagram } from './diagram'
import ComputationProgress from './ComputationProgress.vue'
import CollapsibleSection from './CollapsibleSection.vue'
// Load the visualization library only when a diagram is actually displayed.
const DiagramView = defineAsyncComponent(() => import('./DiagramView.vue'))

const props = defineProps<{ problem: Problem; backend: Backend; saved: SavedDiagram; collapsed?: boolean; collapsedTarget?: HTMLElement | null }>()
const emit = defineEmits<{ update: [analysis: SavedDiagram]; collapse: [collapsed: boolean] }>()
const kinds = computed(() => diagramKinds(props.problem))
const inputDiagrams = computed(() => kinds.value.length > 1)
function applicableOptions(options: DiagramResult['options']): DiagramResult['options'] {
  if (kinds.value.includes(options.kind)) return options
  return { ...options, kind: 'output_only', output: options.input_output === 'exact' ? 'exact' : options.output }
}
const initial = applicableOptions(props.saved.options)
const output = ref<OutputDiagramMethod>(initial.output)
const inputOutput = ref<InputOutputDiagramMethod>(props.saved.options.input_output)
const kind = ref(initial.kind)
const layout = ref(props.saved.layout)
// Imported equivalent diagrams remain usable after removing redundant choices.
const results = shallowRef<DiagramResult[]>(Array.from(new Map(props.saved.results.flatMap(candidate => {
  const reusable = reuseOutputOnlyDiagram(props.problem, candidate)
  return reusable ? [[diagramKey(reusable.options), reusable] as const] : []
})).values()))
const options = computed(() => ({ kind: kind.value, output: output.value, input_output: inputOutput.value }))
const result = computed(() => results.value.find(candidate => diagramKey(candidate.options) === diagramKey(options.value)))
const progress = shallowRef<Progress | null>(null)
const state = ref<'idle' | 'running' | 'cancelled' | 'done' | 'error'>(result.value ? 'done' : 'idle')
const error = ref('')
let controller: AbortController | null = null
onBeforeUnmount(() => { controller?.abort(); controller = null })

watch([kind, output, inputOutput], () => {
  state.value = result.value ? 'done' : 'idle'
  error.value = ''
})
watch([options, layout, results], () => emit('update', {
  options: options.value,
  layout: layout.value, results: results.value,
}), { immediate: true })

function cancel() {
  controller?.abort()
  controller = null
  state.value = 'cancelled'
}

async function run() {
  if (state.value === 'running' || result.value) return
  const current = new AbortController()
  controller = current
  state.value = 'running'
  progress.value = null
  error.value = ''
  try {
    const response = await send({ type: 'compute_diagram', data: {
      problem: props.problem, options: options.value,
    } }, props.backend, { signal: current.signal,
      onProgress: update => { if (controller === current) progress.value = update },
    })
    if (controller !== current) return
    if (response.type === 'diagram') { results.value = [...results.value, response.data]; state.value = 'done' }
    else if (response.type === 'cancelled') state.value = 'cancelled'
    else throw new Error(response.type === 'error' ? response.data.message : 'Unexpected diagram response.')
  } catch (cause) {
    if (controller !== current) return
    if (current.signal.aborted) state.value = 'cancelled'
    else { error.value = cause instanceof Error ? cause.message : String(cause); state.value = 'error' }
  } finally { if (controller === current) controller = null }
}
</script>

<template>
  <CollapsibleSection class="analysis diagram-analysis" :class="{ 'has-diagram': result }"
    title="Diagrams" :aria-busy="state === 'running'"
    :collapsed="collapsed" :collapsed-target="collapsedTarget" @update:collapsed="emit('collapse', $event)">
    <fieldset class="form-fields analysis-options" :disabled="state === 'running'">
      <label v-if="inputDiagrams">Diagram type
        <select v-model="kind" aria-label="Diagram type">
          <option value="output_only">Output-only</option>
          <option v-if="kinds.includes('input_output')" value="input_output">Input-output pairs</option>
          <option v-if="kinds.includes('input_aware')" value="input_aware">Input-aware outputs</option>
        </select>
      </label>
      <label v-if="kind !== 'output_only'">Input-output diagram method
        <select v-model="inputOutput" aria-label="Input-output diagram method">
          <option value="cheap">Cheap, using output arrows</option>
          <option value="exact">Exact, using input promises</option>
        </select>
      </label>
      <label v-if="kind === 'output_only' || inputOutput === 'cheap'">
        {{ kind === 'output_only' ? 'Output-only diagram method' : 'Underlying output-arrow method' }}
        <select v-model="output" :aria-label="kind === 'output_only' ? 'Output-only diagram method' : 'Underlying output-arrow method'">
          <option value="exact">Exact</option>
          <option value="partial">Fast, partial</option>
          <option value="maximized">Fast, assuming maximized</option>
        </select>
      </label>
      <label v-if="result">Layout
        <select v-model="layout" aria-label="Layout">
          <option value="physics">Physics</option>
          <option value="hierarchical">Hierarchical</option>
          <option value="free">Free</option>
        </select>
      </label>
      <button v-if="state !== 'running' && state !== 'done'" type="button" @click="run">
        {{ state === 'idle' ? 'Compute diagram' : 'Retry diagram computation' }}
      </button>
    </fieldset>
    <p v-if="output === 'maximized' && (kind === 'output_only' || inputOutput === 'cheap')" class="hint">Use this method only when the complete passive
      output constraint is already maximized. This computation does not maximize it.</p>
    <details class="help-details analysis-help">
    <summary>Diagram assumptions</summary>
    <p class="hint">Arrows describe replacements preserving passive validity.
      <template v-if="kinds.includes('input_output')">For input-output pairs, arrows keep the input fixed.</template>
      <template v-if="kinds.includes('input_aware')">For input-aware outputs, they hold for every input permitting the source output.</template>
      Active output requirements are separate.</p>
    </details>
    <ComputationProgress v-if="state === 'running'" :progress="progress" label="Diagram computation in progress"
      iteration-label="label rows compared" @cancel="cancel" />
    <p v-if="state === 'cancelled'" role="status">Computation cancelled.</p>
    <p v-if="state === 'error'" class="error" role="alert">{{ error }}</p>
    <template v-if="result">
      <DiagramView :problem="problem" :diagram="result.diagram" :layout="layout" :kind="result.options.kind" />
    </template>
  </CollapsibleSection>
</template>
