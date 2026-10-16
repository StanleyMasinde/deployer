<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { Label } from "reka-ui";
import { setupSchema, useValidation } from "../validation";
import AuthLayout from "../components/AuthLayout.vue";
import UiInput from "../components/ui/UiInput.vue";
import UiButton from "../components/ui/UiButton.vue";
import { useRouter } from "vue-router";
import { body, request } from "../api/client";

definePage({ meta: { ssg: false, title: "Create your root account · Deployer" } });

const router = useRouter();
const form = reactive({ code: "", name: "", email: "", password: "", confirmation: "" });
const checking = ref(true);
const busy = ref(false);
const error = ref("");
const { errors: fieldErrors, validate } = useValidation();

onMounted(async () => {
  try {
    const setup = await request<{ required: boolean }>("/setup");

    if (!setup.required) await router.replace("/console");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not check setup status.";
  } finally {
    checking.value = false;
  }
});

async function createRoot() {
  error.value = "";

  if (!validate(setupSchema, form)) return;

  busy.value = true;

  try {
    await request("/setup", {
      method: "POST",
      body: body("users", {
        code: form.code.trim(),
        name: form.name.trim(),
        email: form.email.trim(),
        password: form.password,
      }),
    });

    form.password = "";
    form.confirmation = "";
    await router.replace("/console");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not create the account.";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <AuthLayout
    title="Create your root account"
    description="Set up the account that will manage this instance."
  >
    <p v-if="checking" role="status">Checking your instance…</p>
    <form novalidate v-else class="auth-form" @submit.prevent="createRoot">
      <p>Read the one-time setup code on your VPS:</p>
      <pre class="setup-command"><code>sudo cat /var/lib/deployer/state/setup-code</code></pre>
      <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
      <Label for="code">Setup code</Label>
      <UiInput
        id="code"
        :error="fieldErrors['code']"
        v-model="form.code"
        autocomplete="off"
        :disabled="busy"
      />
      <Label for="name">Name</Label>
      <UiInput
        id="name"
        :error="fieldErrors['name']"
        v-model="form.name"
        autocomplete="name"
        :disabled="busy"
      />
      <Label for="email">Email address</Label>
      <UiInput
        id="email"
        :error="fieldErrors['email']"
        v-model="form.email"
        type="email"
        autocomplete="email"
        :disabled="busy"
      />
      <Label for="password">Password</Label>
      <UiInput
        id="password"
        :error="fieldErrors['password']"
        v-model="form.password"
        type="password"
        autocomplete="new-password"
        :disabled="busy"
      />
      <Label for="confirmation">Confirm password</Label>
      <UiInput
        id="confirmation"
        :error="fieldErrors['confirmation']"
        v-model="form.confirmation"
        type="password"
        autocomplete="new-password"
        :disabled="busy"
      />
      <UiButton type="submit" class="primary" :disabled="busy">
        {{ busy ? "Creating account…" : "Create root account" }}
      </UiButton>
    </form>
  </AuthLayout>
</template>
