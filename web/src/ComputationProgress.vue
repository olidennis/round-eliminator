<script setup lang="ts">
import { computed } from 'vue'
import type { Progress } from './generated/Progress'
import type { Phase } from './generated/Phase'

const props = defineProps<{ progress: Progress | null; label: string; iterationLabel?: string }>()
defineEmits<{ cancel: [] }>()
const work = computed(() => props.progress?.work)
const fraction = computed(() => work.value ?
  (work.value.total === 0 ? 1 : Math.min(1, work.value.completed / work.value.total)) : undefined)

const phases: Record<Phase, string> = {
  queued: 'Waiting to start…', parsing: 'Reading the problem…',
  enumerating: 'Enumerating input configurations…', encoding: 'Building the SAT constraints…',
  solving: 'Searching for a strategy…', verifying: 'Checking configurations…',
  preparing: 'Preparing constraints…', comparing: 'Comparing label contexts…',
  reducing: 'Grouping equivalent labels and reducing arrows…',
  maximizing: 'Maximizing the passive constraint…',
  transforming: 'Building the transformed problem…',
  closing_active: 'Computing the active fixed-point closure…',
  closing_passive: 'Computing the passive fixed-point closure…',
  cleaning: 'Removing redundant configurations and unused labels…',
}
</script>

<template>
  <div class="computation-progress">
    <progress :aria-label="label" :value="fraction" :max="1" />
    <div role="status">
      {{ progress ? phases[progress.phase] : 'Starting…' }}
      <small v-if="work">Pass {{ work.pass }}: {{ Math.floor((fraction ?? 0) * 100) }}%
        ({{ work.completed }} / {{ work.total }} {{ iterationLabel ?? 'items processed' }}).</small>
      <small v-else-if="progress && progress.iterations > 0">{{ progress.iterations }}
        {{ iterationLabel ?? (progress.iterations === 1 ? 'candidate strategy examined' : 'candidate strategies examined') }}.</small>
    </div>
    <button type="button" @click="$emit('cancel')">Cancel</button>
  </div>
</template>
