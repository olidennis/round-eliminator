<script setup lang="ts">
import type { Progress } from './generated/Progress'
import type { Phase } from './generated/Phase'

defineProps<{ progress: Progress | null; label: string; iterationLabel?: string }>()
defineEmits<{ cancel: [] }>()

const phases: Record<Phase, string> = {
  queued: 'Waiting to start…', parsing: 'Reading the problem…',
  enumerating: 'Enumerating input configurations…', encoding: 'Building the SAT constraints…',
  solving: 'Searching for a strategy…', verifying: 'Checking configurations…',
  preparing: 'Preparing constraints…', comparing: 'Comparing label contexts…',
  reducing: 'Grouping equivalent labels and reducing arrows…',
}
</script>

<template>
  <div class="computation-progress">
    <progress :aria-label="label" />
    <div role="status">
      {{ progress ? phases[progress.phase] : 'Starting…' }}
      <small v-if="progress && progress.iterations > 0">{{ progress.iterations }}
        {{ iterationLabel ?? (progress.iterations === 1 ? 'candidate strategy examined' : 'candidate strategies examined') }}.</small>
    </div>
    <button type="button" @click="$emit('cancel')">Cancel</button>
  </div>
</template>
