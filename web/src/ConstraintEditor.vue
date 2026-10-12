<script setup lang="ts">
import { ref } from 'vue'
import type { ConstraintText } from './generated/ConstraintText'
import CollapsibleSection from './CollapsibleSection.vue'

defineProps<{ title: string }>()
const model = defineModel<ConstraintText>({ required: true })
const collapsed = ref(false)
</script>

<template>
  <CollapsibleSection class="editor-section" :title="title" :level="2" v-model:collapsed="collapsed">
    <div class="editors">
      <label>Active<textarea v-model="model.active" spellcheck="false" rows="4" /></label>
      <label>Passive<textarea v-model="model.passive" spellcheck="false" rows="4" /></label>
    </div>
  </CollapsibleSection>
</template>
