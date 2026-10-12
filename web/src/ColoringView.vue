<script setup lang="ts">
import { computed } from 'vue'
import type { Problem } from './generated/Problem'
import type { ColoringWitness } from './generated/ColoringWitness'

const props = defineProps<{ problem: Problem; witness: ColoringWitness; repeating?: boolean }>()
const inputLabels = computed(() => props.problem.input.kind === 'explicit' ? props.problem.input.data.labels : null)
const neighborColors = computed(() => props.witness.kind !== 'local_colors')
const colorOrder = computed(() => props.witness.kind === 'color_order')
const rows = computed(() => {
  const witness = props.witness
  if (witness.kind === 'color_order') {
    return [
      { relation: 'Smaller', behavior: witness.smaller },
      { relation: 'Larger', behavior: witness.larger },
    ].flatMap(({ relation, behavior }) => behavior.outputs_by_input.map((outputs, input) => ({
      own: 'Any', neighbor: relation, input, outputs,
    })))
  }
  if (witness.kind === 'local_colors') {
    return witness.behaviors.flatMap((behavior, own) => behavior.outputs_by_input.map((outputs, input) => ({
      own: props.repeating ? 'Any' : String(own + 1), neighbor: null as string | null, input, outputs,
    })))
  }
  return witness.behaviors.flatMap((behavior, own) => behavior.outputs_by_neighbor.flatMap((neighborBehavior, neighbor) =>
    own === neighbor ? [] : neighborBehavior.outputs_by_input.map((outputs, input) => ({
      own: String(own + 1), neighbor: String(neighbor + 1), input, outputs,
    }))))
})
</script>

<template>
  <details>
    <summary>Show strategy</summary>
    <details class="help-details">
      <summary>Reading the strategy</summary>
      <p class="hint">For each full local input and color configuration, choose a whole allowed active
        output configuration using these sets. Outputs must be chosen together to satisfy the active
        constraint. Every such choice satisfies the passive constraint.
        Empty sets may occur for unused inputs.</p>
      <p v-if="colorOrder" class="hint">Use whether each neighbor’s color is smaller or larger
        than the node’s own color, together with the original input. This strategy works
        for every locally feasible palette.</p>
      <p v-else-if="neighborColors" class="hint">At each port, look up both the node’s own color
        and that neighbor’s color, together with the original input label.</p>
      <p v-if="repeating && !colorOrder" class="hint">The same behavior works for every color.</p>
    </details>
    <table>
      <thead><tr><th>{{ neighborColors ? 'Own color' : 'Color' }}</th><th v-if="neighborColors">Neighbor color</th><th v-if="inputLabels">Input</th><th>Possible outputs</th></tr></thead>
      <tbody>
        <tr v-for="row in rows" :key="`${row.own}/${row.neighbor}/${row.input}`">
          <td>{{ row.own }}</td>
          <td v-if="neighborColors">{{ row.neighbor }}</td>
          <td v-if="inputLabels">{{ inputLabels[row.input] }}</td>
          <td>{{ row.outputs.length ? row.outputs.map(id => problem.output.labels[id]).join(' ') : '∅' }}</td>
        </tr>
      </tbody>
    </table>
  </details>
</template>
