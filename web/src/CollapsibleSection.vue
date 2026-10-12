<script setup lang="ts">
import { nextTick, ref, useId } from 'vue'

defineOptions({ inheritAttrs: false })
const { title, level = 3, collapsed = false, collapsedTarget } = defineProps<{
  title: string; level?: 2 | 3 | 4; collapsed?: boolean; collapsedTarget?: HTMLElement | null
}>()
const emit = defineEmits<{ 'update:collapsed': [collapsed: boolean] }>()
const id = useId()
const titleButton = ref<HTMLButtonElement | null>(null)

async function toggle() {
  const focused = document.activeElement === titleButton.value
  emit('update:collapsed', !collapsed)
  // Moving a focused header between containers can clear keyboard focus.
  await nextTick()
  if (focused) titleButton.value?.focus({ preventScroll: true })
}

function toggleHeader(event: MouseEvent) {
  // Header controls remain usable without folding their surrounding section.
  if ((event.target as Element).closest('button, input, select, textarea, a, label')) return
  void toggle()
}
</script>

<template>
  <!-- Teleport moves the existing DOM, retaining bodies and computation state.
       Disabling it restores the section to its original position. -->
  <Teleport :to="collapsedTarget ?? 'body'" :disabled="!collapsed || !collapsedTarget">
    <section v-bind="$attrs" class="section-panel" :aria-labelledby="`${id}-title`">
      <div class="section-header" @click="toggleHeader">
        <component :is="`h${level}`" :id="`${id}-title`" class="section-heading">
          <button ref="titleButton" type="button" class="section-title" :aria-expanded="!collapsed" :aria-controls="id"
            @click.stop="toggle">
            <span class="disclosure-arrow" aria-hidden="true">{{ collapsed ? '▸' : '▾' }}</span>{{ title }}
          </button>
        </component>
        <div v-if="$slots.summary" class="section-summary"><slot name="summary" /></div>
        <div v-if="$slots.actions" class="section-actions">
          <slot name="actions" />
        </div>
      </div>
      <!-- Hiding preserves results, form settings, and running computations. -->
      <div :id="id" v-show="!collapsed" class="section-body"><slot /></div>
    </section>
  </Teleport>
</template>
