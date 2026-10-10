<script setup lang="ts">
import { computed } from 'vue'
import type { Problem } from './generated/Problem'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'

const props = defineProps<{ problem: Problem; result: ZeroRoundResult }>()
const labels = computed(() => {
  const p = props.problem
  switch (p.kind) {
    case 'plain': return { input: null, output: p.data.output.labels }
    case 'independent':
    case 'mapped': return { input: p.data.input.labels, output: p.data.output.labels }
    case 'paired': return { input: p.data.constraints.input_labels, output: p.data.constraints.output_labels }
  }
})
function names(ids: number[], table: string[]): string {
  return ids.length ? ids.map(id => table[id]).join(' ') : '()'
}
</script>

<template>
  <section aria-label="Zero-round result" aria-live="polite">
    <p v-if="result.status === 'unsolvable'">No deterministic active-side zero-round strategy exists.</p>
    <template v-else>
      <p>A deterministic active-side zero-round strategy exists.</p>
      <p v-if="!result.strategy.length">There are no active configurations requiring an output.</p>
      <template v-else>
        <p class="hint">Each row gives outputs in occurrence order. With inputs, use the input order
          shown in that row, using local port order to break ties between equal inputs.
          Without inputs, use local port order.</p>
        <table>
          <thead><tr><th>Active degree</th><th v-if="labels.input">Inputs</th><th>Outputs</th></tr></thead>
          <tbody>
            <tr v-for="(rule, index) in result.strategy" :key="index">
              <td>{{ rule.degree }}</td>
              <td v-if="labels.input && rule.input">{{ names(rule.input, labels.input) }}</td>
              <td>{{ names(rule.output, labels.output) }}</td>
            </tr>
          </tbody>
        </table>
      </template>
    </template>
  </section>
</template>
