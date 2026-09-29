<script setup lang="ts">
import type { ConstraintText } from './generated/ConstraintText'

defineProps<{ title: string }>()
const model = defineModel<ConstraintText>({ required: true })
</script>

<template>
  <section class="editor-section">
    <h2>{{ title }}</h2>
    <div class="editors">
      <label>Active<textarea v-model="model.active" spellcheck="false" rows="5" /></label>
      <label>Passive<textarea v-model="model.passive" spellcheck="false" rows="5" /></label>
    </div>
  </section>
</template>
