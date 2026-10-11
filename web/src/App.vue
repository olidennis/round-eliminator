<script setup lang="ts">
import { onBeforeUnmount, ref, shallowRef } from 'vue'
import { send, type Backend } from './api'
import type { ConstraintText } from './generated/ConstraintText'
import type { DegreeText } from './generated/DegreeText'
import type { Field } from './generated/Field'
import type { Problem } from './generated/Problem'
import type { ProblemText } from './generated/ProblemText'
import ConstraintEditor from './ConstraintEditor.vue'
import ProblemView from './ProblemView.vue'
import ZeroRoundAnalysis from './ZeroRoundAnalysis.vue'
import { problemPresets, type ProblemPreset } from './presets'

const kind = ref<ProblemText['kind']>('plain')
const output = ref<ConstraintText>({ active: 'M U^9\nP^10', passive: 'M UP^9\nU^10' })
const input = ref<ConstraintText>({ active: 'a b^9\nb^10', passive: 'a ab^9' })
const pairs = ref<ConstraintText>({ active: '(a,A)(b,B)^2', passive: '(a,A) (b,B)' })
const mapping = ref('a -> M\nb -> UP')
const degrees = ref<DegreeText>({ active: '', passive: '' })
const backend = ref<Backend>(
  new URLSearchParams(location.search).get('backend') === 'wasm' ? 'wasm' : 'server',
)
const pending = ref(false)
const parsed = shallowRef<Problem | null>(null)
let parsing: AbortController | null = null
onBeforeUnmount(() => parsing?.abort())
const error = ref('')
const selectedPreset = shallowRef<ProblemPreset | null>(null)

function loadPreset(preset: ProblemPreset) {
  // Copy the definition: editing a loaded preset must not change its next load.
  const definition = structuredClone(preset.definition)
  kind.value = definition.kind
  degrees.value = definition.degrees
  if (definition.kind === 'paired') {
    pairs.value = definition.constraints
  } else if (definition.kind === 'plain') {
    output.value = definition.constraints
  } else {
    input.value = definition.input
    output.value = definition.output
    if (definition.kind === 'mapped') mapping.value = definition.mapping
  }
  selectedPreset.value = preset
  error.value = ''
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
  parsed.value = null
  parsing = new AbortController()
  try {
    const response = await send({ type: 'parse_problem', data: definition() }, backend.value, { signal: parsing.signal })
    if (response.type === 'problem') {
      parsed.value = response.data
    } else if (response.type === 'error') {
      const location = response.data.location
      const where = location
        ? `${fieldNames[location.field]}${location.line === null ? '' : `, line ${location.line}`}: `
        : ''
      error.value = where + response.data.message
    }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause)
  } finally {
    pending.value = false
    parsing = null
  }
}
</script>

<template>
  <main>
    <header>
      <h1>Round Eliminator 3</h1>
      <p>Enter allowed configurations for the active and passive sides.</p>
    </header>

    <form @submit.prevent="start">
      <fieldset :disabled="pending" class="form-fields">
        <section class="presets" aria-labelledby="presets-title">
          <h2 id="presets-title">Examples</h2>
          <div class="preset-buttons">
            <button v-for="preset in problemPresets" :key="preset.name" type="button"
              :title="preset.description" @click="loadPreset(preset)">
              {{ preset.name }}
            </button>
          </div>
          <p class="hint">Load an example, edit it if needed, then press Start.</p>
          <p v-if="selectedPreset" class="hint">{{ selectedPreset.description }}</p>
        </section>
        <label>Problem type
          <select v-model="kind">
            <option value="plain">No input</option>
            <option value="independent">Input with independent output validity</option>
            <option value="paired">Input/output pairs</option>
            <option value="mapped">Input-to-output mapping</option>
          </select>
        </label>

        <template v-if="kind === 'paired'">
          <p>Write pairs as <code>(input,output)</code>. Adjacent pairs are alternatives;
            <code>(a,A)(b,B)^2</code> chooses either pair at each of two occurrences.</p>
          <ConstraintEditor v-model="pairs" title="Pair constraints" />
          <p class="hint">The input is promised to solve the input projection of these constraints.</p>
        </template>
        <template v-else>
          <p class="hint">Use <code>AB</code> for label choices, <code>(name)</code> for a longer label,
            and <code>^n</code> for repetition.</p>
          <ConstraintEditor v-if="kind !== 'plain'" v-model="input" title="Input constraints" />
          <ConstraintEditor v-model="output" title="Output constraints" />
          <section v-if="kind === 'mapped'" class="editor-section">
            <h2>Allowed outputs for each input</h2>
            <label>Input/output mapping
              <textarea v-model="mapping" rows="4" spellcheck="false" />
            </label>
            <p class="hint">One entry per input label, for example <code>a -> AB</code> or
              <code>(red) -> A(blue)</code>. An empty right side allows no outputs.</p>
          </section>
        </template>
        <section class="editor-section">
          <h2>Graph class</h2>
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
        </section>
        <p class="hint">A line containing <code>()</code> allows the degree-zero configuration.
          A blank constraint allows no configurations. Starred configurations are unsupported.</p>
        <div class="actions">
          <button type="submit">{{ pending ? 'Working…' : 'Start' }}</button>
          <label>Run on
            <select v-model="backend">
              <option value="server">Server</option>
              <option value="wasm">WebAssembly</option>
            </select>
          </label>
        </div>
      </fieldset>
    </form>

    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <ProblemView v-if="parsed" :problem="parsed">
      <ZeroRoundAnalysis :problem="parsed" :backend="backend" />
    </ProblemView>
  </main>
</template>
