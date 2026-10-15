<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { Label } from "reka-ui";
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

  if (form.password !== form.confirmation) {
    error.value = "Passwords do not match.";
    return;
  }

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
  <main id="main" class="console">
    <p class="eyebrow">FIRST RUN / YOUR INSTANCE</p>
    <h1>Make yourself at home.</h1>
    <p>Create the root account that will manage this VPS.</p>
    <p v-if="checking" role="status">Checking your instance…</p>
    <form v-else class="signin" @submit.prevent="createRoot">
      <h2>Create your root account</h2>
      <p>Read the one-time setup code on your VPS:</p>
      <pre class="setup-command"><code>sudo cat /var/lib/deployer/state/setup-code</code></pre>
      <p v-if="error" class="message error" role="alert">{{ error }}</p>
      <Label for="code">Setup code</Label>
      <UiInput id="code" v-model="form.code" required autocomplete="off" :disabled="busy" />
      <Label for="name">Name</Label>
      <UiInput
        id="name"
        v-model="form.name"
        required
        maxlength="128"
        autocomplete="name"
        :disabled="busy"
      />
      <Label for="email">Email address</Label>
      <UiInput
        id="email"
        v-model="form.email"
        type="email"
        required
        maxlength="254"
        autocomplete="email"
        :disabled="busy"
      />
      <Label for="password">Password</Label>
      <UiInput
        id="password"
        v-model="form.password"
        required
        type="password"
        minlength="12"
        autocomplete="new-password"
        :disabled="busy"
      />
      <Label for="confirmation">Confirm password</Label>
      <UiInput
        id="confirmation"
        v-model="form.confirmation"
        required
        type="password"
        minlength="12"
        autocomplete="new-password"
        :disabled="busy"
      />
      <UiButton type="submit" class="primary" :disabled="busy">
        {{ busy ? "Creating account…" : "Create root account" }}
      </UiButton>
    </form>
  </main>
</template>
