<script setup lang="ts">
import { computed, useId } from 'vue'
import type { LabelId } from './generated/LabelId'
import type { Problem } from './generated/Problem'
import ConstraintView from './ConstraintView.vue'
import CollapsibleSection from './CollapsibleSection.vue'
import type { CollapsedSections, ProblemSection } from './session'

const { problem, collapsed = false, current = false, sections } = defineProps<{
  problem: Problem; collapsed?: boolean; current?: boolean; sections: CollapsedSections
}>()
const emit = defineEmits<{ toggle: []; section: [section: ProblemSection, collapsed: boolean] }>()
const bodyId = useId()
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
  <section class="result" aria-label="Problem">
    <div class="problem-header">
      <div class="problem-title"><span v-if="current" class="current-problem">Current</span></div>
      <div class="problem-actions">
        <button type="button" class="secondary-button" :aria-expanded="!collapsed" :aria-controls="bodyId"
          @click="emit('toggle')">{{ collapsed ? 'Expand' : 'Collapse' }}</button>
        <slot name="actions" />
      </div>
    </div>
    <div :id="bodyId" v-show="!collapsed">
      <CollapsibleSection title="Graph class" :collapsed="sections.graph"
        @update:collapsed="emit('section', 'graph', $event)">
        <p>Possible active degrees: {{ graphClass.active.join(', ') || 'none' }}.
          Possible passive degrees: {{ graphClass.passive.join(', ') || 'none' }}.</p>
      </CollapsibleSection>
      <CollapsibleSection v-if="input" title="Input constraints" class="constraint-section" :collapsed="sections.input"
        @update:collapsed="emit('section', 'input', $event)">
        <p>{{ input.labels.length }} input labels</p>
        <ConstraintView :active="input.active" :passive="input.passive"
          :graph-class="graphClass" :format-label="labelFormatter(input.labels)"
          :active-collapsed="sections.inputActive" :passive-collapsed="sections.inputPassive"
          @collapse="(side, collapsed) => emit('section', side === 'active' ? 'inputActive' : 'inputPassive', collapsed)" />
      </CollapsibleSection>
      <CollapsibleSection title="Output constraints" class="constraint-section" :collapsed="sections.output"
        @update:collapsed="emit('section', 'output', $event)">
        <p>{{ output.labels.length }} output labels</p>
        <ConstraintView :active="output.active" :passive="output.passive"
          :graph-class="graphClass" :format-label="labelFormatter(output.labels)"
          :active-collapsed="sections.outputActive" :passive-collapsed="sections.outputPassive"
          @collapse="(side, collapsed) => emit('section', side === 'active' ? 'outputActive' : 'outputPassive', collapsed)" />
      </CollapsibleSection>
      <CollapsibleSection v-if="mapping" title="Allowed outputs for each input" class="constraint-section"
        :collapsed="sections.mapping" @update:collapsed="emit('section', 'mapping', $event)">
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
      <slot />
    </div>
  </section>
</template>
