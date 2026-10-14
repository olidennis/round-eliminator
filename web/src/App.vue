<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef } from 'vue'
import { defaultBackend, send, type Backend } from './api'
import type { ConstraintText } from './generated/ConstraintText'
import type { DegreeText } from './generated/DegreeText'
import type { Field } from './generated/Field'
import type { ProblemText } from './generated/ProblemText'
import type { Problem } from './generated/Problem'
import ConstraintEditor from './ConstraintEditor.vue'
import CollapsibleSection from './CollapsibleSection.vue'
import ProblemCard from './ProblemCard.vue'
import SessionControls from './SessionControls.vue'
import { newSavedProblem, type SavedProblem, type Session } from './session'
import { problemPresets, type ProblemPreset } from './presets'

const kind = ref<ProblemText['kind']>('plain')
const output = ref<ConstraintText>({ active: 'M U^9\nP^10', passive: 'M UP^9\nU^10' })
const input = ref<ConstraintText>({ active: 'a b^9\nb^10', passive: 'a ab^9' })
const pairs = ref<ConstraintText>({ active: '(a,A)(b,B)^2', passive: '(a,A) (b,B)' })
const mapping = ref('a -> M\nb -> UP')
const degrees = ref<DegreeText>({ active: '', passive: '' })
const backend = ref<Backend>(defaultBackend(new URL(location.href)))
const pending = ref(false)
const editorCollapsed = ref(false)
const mappingEditorCollapsed = ref(false)
// Stable keys give every problem its own analysis state and cancellation scope.
type OpenProblem = SavedProblem & { id: number }
const problems = ref<OpenProblem[]>([])
const currentProblemId = ref<number | null>(null)
let nextProblemId = 0
let parsing: AbortController | null = null
onBeforeUnmount(() => parsing?.abort())
const error = ref('')
const selectedPreset = shallowRef<ProblemPreset | null>(null)

function loadPreset(preset: ProblemPreset) {
  // Copy the definition: editing a loaded preset must not change its next load.
  loadDefinition(structuredClone(preset.definition))
  selectedPreset.value = preset
  error.value = ''
}

function loadDefinition(text: ProblemText) {
  kind.value = text.kind
  degrees.value = text.degrees
  if (text.kind === 'paired') {
    pairs.value = text.constraints
  } else if (text.kind === 'plain') {
    output.value = text.constraints
  } else {
    input.value = text.input
    output.value = text.output
    if (text.kind === 'mapped') mapping.value = text.mapping
  }
}

const session = computed<Session>(() => ({
  editor: definition(),
  problems: problems.value.map(({ id: _id, ...saved }) => saved),
}))

function importSession(saved: Session) {
  parsing?.abort()
  parsing = null
  pending.value = false
  loadDefinition(saved.editor)
  problems.value = saved.problems.map(problem => ({ ...problem,
    collapsed: problem.collapsed ?? false, collapsedSections: problem.collapsedSections ?? {}, id: nextProblemId++,
  }))
  currentProblemId.value = problems.value.at(-1)?.id ?? null
  selectedPreset.value = null
  error.value = ''
}

function closeProblem(id: number) {
  // Unmounting the card aborts its analyses without affecting other problems.
  problems.value = problems.value.filter(problem => problem.id !== id)
  if (currentProblemId.value === id) currentProblemId.value = problems.value.at(-1)?.id ?? null
}

function closeAllProblems() {
  problems.value = []
  currentProblemId.value = null
}

function appendProblem(problem: Problem) {
  const id = nextProblemId++
  problems.value.push({ ...newSavedProblem(problem), id })
  currentProblemId.value = id
}

function collapseProblems(mode: 'all' | 'others' | 'none') {
  for (const entry of problems.value) {
    entry.collapsed = mode === 'all' || (mode === 'others' && entry.id !== currentProblemId.value)
  }
}

const fieldNames: Record<Field, string> = {
  active: 'Active constraint', passive: 'Passive constraint',
  input_active: 'Input active constraint', input_passive: 'Input passive constraint',
  mapping: 'Input/output mapping', active_degrees: 'Active degrees', passive_degrees: 'Passive degrees',
}

function definition(): ProblemText {
  switch (kind.value) {
    case 'plain': return { kind: 'plain', constraints: output.value, degrees: degrees.value }
    case 'independent': return { kind: 'independent', input: input.value, output: output.value, degrees: degrees.value }
    case 'paired': return { kind: 'paired', constraints: pairs.value, degrees: degrees.value }
    case 'mapped': return { kind: 'mapped', input: input.value, output: output.value, mapping: mapping.value, degrees: degrees.value }
  }
}

async function start() {
  if (pending.value) return
  pending.value = true
  error.value = ''
  const current = new AbortController()
  parsing = current
  try {
    const response = await send({ type: 'parse_problem', data: definition() }, backend.value, { signal: current.signal })
    if (parsing !== current) return
    if (response.type === 'problem') {
      appendProblem(response.data)
    } else if (response.type === 'error') {
      const location = response.data.location
      const where = location
        ? `${fieldNames[location.field]}${location.line === null ? '' : `, line ${location.line}`}: `
        : ''
      error.value = where + response.data.message
    }
  } catch (cause) {
    if (parsing === current) error.value = cause instanceof Error ? cause.message : String(cause)
  } finally {
    if (parsing === current) {
      pending.value = false
      parsing = null
    }
  }
}
</script>

<template>
  <main>
    <header class="app-header">
      <h1>Round Eliminator 3</h1>
      <div class="app-toolbar">
        <div v-if="problems.length" class="actions problem-list-actions">
          <button type="button" class="secondary-button" @click="collapseProblems('all')">Collapse all</button>
          <button type="button" class="secondary-button" @click="collapseProblems('others')">Collapse all except current</button>
          <button type="button" class="secondary-button" @click="collapseProblems('none')">Expand all</button>
          <button type="button" class="secondary-button" @click="closeAllProblems">Close all</button>
        </div>
        <SessionControls :session="session" @import="importSession" />
      </div>
    </header>

    <form @submit.prevent="start">
      <fieldset :disabled="pending" class="form-fields">
        <CollapsibleSection class="problem-editor" title="New problem" :level="2"
          :collapsed="editorCollapsed" @update:collapsed="editorCollapsed = $event">
          <template #actions>
            <label>Problem type
              <select v-model="kind">
                <option value="plain">No input</option>
                <option value="independent">Input with independent output validity</option>
                <option value="paired">Input/output pairs</option>
                <option value="mapped">Input-to-output mapping</option>
              </select>
            </label>
            <label>Run on
              <select v-model="backend">
                <option value="server">Server</option>
                <option value="wasm">WebAssembly</option>
              </select>
            </label>
            <button type="submit">{{ pending ? 'Working…' : 'Start' }}</button>
          </template>
          <details class="presets">
            <summary>Examples</summary>
            <div class="preset-buttons">
              <button v-for="preset in problemPresets" :key="preset.name" type="button"
                :title="preset.description" @click="loadPreset(preset)">
                {{ preset.name }}
              </button>
            </div>
            <p class="hint">Load an example, edit it if needed, then press Start.</p>
          </details>
          <p v-if="selectedPreset" class="hint preset-description">{{ selectedPreset.description }}</p>
          <div class="editor-grid">
            <ConstraintEditor v-if="kind === 'paired'" v-model="pairs" title="Pair constraints" />
            <template v-else>
              <ConstraintEditor v-if="kind !== 'plain'" v-model="input" title="Input constraints" />
              <ConstraintEditor v-model="output" title="Output constraints" />
              <CollapsibleSection v-if="kind === 'mapped'" class="editor-section mapping-editor"
                title="Allowed outputs for each input" :level="2" v-model:collapsed="mappingEditorCollapsed">
                <label>Input/output mapping
                  <textarea v-model="mapping" rows="4" spellcheck="false" />
                </label>
              </CollapsibleSection>
            </template>
          </div>
          <div class="editor-details">
            <details class="degree-editor">
              <summary>Graph class: {{ degrees.active || degrees.passive ? 'custom degrees' : 'inferred' }}</summary>
              <div class="editors">
                <label>Possible active degrees
                  <input v-model="degrees.active" placeholder="Inferred if blank" />
                </label>
                <label>Possible passive degrees
                  <input v-model="degrees.passive" placeholder="Inferred if blank" />
                </label>
              </div>
              <p class="hint">List degrees separated by commas or spaces. Blank fields use degrees from
                {{ kind === 'plain' ? 'the output constraints' : kind === 'paired' ? 'the input projection' : 'the input constraints only' }}.</p>
            </details>
            <details class="help-details">
              <summary>Notation help</summary>
              <p v-if="kind === 'paired'" class="hint">Write pairs as <code>(input,output)</code>.
                Adjacent pairs are alternatives; <code>(a,A)(b,B)^2</code> chooses either pair at each
                of two occurrences. The input is promised to solve the input projection.</p>
              <p v-else class="hint">Use <code>AB</code> for label choices, <code>(name)</code> for a
                longer label, and <code>^n</code> for repetition.</p>
              <p v-if="kind === 'mapped'" class="hint">Write one mapping entry per input label, such as
                <code>a -> AB</code> or <code>(red) -> A(blue)</code>. An empty right side allows no outputs.</p>
              <p class="hint">A line containing <code>()</code> allows the degree-zero configuration.
                A blank constraint allows no configurations. Starred configurations are unsupported.</p>
            </details>
          </div>
        </CollapsibleSection>
      </fieldset>
    </form>

    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <section v-if="problems.length" aria-label="Problems">
      <ProblemCard v-for="(entry, index) in problems" :key="entry.id" :saved="entry" :number="index + 1" :backend="backend"
        :current="entry.id === currentProblemId" @activate="currentProblemId = entry.id"
        @collapse="entry.collapsed = $event"
        @section="(section, collapsed) => entry.collapsedSections[section] = collapsed"
        @close="closeProblem(entry.id)"
        @coloring="entry.coloring = $event" @diagram="entry.diagram = $event"
        @transformed="appendProblem" />
    </section>
  </main>
</template>
