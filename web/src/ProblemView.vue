<script setup lang="ts">
import { computed } from 'vue'
import type { LabelId } from './generated/LabelId'
import type { LabelPair } from './generated/LabelPair'
import type { InputOutputConstraintPair } from './generated/InputOutputConstraintPair'
import type { Problem } from './generated/Problem'
import ConstraintView from './ConstraintView.vue'

const { problem } = defineProps<{ problem: Problem }>()

const input = computed(() => problem.kind === 'plain' ? null : problem.data.input)
const output = computed(() => problem.kind === 'paired' ? null : problem.data.output)
const graphClass = computed(() => problem.data.graph_class)

function labelText(name: string): string {
  return [...name].length === 1 && !'()^*'.includes(name) ? name : `(${name})`
}

function labelFormatter(labels: string[]) {
  return (id: LabelId) => labelText(labels[id])
}

function pairFormatter(paired: InputOutputConstraintPair) {
  return (pair: LabelPair) => `(${paired.input_labels[pair.input]},${paired.output_labels[pair.output]})`
}
</script>

<template>
  <section class="result" aria-label="Parsed problem">
    <h2>Parsed problem</h2>
    <p>Possible active degrees: {{ graphClass.active.join(', ') || 'none' }}.
      Possible passive degrees: {{ graphClass.passive.join(', ') || 'none' }}.</p>
    <p v-if="problem.kind === 'paired'" class="hint">The input constraints below are projected from the pairs.
      The graph is promised to have an input satisfying them.</p>
    <template v-if="input">
      <p>{{ input.labels.length }} input labels</p>
      <ConstraintView title="Input constraints" :active="input.active" :passive="input.passive"
        :graph-class="graphClass" :format-label="labelFormatter(input.labels)" />
    </template>
    <template v-if="output">
      <p>{{ output.labels.length }} output labels</p>
      <ConstraintView title="Output constraints" :active="output.active" :passive="output.passive"
        :graph-class="graphClass" :format-label="labelFormatter(output.labels)" />
    </template>
    <template v-if="problem.kind === 'paired'">
      <p>{{ problem.data.constraints.output_labels.length }} output labels</p>
      <ConstraintView title="Pair constraints" :active="problem.data.constraints.active" :passive="problem.data.constraints.passive"
        :graph-class="graphClass" :format-label="pairFormatter(problem.data.constraints)" />
    </template>
    <section v-if="problem.kind === 'mapped'" class="constraint-section">
      <h3>Allowed outputs for each input</h3>
      <table>
        <thead><tr><th>Input</th><th>Allowed outputs</th></tr></thead>
        <tbody>
          <tr v-for="(labels, id) in problem.data.allowed_outputs" :key="id">
            <td>{{ labelText(problem.data.input.labels[id]) }}</td>
            <td>{{ labels.map(labelFormatter(problem.data.output.labels)).join(', ') || 'No allowed outputs' }}</td>
          </tr>
        </tbody>
      </table>
    </section>
  </section>
</template>
