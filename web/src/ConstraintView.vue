<script setup lang="ts" generic="L">
import type { CondensedConfiguration } from './generated/CondensedConfiguration'
import type { Constraint } from './generated/Constraint'
import type { GraphClass } from './generated/GraphClass'
import CollapsibleSection from './CollapsibleSection.vue'

const props = defineProps<{
  active: Constraint<L>
  passive: Constraint<L>
  graphClass: GraphClass
  formatLabel: (label: L) => string
  activeCollapsed?: boolean
  passiveCollapsed?: boolean
}>()
const emit = defineEmits<{ collapse: [side: 'active' | 'passive', collapsed: boolean] }>()

function configurationText(configuration: CondensedConfiguration<L>): string {
  if (configuration.parts.length === 0) return '()'
  return configuration.parts.map((part) => {
    const choices = part.labels.map(props.formatLabel).join('')
    return part.multiplicity === 1 ? choices : `${choices}^${part.multiplicity}`
  }).join(' ')
}

function rows(side: 'active' | 'passive') {
  const groups = new Map(props[side].degrees.map((group) => [group.degree, group.configurations]))
  const allowed = new Set(props.graphClass[side])
  return [...new Set([...groups.keys(), ...allowed])].sort((a, b) => a - b).map((degree) => ({
    degree, configurations: groups.get(degree) ?? [], allowed: allowed.has(degree),
  }))
}
</script>

<template>
  <div class="constraints">
      <CollapsibleSection v-for="side in (['active', 'passive'] as const)" :key="side" :level="4"
        :title="side === 'active' ? 'Active' : 'Passive'" :collapsed="side === 'active' ? activeCollapsed : passiveCollapsed"
        @update:collapsed="emit('collapse', side, $event)">
        <table>
          <thead><tr><th scope="col" title="Degree" aria-label="Degree">Deg.</th><th>Configurations</th></tr></thead>
          <tbody>
            <tr v-for="row in rows(side)" :key="row.degree" :class="{ excluded: !row.allowed }">
              <td>{{ row.degree }}</td>
              <td>
                <div v-for="(configuration, index) in row.configurations" :key="index">
                  {{ configurationText(configuration) }}
                </div>
                <span v-if="row.configurations.length === 0">No allowed configurations</span>
                <small v-if="!row.allowed">Outside the selected graph class</small>
              </td>
            </tr>
            <tr v-if="rows(side).length === 0"><td colspan="2">No possible degrees</td></tr>
          </tbody>
        </table>
      </CollapsibleSection>
  </div>
</template>
