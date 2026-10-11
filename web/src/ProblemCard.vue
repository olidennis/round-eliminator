<script setup lang="ts">
import type { Backend } from './api'
import type { ProblemSection, SavedProblem } from './session'
import ProblemView from './ProblemView.vue'
import ZeroRoundAnalysis from './ZeroRoundAnalysis.vue'
import ColoringAnalysis from './ColoringAnalysis.vue'

defineProps<{ saved: SavedProblem; backend: Backend; current: boolean }>()
const emit = defineEmits<{
  close: []
  activate: []
  collapse: [collapsed: boolean]
  section: [section: ProblemSection, collapsed: boolean]
  zeroRound: [result: SavedProblem['zeroRound']]
  coloring: [analysis: SavedProblem['coloring']]
}>()
</script>

<template>
  <ProblemView :problem="saved.problem" :collapsed="saved.collapsed" :current="current"
    :sections="saved.collapsedSections" @section="(section, collapsed) => emit('section', section, collapsed)"
    @click="emit('activate')" @focusin="emit('activate')" @toggle="emit('collapse', !saved.collapsed)">
    <template #actions>
      <button type="button" class="secondary-button" aria-label="Close problem" @click.stop="emit('close')">Close</button>
    </template>
    <ZeroRoundAnalysis :problem="saved.problem" :backend="backend" :saved="saved.zeroRound"
      :collapsed="saved.collapsedSections.zeroRound" @collapse="emit('section', 'zeroRound', $event)"
      @update="emit('zeroRound', $event)" />
    <ColoringAnalysis :problem="saved.problem" :backend="backend" :saved="saved.coloring"
      :collapsed="saved.collapsedSections.coloring" @collapse="emit('section', 'coloring', $event)"
      @update="emit('coloring', $event)" />
  </ProblemView>
</template>
