<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import cytoscape, { type Core, type Layouts, type StylesheetJson } from 'cytoscape'
import dagre from 'cytoscape-dagre'
import type { Diagram } from './generated/Diagram'
import type { Problem } from './generated/Problem'
import type { DiagramLayout } from './session'
import type { DiagramKind } from './generated/DiagramKind'
import { diagramNodeName, diagramText } from './diagram'
import { downloadFile } from './files'

cytoscape.use(dagre)
const props = defineProps<{ problem: Problem; diagram: Diagram; layout: DiagramLayout; kind: DiagramKind }>()
const container = ref<HTMLDivElement | null>(null)
const selected = ref('')
const status = ref('')
const error = ref('')
const manualText = ref('')
const names = computed(() => props.diagram.components.map(members =>
  members.map(index => diagramNodeName(props.problem, props.diagram.nodes[index]))))
let graph: Core | null = null
let activeLayout: Layouts | null = null
let observer: ResizeObserver | null = null
let pendingLayout = false
const fontSize = 18
const lineHeight = 1.25
const labelWidth = 220
const textMeasure = document.createElement('canvas').getContext('2d')!
textMeasure.font = `${fontSize}px monospace`

const style: StylesheetJson = [
  { selector: 'node', style: { 'background-color': '#edf3fb', 'border-color': '#5684bd', 'border-width': 1.5,
    color: '#17334f', label: 'data(label)', 'text-wrap': 'wrap', 'text-max-width': `${labelWidth}px`, 'font-size': fontSize,
    'text-overflow-wrap': 'anywhere', 'line-height': lineHeight,
    'font-family': 'monospace', 'text-valign': 'center', 'text-halign': 'center', shape: 'round-rectangle',
    width: 'data(width)', height: 'data(height)', padding: '6px' } },
  { selector: 'edge', style: { width: 2, 'line-color': '#7891ac', 'target-arrow-color': '#7891ac',
    'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 1.2 } },
  { selector: ':selected', style: { 'background-color': '#d1e3fb', 'border-color': '#175fbd',
    'line-color': '#175fbd', 'target-arrow-color': '#175fbd', 'border-width': 3 } },
]

function nodeData(members: string[], index: number) {
  // Large SCCs keep a compact graph vertex; selection and text export retain
  // every member. Numeric sizes avoid the library's deprecated label sizing.
  const lines = members.length <= 4 ? members : [...members.slice(0, 3), `+ ${members.length - 3} more`]
  const displayedLines = lines.map((name, i) => i === 0 ? name : `≡ ${name}`)
  const widths = displayedLines.map(line => textMeasure.measureText(line).width)
  const width = widths.reduce((maximum, value) => Math.max(maximum, value), 16)
  const wrappedLines = widths.reduce((count, value) => count + Math.max(1, Math.ceil(value / labelWidth)), 0)
  return { id: `n${index}`, label: displayedLines.join('\n'), members,
    width: Math.ceil(Math.min(labelWidth, width)), height: Math.ceil(wrappedLines * fontSize * lineHeight) }
}

function arrange() {
  if (!graph || !container.value?.clientWidth || !container.value.clientHeight) { pendingLayout = true; return }
  graph.resize()
  activeLayout?.stop()
  graph.elements().stop()
  const hierarchical: dagre.DagreLayoutOptions = { name: 'dagre', rankDir: 'TB', rankSep: 50, nodeSep: 24, fit: false, animate: false }
  activeLayout = graph.layout(props.layout === 'hierarchical'
    ? hierarchical
    : { name: 'cose', fit: false, animate: false })
  activeLayout.run()
  fit()
  pendingLayout = false
}

function render() {
  activeLayout?.stop()
  activeLayout = null
  graph?.destroy()
  graph = null
  selected.value = status.value = error.value = manualText.value = ''
  if (!container.value?.clientWidth || !container.value.clientHeight) { pendingLayout = true; return }
  graph = cytoscape({ container: container.value, style,
    elements: [
      ...names.value.map((members, index) => ({ data: nodeData(members, index) })),
      ...props.diagram.edges.map(([source, target]) => ({ data: { id: `e${source}-${target}`, source: `n${source}`, target: `n${target}` } })),
    ], layout: { name: 'preset' }, minZoom: .08, maxZoom: 4,
  })
  graph.on('select unselect', () => {
    selected.value = graph?.elements(':selected').map(element => element.isNode()
      ? (element.data('members') as string[]).join(' ≡ ')
      : `${graph?.getElementById(element.data('source')).data('label')} → ${graph?.getElementById(element.data('target')).data('label')}`).join('; ') ?? ''
  })
  arrange()
}

onMounted(() => {
  observer = new ResizeObserver(() => {
    if (!container.value?.clientWidth || !container.value.clientHeight) return
    if (!graph) render()
    else { graph.resize(); if (pendingLayout) arrange() }
  })
  if (container.value) observer.observe(container.value)
  render()
})
onBeforeUnmount(() => { observer?.disconnect(); activeLayout?.stop(); graph?.destroy(); graph = null })
watch(() => props.diagram, render)
watch(() => props.layout, arrange)

async function copy() {
  error.value = status.value = manualText.value = ''
  const text = diagramText(props.problem, props.diagram)
  try { await navigator.clipboard.writeText(text); status.value = 'Diagram copied.' }
  catch { manualText.value = text; status.value = 'Select and copy the diagram text below.' }
}
function exportText() { downloadFile(new Blob([diagramText(props.problem, props.diagram)], { type: 'text/plain;charset=utf-8' }), `diagram-${props.kind}.txt`) }
function fit() {
  if (!graph) return
  graph.fit(undefined, 30)
  // Fit large diagrams, but keep small ones at a readable natural size.
  if (graph.zoom() > 1) { graph.zoom(1); graph.center() }
}
async function exportImage() {
  error.value = ''
  try {
    if (!graph) throw new Error('Expand the diagram before exporting its image.')
    const image = await graph.png({ output: 'blob-promise', full: true, bg: '#ffffff', scale: 2 })
    downloadFile(image, `diagram-${props.kind}.png`)
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
</script>

<template>
  <section class="diagram-view" aria-label="Label-strength diagram">
    <p role="status">{{ diagram.complete ? 'Complete diagram.' : 'Partial diagram: missing arrows are unknown.' }}
      {{ diagram.nodes.length }} labels in {{ diagram.components.length }} equivalence groups;
      {{ diagram.edges.length }} displayed arrows.</p>
    <p class="hint">Equivalent labels are grouped together. Transitive arrows and self-loops are hidden.
      Drag nodes, pan, zoom, or select nodes and arrows.</p>
    <div ref="container" class="diagram-canvas" role="img" aria-label="Directed graph of passive label strength" />
    <div class="actions diagram-actions">
      <button type="button" class="secondary-button" @click="fit">Fit</button>
      <button type="button" class="secondary-button" @click="arrange">Reset layout</button>
      <button type="button" @click="copy">Export to Clipboard</button>
      <button type="button" @click="exportText">Download text</button>
      <button type="button" :disabled="!diagram.components.length" @click="exportImage">Download PNG</button>
    </div>
    <p v-if="selected" role="status">Selected: {{ selected }}</p>
    <p v-if="status" role="status">{{ status }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <label v-if="manualText">Diagram text
      <textarea :value="manualText" readonly rows="6" @focus="($event.target as HTMLTextAreaElement).select()" />
    </label>
    <details>
      <summary>Nodes and arrows as text</summary>
      <pre class="diagram-text">{{ diagramText(problem, diagram) }}</pre>
    </details>
  </section>
</template>
