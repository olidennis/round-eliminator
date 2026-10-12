<script setup lang="ts">
import { computed } from 'vue'
import type { Problem } from './generated/Problem'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'

const props = defineProps<{ problem: Problem; result: ZeroRoundResult }>()
const labels = computed(() => {
  const p = props.problem
  return { input: p.input.kind === 'explicit' ? p.input.data.labels : null, output: p.output.labels }
})
function names(ids: number[], table: string[]): string {
  return ids.length ? ids.map(id => table[id]).join(' ') : '()'
}
</script>

<template>
  <section aria-label="Zero-round result" aria-live="polite">
    <p v-if="result.status === 'unsolvable'">NOT solvable in 0 rounds</p>
    <template v-else>
      <p>The problem IS solvable in 0 rounds</p>
      <p v-if="!result.strategy.length">There are no active configurations requiring an output.</p>
      <template v-else>
        <details class="help-details">
          <summary>Reading the strategy</summary>
          <p class="hint">Each row gives outputs in occurrence order. With inputs, use the input order
            shown in that row, using local port order to break ties between equal inputs.
            Without inputs, use local port order.</p>
        </details>
        <table>
          <thead><tr><th scope="col" title="Active degree" aria-label="Active degree">Deg.</th><th v-if="labels.input">Inputs</th><th>Outputs</th></tr></thead>
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
