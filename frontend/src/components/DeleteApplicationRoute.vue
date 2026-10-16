<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  AlertDialogRoot,
  AlertDialogPortal,
  AlertDialogOverlay,
  AlertDialogContent,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogCancel,
  Label,
} from 'reka-ui'
import { deletionSchema, useValidation } from '../validation'
import { request } from '../api/client'
import UiInput from './ui/UiInput.vue'
import UiButton from './ui/UiButton.vue'
const route = useRoute(),
  router = useRouter()
const app = route.meta.applicationData
const confirmation = ref(''),
  busy = ref(false),
  error = ref('')
const { errors, validate } = useValidation()
function close() {
  const parent = route.path.replace(/\/delete$/, '')
  const state = router.options.history.state
  if (state.modalReturn && state.back === state.modalReturn) router.back()
  else void router.replace({ path: parent, query: route.query })
}
async function remove() {
  if (!app || !validate(deletionSchema(app.domain), { confirmation: confirmation.value })) return
  busy.value = true
  try {
    await request(`/apps/${app.id}`, { method: 'DELETE' })
    await router.replace('/console/applications')
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not delete application.'
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <AlertDialogRoot :open="true" @update:open="!$event && close()"
    ><AlertDialogPortal
      ><AlertDialogOverlay class="confirm-overlay" /><AlertDialogContent
        class="console-confirm"
        @escape-key-down="busy && $event.preventDefault()"
      >
        <AlertDialogTitle>Delete application?</AlertDialogTitle
        ><AlertDialogDescription
          >The worker will remove the application, its files, and deployment history. This cannot be
          undone.</AlertDialogDescription
        >
        <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
        <form novalidate @submit.prevent="remove">
          <Label for="delete-confirmation">Type {{ app?.domain }} to confirm</Label
          ><UiInput
            id="delete-confirmation"
            v-model="confirmation"
            :error="errors.confirmation"
            autocomplete="off"
            :disabled="busy"
          />
          <div class="modal-actions">
            <AlertDialogCancel as-child
              ><UiButton :disabled="busy">Cancel</UiButton></AlertDialogCancel
            ><UiButton type="submit" class="danger-button" :disabled="busy">{{
              busy ? 'Deleting…' : 'Delete application'
            }}</UiButton>
          </div>
        </form>
      </AlertDialogContent></AlertDialogPortal
    ></AlertDialogRoot
  >
</template>
