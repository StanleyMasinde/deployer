<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Label } from 'reka-ui'
import AuthLayout from '../components/AuthLayout.vue'
import UiInput from '../components/ui/UiInput.vue'
import UiButton from '../components/ui/UiButton.vue'
import { resetPasswordSchema, useValidation } from '../validation'
import { request, body } from '../api/client'
import { stopLiveUpdates } from '../live'
definePage({ meta: { auth: true, ssg: false, title: 'Choose a password · Deployer' } })
const route = useRoute(),
  router = useRouter()
const token = computed(() => new URLSearchParams(route.hash.slice(1)).get('token') ?? '')
const form = reactive({ password: '', confirmation: '' })
const busy = ref(false),
  done = ref(false),
  error = ref('')
const { errors, validate } = useValidation()
async function submit() {
  const data = { ...form, token: token.value }
  if (!validate(resetPasswordSchema, data)) return
  busy.value = true
  error.value = ''
  try {
    await request('/password-resets/current', {
      method: 'PATCH',
      body: body('password-resets', data),
    })
    form.password = ''
    form.confirmation = ''
    done.value = true
    stopLiveUpdates()
    await router.replace({ hash: '' })
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not reset the password.'
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <AuthLayout
    :title="done ? 'Password updated' : 'Choose a new password'"
    :description="
      done
        ? 'Your previous sessions have been signed out.'
        : 'Use a unique password with at least 15 characters.'
    "
  >
    <div v-if="done" class="auth-result" role="status">
      <p>Your password has been changed. Sign in with your new password to continue.</p>
      <RouterLink class="button primary" to="/console">Back to sign in</RouterLink>
    </div>
    <div v-else-if="!token" class="auth-result">
      <p>This link is missing a reset token.</p>
      <RouterLink to="/forgot-password">Request a new reset link</RouterLink>
    </div>
    <form v-else novalidate class="auth-form" @submit.prevent="submit">
      <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
      <p v-if="errors.token" class="field-error" role="alert">{{ errors.token }}</p>
      <Label for="new-password">New password</Label
      ><UiInput
        id="new-password"
        v-model="form.password"
        :error="errors.password"
        type="password"
        autocomplete="new-password"
        :disabled="busy"
      /><Label for="confirm-password">Confirm password</Label
      ><UiInput
        id="confirm-password"
        v-model="form.confirmation"
        :error="errors.confirmation"
        type="password"
        autocomplete="new-password"
        :disabled="busy"
      /><UiButton type="submit" class="primary" :disabled="busy">{{
        busy ? 'Updating password…' : 'Update password'
      }}</UiButton
      ><RouterLink class="auth-secondary-link" to="/forgot-password"
        >Request a new reset link</RouterLink
      >
    </form>
  </AuthLayout>
</template>
