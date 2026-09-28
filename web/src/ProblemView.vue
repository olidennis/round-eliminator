<script setup lang="ts">
import type { LabelId } from './generated/LabelId'
import type { Part } from './generated/Part'
import type { PlainProblem } from './generated/PlainProblem'

const { problem } = defineProps<{ problem: PlainProblem }>()

function partText(part: Part<LabelId>): string {
  const choices = part.labels.map((id) => {
    const name = problem.labels[id]
    return [...name].length === 1 && !'()^*'.includes(name) ? name : `(${name})`
  }).join('')
  return part.multiplicity === 1 ? choices : `${choices}^${part.multiplicity}`
}

function configurationText(parts: Part<LabelId>[]): string {
  return parts.map(partText).join(' ')
}
</script>

<template>
  <section class="result" aria-label="Parsed problem">
    <h2>Parsed problem</h2>
    <p>{{ problem.labels.length }} labels</p>
    <div class="constraints">
      <section v-for="side in (['active', 'passive'] as const)" :key="side">
        <h3>{{ side === 'active' ? 'Active' : 'Passive' }}</h3>
        <table>
          <thead><tr><th>Degree</th><th>Allowed configurations</th></tr></thead>
          <tbody>
            <tr v-for="(group, index) in problem[side].degrees" :key="index">
              <td>{{ group.degree }}</td>
              <td><div v-for="(configuration, row) in group.configurations" :key="row">
                {{ configurationText(configuration.parts) }}
              </div></td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </section>
</template>
