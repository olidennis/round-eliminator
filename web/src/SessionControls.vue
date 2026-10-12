<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { deserializeSession, serializeSession, sessionFromHash, sessionLink, type Session } from './session'
import { downloadFile } from './files'

const props = defineProps<{ session: Session }>()
const emit = defineEmits<{ import: [session: Session] }>()
const fileInput = ref<HTMLInputElement | null>(null)
const busy = ref(false)
const error = ref('')
const status = ref('')
const manualLink = ref('')
let hashRequest = 0

function reportError(cause: unknown) {
  error.value = cause instanceof Error ? cause.message : String(cause)
}

function exportSession() {
  error.value = ''
  downloadFile(new Blob([serializeSession(props.session)], { type: 'application/json' }), 'round-eliminator-3.re3.json')
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  busy.value = true
  error.value = status.value = manualLink.value = ''
  try {
    emit('import', deserializeSession(await file.text()))
    status.value = 'Session imported.'
  } catch (cause) { reportError(cause) }
  finally { busy.value = false; input.value = '' }
}

async function copyLink() {
  busy.value = true
  error.value = status.value = manualLink.value = ''
  try {
    const link = await sessionLink(props.session)
    try {
      await navigator.clipboard.writeText(link)
      status.value = 'Link copied.'
    } catch {
      // Clipboard access can be unavailable on an HTTP server. Offer the same
      // link for manual copying rather than losing the completed export.
      manualLink.value = link
      status.value = 'Select and copy the link below.'
    }
  } catch (cause) { reportError(cause) }
  finally { busy.value = false }
}

async function loadHash() {
  const current = ++hashRequest
  if (!location.hash) return
  error.value = status.value = manualLink.value = ''
  try {
    const session = await sessionFromHash(location.hash)
    if (current !== hashRequest) return
    emit('import', session)
    status.value = 'Shared session loaded.'
  } catch (cause) { if (current === hashRequest) reportError(cause) }
}

onMounted(() => { void loadHash(); window.addEventListener('hashchange', loadHash) })
onBeforeUnmount(() => { hashRequest++; window.removeEventListener('hashchange', loadHash) })
</script>

<template>
  <section class="session-controls" aria-label="Session import and export">
    <div class="actions">
      <button type="button" :disabled="busy" @click="fileInput?.click()">Import</button>
      <button type="button" :disabled="busy" @click="exportSession">Export</button>
      <button type="button" :disabled="busy" @click="copyLink">Copy link</button>
      <input ref="fileInput" type="file" accept=".json,.re3,application/json" hidden @change="importFile" />
    </div>
    <details class="help-details">
      <summary>Sharing help</summary>
      <p class="hint">Save or share the editor, open problems, analysis results, and proved bounds.
        Import replaces the current session; running computations are not resumed.</p>
    </details>
    <p v-if="status" role="status">{{ status }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <label v-if="manualLink">Session link
      <textarea :value="manualLink" readonly rows="3" @focus="($event.target as HTMLTextAreaElement).select()" />
    </label>
  </section>
</template>
