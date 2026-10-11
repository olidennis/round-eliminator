<script setup lang="ts">
import type { Progress } from './generated/Progress'
import type { Phase } from './generated/Phase'

defineProps<{ progress: Progress | null; label: string }>()
defineEmits<{ cancel: [] }>()

const phases: Record<Phase, string> = {
  queued: 'Waiting to start…', parsing: 'Reading the problem…',
  enumerating: 'Enumerating input configurations…', encoding: 'Building the SAT constraints…',
  solving: 'Searching for a strategy…', verifying: 'Checking configurations…',
}
</script>

<template>
  <div class="computation-progress">
    <progress :aria-label="label" />
    <div role="status">
      {{ progress ? phases[progress.phase] : 'Starting…' }}
      <small v-if="progress && progress.iterations > 0">{{ progress.iterations }} candidate
        {{ progress.iterations === 1 ? 'strategy examined' : 'strategies examined' }}.</small>
    </div>
    <button type="button" @click="$emit('cancel')">Cancel</button>
  </div>
</template>
