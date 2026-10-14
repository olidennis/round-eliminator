<script setup lang="ts">
import { computed, ref } from 'vue'
import type { LabelId } from './generated/LabelId'
import type { Problem } from './generated/Problem'
import ConstraintView from './ConstraintView.vue'
import CollapsibleSection from './CollapsibleSection.vue'
import type { CollapsedSections, ProblemSection } from './session'

const { problem, number, collapsed = false, current = false, sections } = defineProps<{
  problem: Problem; number: number; collapsed?: boolean; current?: boolean; sections: CollapsedSections
}>()
const emit = defineEmits<{ toggle: []; section: [section: ProblemSection, collapsed: boolean] }>()
const input = computed(() => problem.input.kind === 'explicit' ? problem.input.data : null)
const output = computed(() => problem.output)
const graphClass = computed(() => problem.graph_class)
const mapping = computed(() => problem.mapping.kind === 'restricted' ? problem.mapping.data : null)
const collapsedTarget = ref<HTMLElement | null>(null)
const sectionKeys = computed<ProblemSection[]>(() => [
  ...(input.value ? ['input' as const] : []), 'output',
  ...(mapping.value ? ['mapping' as const] : []), 'zeroRound', 'coloring', 'diagram',
  'maximization',
])
const hasCollapsedSections = computed(() => sectionKeys.value.some(section => sections[section]))
const hasExpandedSections = computed(() => sectionKeys.value.some(section => !sections[section]))

function labelText(name: string): string {
  return [...name].length === 1 && !'()^*'.includes(name) ? name : `(${name})`
}

function labelFormatter(labels: string[]) {
  return (id: LabelId) => labelText(labels[id])
}
</script>

<template>
  <CollapsibleSection class="result" :title="`Problem ${number}`" :level="2" :collapsed="collapsed"
    @update:collapsed="emit('toggle')">
    <template #summary>
      <div class="header-graph-class" role="group" aria-label="Graph class">
        <span>Active degrees: {{ graphClass.active.join(', ') || 'none' }}.</span>
        <span>Passive degrees: {{ graphClass.passive.join(', ') || 'none' }}.</span>
      </div>
    </template>
    <template #actions>
      <span v-if="current" class="current-problem">Current</span>
      <slot name="actions" />
    </template>
    <div class="problem-layout">
      <div v-show="hasExpandedSections" class="problem-content">
        <div class="problem-definition">
          <CollapsibleSection v-if="input" title="Input constraints" class="constraint-section" :collapsed="sections.input"
            :collapsed-target="collapsedTarget" @update:collapsed="emit('section', 'input', $event)">
            <template #actions><span class="label-count">{{ input.labels.length }} labels</span></template>
            <ConstraintView :active="input.active" :passive="input.passive"
              :graph-class="graphClass" :format-label="labelFormatter(input.labels)"
              :active-collapsed="sections.inputActive" :passive-collapsed="sections.inputPassive"
              @collapse="(side, collapsed) => emit('section', side === 'active' ? 'inputActive' : 'inputPassive', collapsed)" />
          </CollapsibleSection>
          <CollapsibleSection title="Output constraints" class="constraint-section" :collapsed="sections.output"
            :collapsed-target="collapsedTarget" @update:collapsed="emit('section', 'output', $event)">
            <template #actions><span class="label-count">{{ output.labels.length }} labels</span></template>
            <ConstraintView :active="output.active" :passive="output.passive"
              :graph-class="graphClass" :format-label="labelFormatter(output.labels)"
              :active-collapsed="sections.outputActive" :passive-collapsed="sections.outputPassive"
              @collapse="(side, collapsed) => emit('section', side === 'active' ? 'outputActive' : 'outputPassive', collapsed)" />
          </CollapsibleSection>
          <CollapsibleSection v-if="mapping" title="Allowed outputs for each input" class="constraint-section"
            :collapsed="sections.mapping" :collapsed-target="collapsedTarget" @update:collapsed="emit('section', 'mapping', $event)">
            <table>
              <thead><tr><th>Input</th><th>Allowed outputs</th></tr></thead>
              <tbody>
                <tr v-for="(labels, id) in mapping" :key="id">
                  <td>{{ input ? labelText(input.labels[id]) : 'Trivial input' }}</td>
                  <td>{{ labels.map(labelFormatter(output.labels)).join(', ') || 'No allowed outputs' }}</td>
                </tr>
              </tbody>
            </table>
          </CollapsibleSection>
        </div>
        <slot :collapsed-target="collapsedTarget" />
      </div>
      <div ref="collapsedTarget" v-show="hasCollapsedSections" class="collapsed-sections"
        role="group" aria-label="Collapsed sections" />
    </div>
  </CollapsibleSection>
</template>
