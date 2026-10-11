<script setup lang="ts">
import { useId } from 'vue'

const { title, level = 3, collapsed = false } = defineProps<{ title: string; level?: 3 | 4; collapsed?: boolean }>()
const emit = defineEmits<{ 'update:collapsed': [collapsed: boolean] }>()
const id = useId()
</script>

<template>
  <section :aria-labelledby="`${id}-title`">
    <div class="section-header">
      <component :is="`h${level}`" :id="`${id}-title`">{{ title }}</component>
      <button type="button" class="section-toggle" :aria-expanded="!collapsed" :aria-controls="id"
        :aria-label="`${collapsed ? 'Expand' : 'Collapse'} ${title}`"
        @click="emit('update:collapsed', !collapsed)">{{ collapsed ? 'Expand' : 'Collapse' }}</button>
    </div>
    <!-- Hiding preserves results, form settings, and running computations. -->
    <div :id="id" v-show="!collapsed"><slot /></div>
  </section>
</template>
