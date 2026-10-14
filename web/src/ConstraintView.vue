<script setup lang="ts" generic="L">
import type { Constraint } from './generated/Constraint'
import type { GraphClass } from './generated/GraphClass'
import CollapsibleSection from './CollapsibleSection.vue'
import ConstraintTable from './ConstraintTable.vue'

const props = defineProps<{
  active: Constraint<L>
  passive: Constraint<L>
  graphClass: GraphClass
  formatLabel: (label: L) => string
  activeCollapsed?: boolean
  passiveCollapsed?: boolean
}>()
const emit = defineEmits<{ collapse: [side: 'active' | 'passive', collapsed: boolean] }>()

</script>

<template>
  <div class="constraints">
      <CollapsibleSection v-for="side in (['active', 'passive'] as const)" :key="side" :level="4"
        :title="side === 'active' ? 'Active' : 'Passive'" :collapsed="side === 'active' ? activeCollapsed : passiveCollapsed"
        @update:collapsed="emit('collapse', side, $event)">
        <ConstraintTable :constraint="props[side]" :degrees="graphClass[side]" :format-label="formatLabel" />
      </CollapsibleSection>
  </div>
</template>
