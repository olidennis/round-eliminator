<script setup lang="ts">
import { computed } from 'vue'
import type { LabelId } from './generated/LabelId'
import type { Problem } from './generated/Problem'
import ConstraintView from './ConstraintView.vue'

const { problem } = defineProps<{ problem: Problem }>()
const input = computed(() => problem.input.kind === 'explicit' ? problem.input.data : null)
const output = computed(() => problem.output)
const graphClass = computed(() => problem.graph_class)
const mapping = computed(() => problem.mapping.kind === 'restricted' ? problem.mapping.data : null)

function labelText(name: string): string {
  return [...name].length === 1 && !'()^*'.includes(name) ? name : `(${name})`
}

function labelFormatter(labels: string[]) {
  return (id: LabelId) => labelText(labels[id])
}
</script>

<template>
  <section class="result" aria-label="Parsed problem">
    <h2>Parsed problem</h2>
    <p>Possible active degrees: {{ graphClass.active.join(', ') || 'none' }}.
      Possible passive degrees: {{ graphClass.passive.join(', ') || 'none' }}.</p>
    <template v-if="input">
      <p>{{ input.labels.length }} input labels</p>
      <ConstraintView title="Input constraints" :active="input.active" :passive="input.passive"
        :graph-class="graphClass" :format-label="labelFormatter(input.labels)" />
    </template>
    <p>{{ output.labels.length }} output labels</p>
    <ConstraintView title="Output constraints" :active="output.active" :passive="output.passive"
      :graph-class="graphClass" :format-label="labelFormatter(output.labels)" />
    <section v-if="mapping" class="constraint-section">
      <h3>Allowed outputs for each input</h3>
      <table>
        <thead><tr><th>Input</th><th>Allowed outputs</th></tr></thead>
        <tbody>
          <tr v-for="(labels, id) in mapping" :key="id">
            <td>{{ input ? labelText(input.labels[id]) : 'Trivial input' }}</td>
            <td>{{ labels.map(labelFormatter(output.labels)).join(', ') || 'No allowed outputs' }}</td>
          </tr>
        </tbody>
      </table>
    </section>
    <slot />
  </section>
</template>
