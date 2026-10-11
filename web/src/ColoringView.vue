<script setup lang="ts">
import { computed } from 'vue'
import type { Problem } from './generated/Problem'
import type { ColoringWitness } from './generated/ColoringWitness'

const props = defineProps<{ problem: Problem; witness: ColoringWitness; repeating?: boolean }>()
const inputLabels = computed(() => props.problem.input.kind === 'explicit' ? props.problem.input.data.labels : null)
</script>

<template>
  <details>
    <summary>Show strategy</summary>
    <p class="hint">For each full local input and color configuration, choose an allowed active
      output configuration using these sets. Every such choice satisfies the passive constraint.
      Empty sets may occur for unused inputs.</p>
    <p v-if="repeating" class="hint">The same behavior works for every color.</p>
    <table>
      <thead><tr><th>Color</th><th v-if="inputLabels">Input</th><th>Possible outputs</th></tr></thead>
      <tbody>
        <template v-for="(behavior, color) in witness.behaviors" :key="color">
          <tr v-for="(outputs, input) in behavior.outputs_by_input" :key="input">
            <td>{{ repeating ? 'Any' : color + 1 }}</td>
            <td v-if="inputLabels">{{ inputLabels[input] }}</td>
            <td>{{ outputs.length ? outputs.map(id => problem.output.labels[id]).join(' ') : '∅' }}</td>
          </tr>
        </template>
      </tbody>
    </table>
  </details>
</template>
