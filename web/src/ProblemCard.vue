<script setup lang="ts">
import { ref } from 'vue'
import type { Backend } from './api'
import type { Problem } from './generated/Problem'
import type { ProblemSection, SavedProblem } from './session'
import ProblemView from './ProblemView.vue'
import ZeroRoundAnalysis from './ZeroRoundAnalysis.vue'
import ColoringAnalysis from './ColoringAnalysis.vue'
import DiagramAnalysis from './DiagramAnalysis.vue'
import MaximizationAnalysis from './MaximizationAnalysis.vue'
import ProblemTransformations from './ProblemTransformations.vue'

defineProps<{ saved: SavedProblem; number: number; backend: Backend; current: boolean }>()
const emit = defineEmits<{
  close: []
  activate: []
  collapse: [collapsed: boolean]
  section: [section: ProblemSection, collapsed: boolean]
  coloring: [analysis: SavedProblem['coloring']]
  diagram: [analysis: SavedProblem['diagram']]
  transformed: [problem: Problem]
}>()
const maximizing = ref(false)
const transforming = ref(false)
</script>

<template>
  <ProblemView :problem="saved.state.problem" :number="number" :collapsed="saved.collapsed" :current="current"
    :sections="saved.collapsedSections" @section="(section, collapsed) => emit('section', section, collapsed)"
    @click="emit('activate')" @focusin="emit('activate')" @toggle="emit('collapse', !saved.collapsed)">
    <template #actions>
      <ProblemTransformations :state="saved.state" :backend="backend"
        :disabled="maximizing" @busy="transforming = $event"
        @created="emit('transformed', $event)" />
      <button type="button" class="close-problem" aria-label="Close problem" title="Close problem"
        @click.stop="emit('close')"><span aria-hidden="true">×</span></button>
    </template>
    <template #default="{ collapsedTarget }">
      <div class="problem-analyses">
        <MaximizationAnalysis :state="saved.state" :backend="backend"
          :disabled="transforming" @busy="maximizing = $event"
          :collapsed="saved.collapsedSections.maximization" :collapsed-target="collapsedTarget"
          @collapse="emit('section', 'maximization', $event)" />
        <ZeroRoundAnalysis :state="saved.state" :backend="backend"
          :collapsed="saved.collapsedSections.zeroRound" :collapsed-target="collapsedTarget" @collapse="emit('section', 'zeroRound', $event)" />
        <ColoringAnalysis :state="saved.state" :backend="backend" :saved="saved.coloring"
          :collapsed="saved.collapsedSections.coloring" :collapsed-target="collapsedTarget" @collapse="emit('section', 'coloring', $event)"
          @update="emit('coloring', $event)" />
        <DiagramAnalysis :state="saved.state" :backend="backend" :saved="saved.diagram"
          :collapsed="saved.collapsedSections.diagram" :collapsed-target="collapsedTarget" @collapse="emit('section', 'diagram', $event)"
          @update="emit('diagram', $event)" />
      </div>
    </template>
  </ProblemView>
</template>
