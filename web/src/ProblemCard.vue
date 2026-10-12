<script setup lang="ts">
import type { Backend } from './api'
import type { ProblemSection, SavedProblem } from './session'
import ProblemView from './ProblemView.vue'
import ZeroRoundAnalysis from './ZeroRoundAnalysis.vue'
import ColoringAnalysis from './ColoringAnalysis.vue'
import DiagramAnalysis from './DiagramAnalysis.vue'

defineProps<{ saved: SavedProblem; number: number; backend: Backend; current: boolean }>()
const emit = defineEmits<{
  close: []
  activate: []
  collapse: [collapsed: boolean]
  section: [section: ProblemSection, collapsed: boolean]
  zeroRound: [result: SavedProblem['zeroRound']]
  coloring: [analysis: SavedProblem['coloring']]
  diagram: [analysis: SavedProblem['diagram']]
}>()
</script>

<template>
  <ProblemView :problem="saved.problem" :number="number" :collapsed="saved.collapsed" :current="current"
    :sections="saved.collapsedSections" @section="(section, collapsed) => emit('section', section, collapsed)"
    @click="emit('activate')" @focusin="emit('activate')" @toggle="emit('collapse', !saved.collapsed)">
    <template #actions>
      <button type="button" class="close-problem" aria-label="Close problem" title="Close problem"
        @click.stop="emit('close')"><span aria-hidden="true">×</span></button>
    </template>
    <template #default="{ collapsedTarget }">
      <div class="problem-analyses">
        <ZeroRoundAnalysis :problem="saved.problem" :backend="backend" :saved="saved.zeroRound"
          :collapsed="saved.collapsedSections.zeroRound" :collapsed-target="collapsedTarget" @collapse="emit('section', 'zeroRound', $event)"
          @update="emit('zeroRound', $event)" />
        <ColoringAnalysis :problem="saved.problem" :backend="backend" :saved="saved.coloring"
          :collapsed="saved.collapsedSections.coloring" :collapsed-target="collapsedTarget" @collapse="emit('section', 'coloring', $event)"
          @update="emit('coloring', $event)" />
        <DiagramAnalysis :problem="saved.problem" :backend="backend" :saved="saved.diagram"
          :collapsed="saved.collapsedSections.diagram" :collapsed-target="collapsedTarget" @collapse="emit('section', 'diagram', $event)"
          @update="emit('diagram', $event)" />
      </div>
    </template>
  </ProblemView>
</template>
