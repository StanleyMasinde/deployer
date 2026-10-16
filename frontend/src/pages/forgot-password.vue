<script setup lang="ts">
import { ref } from 'vue'
import { Label } from 'reka-ui'
import AuthLayout from '../components/AuthLayout.vue'
import UiInput from '../components/ui/UiInput.vue'
import UiButton from '../components/ui/UiButton.vue'
import { forgotPasswordSchema, useValidation } from '../validation'
import { request, body } from '../api/client'
definePage({ meta: { auth: true, ssg: false, title: 'Forgot password · Deployer' } })
const email = ref(''),
  busy = ref(false),
  sent = ref(false),
  error = ref('')
const { errors, validate } = useValidation()
async function submit() {
  if (!validate(forgotPasswordSchema, { email: email.value })) return
  busy.value = true
  error.value = ''
  try {
    await request('/password-resets', {
      method: 'POST',
      body: body('password-resets', { email: email.value.trim() }),
    })
    sent.value = true
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not request a reset. Try again.'
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <AuthLayout
    title="Reset your password"
    description="Enter the email address for your owner account."
  >
    <div v-if="sent" class="auth-result" role="status">
      <h2>Check your email</h2>
      <p>If an account matches that address, a reset link will be emailed shortly.</p>
      <p>
        The link expires in 30 minutes. If it does not arrive, check spam or contact the server
        administrator.
      </p>
    </div>
    <form v-else novalidate class="auth-form" @submit.prevent="submit">
      <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
      <Label for="reset-email">Email address</Label
      ><UiInput
        id="reset-email"
        v-model="email"
        :error="errors.email"
        type="email"
        autocomplete="email"
        :disabled="busy"
      /><UiButton type="submit" class="primary" :disabled="busy">{{
        busy ? 'Requesting…' : 'Send reset link'
      }}</UiButton>
    </form>
    <RouterLink class="auth-secondary-link" to="/console">Back to sign in</RouterLink>
  </AuthLayout>
</template>
