<script setup lang="ts">
import { ref, watch } from "vue";
import {
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
  AlertDialogRoot,
  AlertDialogTrigger,
  AlertDialogPortal,
  AlertDialogOverlay,
  AlertDialogContent,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogCancel,
  Label,
} from "reka-ui";
import { request, type App, type Deployment } from "../api/client";
import UiButton from "./ui/UiButton.vue";
import UiInput from "./ui/UiInput.vue";
import UiIcon from "./ui/UiIcon.vue";
const props = defineProps<{ open: boolean; appId: number | null }>();
const emit = defineEmits<{
  "update:open": [boolean];
  edit: [App];
  changed: [];
  closeAutoFocus: [Event];
}>();
const app = ref<App>();
const deployments = ref<Deployment[]>([]);
const loading = ref(false),
  busy = ref(false),
  error = ref(""),
  confirming = ref(false),
  confirmation = ref("");
let generation = 0;
async function load() {
  const current = ++generation;
  if (!props.open || props.appId === null) return;
  loading.value = true;
  error.value = "";
  try {
    const [record, history] = await Promise.all([
      request<App>(`/apps/${props.appId}`),
      request<Deployment[]>(`/apps/${props.appId}/deployments`),
    ]);
    if (current !== generation) return;
    app.value = record;
    deployments.value = history;
  } catch (cause) {
    if (current === generation)
      error.value = cause instanceof Error ? cause.message : "Could not load application.";
  } finally {
    if (current === generation) loading.value = false;
  }
}
watch(
  () => [props.open, props.appId],
  () => {
    app.value = undefined;
    deployments.value = [];
    confirming.value = false;
    confirmation.value = "";
    void load();
  },
  { immediate: true },
);
async function remove() {
  if (!app.value || confirmation.value !== app.value.domain) return;
  busy.value = true;
  error.value = "";
  try {
    await request(`/apps/${app.value.id}`, { method: "DELETE" });
    confirming.value = false;
    emit("update:open", false);
    emit("changed");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not delete application.";
  } finally {
    busy.value = false;
  }
}
async function workflow() {
  if (!app.value) return;
  busy.value = true;
  error.value = "";
  try {
    app.value = await request<App>(`/apps/${app.value.id}/workflow`, { method: "POST" });
    emit("changed");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not update workflow PR.";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <DialogRoot :open="open" @update:open="emit('update:open', $event)"
    ><DialogPortal
      ><DialogOverlay class="console-dialog-overlay" /><DialogContent
        class="console-drawer application-details"
        @close-auto-focus="emit('closeAutoFocus', $event)"
        @interact-outside="busy && $event.preventDefault()"
        @escape-key-down="busy && $event.preventDefault()"
      >
        <div class="drawer-heading">
          <span class="drawer-icon"><UiIcon name="apps" /></span
          ><DialogClose as-child
            ><UiButton aria-label="Close application details" :disabled="busy"
              ><UiIcon name="close" /></UiButton
          ></DialogClose>
        </div>
        <DialogTitle class="drawer-title">{{ app?.domain ?? "Application details" }}</DialogTitle>
        <DialogDescription class="drawer-description"
          >Configuration and deployment history for this application.</DialogDescription
        >
        <p v-if="error && !confirming" class="console-message is-error" role="alert">{{ error }}</p>
        <p v-if="loading" role="status">Loading application…</p>
        <UiButton v-else-if="!app" @click="load">Try again</UiButton>
        <template v-else>
          <p v-if="app.deletionRequested" class="console-message" role="status">
            Deletion requested. The worker will stop serving this application and remove its files
            and deployment history. If cleanup failed, retry removal below.
          </p>
          <div class="detail-actions">
            <UiButton :disabled="busy || app.deletionRequested" @click="emit('edit', app)"
              >Edit settings</UiButton
            ><a
              class="button"
              :href="`http://${app.domain}`"
              target="_blank"
              rel="noopener noreferrer"
              >Visit site<UiIcon name="arrow"
            /></a>
          </div>
          <section class="detail-section">
            <h2>Source & destination</h2>
            <dl class="detail-values">
              <div>
                <dt>Repository</dt>
                <dd>
                  <a
                    :href="`https://github.com/${app.repository}`"
                    target="_blank"
                    rel="noopener noreferrer"
                    >{{ app.repository }}</a
                  >
                </dd>
              </div>
              <div>
                <dt>Branch</dt>
                <dd>
                  <code>{{ app.branch }}</code>
                </dd>
              </div>
              <div>
                <dt>Domain</dt>
                <dd>{{ app.domain }}</dd>
              </div>
              <div>
                <dt>Routing</dt>
                <dd>{{ app.spa ? "Single-page application" : "Static files" }}</dd>
              </div>
            </dl>
          </section>
          <section class="detail-section">
            <h2>Build configuration</h2>
            <dl class="detail-values">
              <div>
                <dt>Deployment type</dt>
                <dd>Static site</dd>
              </div>
              <div>
                <dt>Ecosystem</dt>
                <dd>
                  {{ app.buildConfig.ecosystem === "none" ? "No build tools" : "JavaScript" }}
                </dd>
              </div>
              <div v-if="app.buildConfig.packageManager">
                <dt>Package manager</dt>
                <dd>{{ app.buildConfig.packageManager }}</dd>
              </div>
              <div>
                <dt>Framework</dt>
                <dd>
                  {{
                    app.buildConfig.framework === "vite" ? "Vite" : "Plain HTML / CSS / JavaScript"
                  }}
                </dd>
              </div>
              <div v-if="app.installCommand">
                <dt>Install command</dt>
                <dd>
                  <code>{{ app.installCommand }}</code>
                </dd>
              </div>
              <div v-if="app.buildCommand">
                <dt>Build command</dt>
                <dd>
                  <code>{{ app.buildCommand }}</code>
                </dd>
              </div>
              <div>
                <dt>Output directory</dt>
                <dd>
                  <code>{{ app.outputDirectory }}</code>
                </dd>
              </div>
            </dl>
            <div class="detail-actions">
              <a
                v-if="app.workflowPr"
                class="workflow-link"
                :href="app.workflowPr"
                target="_blank"
                rel="noopener noreferrer"
                >Open workflow PR<UiIcon name="arrow" /></a
              ><UiButton :disabled="busy || app.deletionRequested" @click="workflow">{{
                busy ? "Working…" : "Update workflow PR"
              }}</UiButton>
            </div>
            <p class="field-hint">
              Review and merge workflow changes on GitHub to apply new build settings.
            </p>
          </section>
          <section class="detail-section">
            <h2>Deployment history</h2>
            <p v-if="!deployments.length" class="field-hint">
              No deployments yet. Merge the workflow PR and push to your connected branch.
            </p>
            <ol v-else class="detail-deployments">
              <li v-for="deployment in deployments" :key="deployment.id">
                <div>
                  <span class="deployment-status" :data-status="deployment.status">{{
                    deployment.status
                  }}</span
                  ><code>{{ deployment.commitSha.slice(0, 7) }}</code
                  ><time :datetime="new Date(deployment.createdAt * 1000).toISOString()">{{
                    new Date(deployment.createdAt * 1000).toLocaleString()
                  }}</time>
                </div>
                <p v-if="deployment.error" class="application-error">{{ deployment.error }}</p>
              </li>
            </ol>
            <p v-if="deployments.length" class="field-hint">
              Showing the latest {{ deployments.length }} deployments (up to 100).
            </p>
          </section>
          <section class="detail-section danger-section">
            <h2>Delete application</h2>
            <p>
              Stop serving this application and remove its published files and deployment history.
              Your GitHub repository and workflow stay intact.
            </p>
            <AlertDialogRoot v-model:open="confirming"
              ><AlertDialogTrigger as-child
                ><UiButton
                  class="danger-button"
                  :disabled="busy"
                  @click="
                    confirmation = '';
                    error = '';
                  "
                  >{{ app.deletionRequested ? "Retry removal" : "Delete application" }}</UiButton
                ></AlertDialogTrigger
              ><AlertDialogPortal
                ><AlertDialogOverlay class="confirm-overlay" /><AlertDialogContent
                  class="console-confirm"
                  @escape-key-down="busy && $event.preventDefault()"
                  ><AlertDialogTitle>Delete application?</AlertDialogTitle
                  ><AlertDialogDescription
                    >The worker will remove the application, its files, and deployment history. This
                    cannot be undone.</AlertDialogDescription
                  >
                  <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
                  <form @submit.prevent="remove">
                    <Label for="delete-confirmation">Type {{ app.domain }} to confirm</Label
                    ><UiInput
                      id="delete-confirmation"
                      v-model="confirmation"
                      autocomplete="off"
                      :disabled="busy"
                    />
                    <div class="drawer-actions">
                      <AlertDialogCancel as-child
                        ><UiButton :disabled="busy">Cancel</UiButton></AlertDialogCancel
                      ><UiButton
                        type="submit"
                        class="danger-button"
                        :disabled="busy || confirmation !== app.domain"
                        >{{ busy ? "Deleting…" : "Delete application" }}</UiButton
                      >
                    </div>
                  </form></AlertDialogContent
                ></AlertDialogPortal
              ></AlertDialogRoot
            >
          </section>
        </template>
      </DialogContent></DialogPortal
    ></DialogRoot
  >
</template>
