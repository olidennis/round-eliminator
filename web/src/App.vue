<script setup lang="ts">
import { ref } from 'vue'
import { send, type Backend } from './api'
import type { PlainProblem } from './generated/PlainProblem'
import ProblemView from './ProblemView.vue'

const active = ref('M U^9\nP^10')
const passive = ref('M UP^9\nU^10')
const backend = ref<Backend>(
  new URLSearchParams(location.search).get('backend') === 'wasm' ? 'wasm' : 'server',
)
const pending = ref(false)
const problem = ref<PlainProblem | null>(null)
const error = ref('')

async function start() {
  pending.value = true
  error.value = ''
  problem.value = null
  try {
    const response = await send(
      { type: 'parse_problem', data: { active: active.value, passive: passive.value } },
      backend.value,
    )
    if (response.type === 'problem') {
      problem.value = response.data
    } else {
      const location = response.data.location
      error.value = location
        ? `${location.side} line ${location.line}: ${response.data.message}`
        : response.data.message
    }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause)
  } finally {
    pending.value = false
  }
}
</script>

<template>
  <main>
    <header>
      <h1>Round Eliminator 3</h1>
      <p>Enter allowed configurations for the active and passive sides.</p>
    </header>

    <form @submit.prevent="start">
      <div class="editors">
        <label>Active<textarea v-model="active" spellcheck="false" rows="8" /></label>
        <label>Passive<textarea v-model="passive" spellcheck="false" rows="8" /></label>
      </div>
      <div class="actions">
        <button type="submit" :disabled="pending">{{ pending ? 'Working…' : 'Start' }}</button>
        <label class="backend">Run on
          <select v-model="backend">
            <option value="server">Server</option>
            <option value="wasm">WebAssembly</option>
          </select>
        </label>
      </div>
    </form>

    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <ProblemView v-if="problem" :problem="problem" />
  </main>
</template>
