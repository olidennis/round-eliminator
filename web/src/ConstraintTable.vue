<script setup lang="ts" generic="L">
import { computed } from 'vue'
import type { CondensedConfiguration } from './generated/CondensedConfiguration'
import type { Constraint } from './generated/Constraint'

const props = defineProps<{
  constraint: Constraint<L>
  degrees: number[]
  formatLabel: (label: L) => string
}>()

function configurationText(configuration: CondensedConfiguration<L>): string {
  if (configuration.parts.length === 0) return '()'
  return configuration.parts.map((part) => {
    const choices = part.labels.map(props.formatLabel).join('')
    return part.multiplicity === 1 ? choices : `${choices}^${part.multiplicity}`
  }).join(' ')
}

const rows = computed(() => {
  const groups = new Map(props.constraint.degrees.map((group) => [group.degree, group.configurations]))
  const allowed = new Set(props.degrees)
  return [...new Set([...groups.keys(), ...allowed])].sort((a, b) => a - b).map((degree) => ({
    degree, configurations: groups.get(degree) ?? [], allowed: allowed.has(degree),
  }))
})
</script>

<template>
  <table>
    <thead><tr><th scope="col" title="Degree" aria-label="Degree">Deg.</th><th>Configurations</th></tr></thead>
    <tbody>
      <tr v-for="row in rows" :key="row.degree" :class="{ excluded: !row.allowed }">
        <td>{{ row.degree }}</td>
        <td>
          <div v-for="(configuration, index) in row.configurations" :key="index">
            {{ configurationText(configuration) }}
          </div>
          <span v-if="row.configurations.length === 0">No allowed configurations</span>
          <small v-if="!row.allowed">Outside the selected graph class</small>
        </td>
      </tr>
      <tr v-if="rows.length === 0"><td colspan="2">No possible degrees</td></tr>
    </tbody>
  </table>
</template>
