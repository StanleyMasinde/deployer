<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from 'reka-ui'
const route = useRoute(),
  router = useRouter()
const tab = computed(() => (route.query.tab === 'deployments' ? 'deployments' : 'overview'))
function changeTab(value: string | number) {
  void router.push({
    query: { ...route.query, tab: value === 'overview' ? undefined : String(value) },
  })
}
import { httpsStatus, appUrl } from '../https'
import { request, type App, type Deployment } from '../api/client'
import UiButton from './ui/UiButton.vue'
import UiIcon from './ui/UiIcon.vue'
const props = defineProps<{
  open: boolean
  appId: number | null
  initialApp?: App
  initialDeployments?: Deployment[]
}>()
const emit = defineEmits<{
  'update:open': [boolean]
  edit: [App]
  changed: []
  closeAutoFocus: [Event]
}>()
const app = ref<App | undefined>(props.initialApp)
const deployments = ref<Deployment[]>(props.initialDeployments ?? [])
const loadingOlder = ref(false)
const hasOlder = ref((props.initialDeployments?.length ?? 0) === 100)
function mergeHistory(records: Deployment[]) {
  const history = new Map(deployments.value.map((d) => [d.id, d]))
  for (const record of records) history.set(record.id, record)
  deployments.value = [...history.values()].sort((a, b) => b.id - a.id)
}
async function loadOlder() {
  if (!app.value || loadingOlder.value) return
  loadingOlder.value = true
  error.value = ''
  try {
    const records = await request<Deployment[]>(
      `/apps/${app.value.id}/deployments?before=${Math.min(...deployments.value.map((d) => d.id))}`,
    )
    mergeHistory(records)
    hasOlder.value = records.length === 100
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not load older deployments.'
  } finally {
    loadingOlder.value = false
  }
}
const loading = ref(false),
  busy = ref(false),
  error = ref('')
const certificateStatus = computed(() =>
  app.value ? httpsStatus(app.value.domain, app.value.https) : 'disabled',
)
let poll: ReturnType<typeof setInterval> | undefined
watch(
  () => props.open,
  (open) => {
    clearInterval(poll)
    if (open)
      poll = setInterval(async () => {
        if (busy.value || !app.value) return
        const id = props.appId
        try {
          const record = await request<App>(`/apps/${id}`)
          if (props.open && props.appId === id) app.value = record
        } catch {
          /* A manual refresh remains available. */
        }
      }, 5000)
  },
  { immediate: true },
)
onUnmounted(() => {
  clearInterval(poll)
  generation++
})
async function enableHttps() {
  if (!app.value) return
  busy.value = true
  error.value = ''
  try {
    app.value = await request<App>(`/apps/${app.value.id}/https`, { method: 'POST' })
    emit('changed')
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not request HTTPS.'
  } finally {
    busy.value = false
  }
}
let generation = 0
async function load() {
  const current = ++generation
  if (!props.open || props.appId === null) return
  loading.value = true
  error.value = ''
  try {
    const [record, history] = await Promise.all([
      request<App>(`/apps/${props.appId}`),
      request<Deployment[]>(`/apps/${props.appId}/deployments`),
    ])
    if (current !== generation) return
    app.value = record
    mergeHistory(history)
  } catch (cause) {
    if (current === generation)
      error.value = cause instanceof Error ? cause.message : 'Could not load application.'
  } finally {
    if (current === generation) loading.value = false
  }
}
watch(
  () => [props.initialApp, props.initialDeployments],
  () => {
    app.value = props.initialApp
    mergeHistory(props.initialDeployments ?? [])
  },
)
async function workflow() {
  if (!app.value) return
  busy.value = true
  error.value = ''
  try {
    app.value = await request<App>(`/apps/${app.value.id}/workflow`, { method: 'POST' })
    emit('changed')
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not update workflow PR.'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="application-detail-page">
    <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
    <p v-if="loading" role="status">Loading application…</p>
    <UiButton v-else-if="!app" @click="load">Try again</UiButton>
    <template v-else>
      <p v-if="app.deletionRequested" class="console-message" role="status">
        Deletion requested. The worker will stop serving this application and remove its files and
        deployment history. If cleanup failed, retry removal below.
      </p>
      <div class="detail-actions">
        <UiButton :disabled="busy || loading" @click="load"
          ><UiIcon name="refresh" />Refresh details</UiButton
        >
        <UiButton :disabled="busy || app.deletionRequested" @click="emit('edit', app)"
          >Edit settings</UiButton
        ><a class="button" :href="appUrl(app)" target="_blank" rel="noopener noreferrer"
          >Visit site<UiIcon name="arrow"
        /></a>
      </div>
      <TabsRoot :model-value="tab" @update:model-value="changeTab" class="application-tabs-root">
        <TabsList class="application-tabs" aria-label="Application sections"
          ><TabsTrigger value="overview">Overview</TabsTrigger
          ><TabsTrigger value="deployments"
            >Deployments <span>{{ deployments.length }}{{ hasOlder ? '+' : '' }}</span></TabsTrigger
          ></TabsList
        >
        <TabsContent value="overview" class="application-detail-grid">
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
                <dd>{{ app.spa ? 'Single-page application' : 'Static files' }}</dd>
              </div>
            </dl>
          </section>
          <section class="detail-section">
            <h2>HTTPS</h2>
            <p role="status">
              <span
                class="deployment-status"
                :data-status="
                  certificateStatus === 'active'
                    ? 'live'
                    : certificateStatus === 'failed' || certificateStatus === 'expired'
                      ? 'failed'
                      : 'queued'
                "
                >{{
                  {
                    disabled: 'Not enabled',
                    pending: 'Certificate pending',
                    active: 'HTTPS active',
                    failed: 'Certificate failed',
                    expired: 'Certificate expired',
                  }[certificateStatus]
                }}</span
              >
            </p>
            <p v-if="app.https?.expiresAt && app.https.domain === app.domain" class="field-hint">
              Certificate expires {{ new Date(app.https.expiresAt * 1000).toLocaleString() }}.
              Renewal is automatic.
            </p>
            <p v-if="app.https?.error" class="console-message is-error" role="alert">
              {{ app.https.error }}
            </p>
            <p v-if="certificateStatus === 'disabled'" class="field-hint">
              Point this domain’s DNS at the VPS and allow incoming traffic on ports 80 and 443.
              Deployer obtains and renews a Let’s Encrypt certificate.
            </p>
            <p v-if="certificateStatus === 'pending'" class="field-hint">
              The worker is requesting your certificate. This view updates automatically.
            </p>
            <UiButton
              v-if="certificateStatus !== 'active' || app.https?.error"
              :disabled="busy || app.deletionRequested || certificateStatus === 'pending'"
              @click="enableHttps"
              >{{
                busy
                  ? 'Requesting…'
                  : certificateStatus === 'disabled'
                    ? 'Enable HTTPS'
                    : 'Retry certificate'
              }}</UiButton
            >
          </section>
          <section class="detail-section">
            <h2>Build configuration</h2>
            <p
              v-if="app.workflowPr"
              class="workflow-state"
              :data-status="app.workflowPrState ?? 'unknown'"
            >
              <UiIcon :name="app.workflowPrState === 'merged' ? 'check' : 'repo'" />{{
                app.workflowPrState === 'merged'
                  ? 'Workflow PR merged'
                  : app.workflowPrState === 'closed'
                    ? 'Workflow PR closed without merging'
                    : app.workflowPrState === 'open'
                      ? 'Workflow PR open — awaiting merge'
                      : 'Workflow PR status has not been received yet'
              }}
            </p>

            <dl class="detail-values">
              <div>
                <dt>Deployment type</dt>
                <dd>Static site</dd>
              </div>
              <div>
                <dt>Ecosystem</dt>
                <dd>
                  {{ app.buildConfig.ecosystem === 'none' ? 'No build tools' : 'JavaScript' }}
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
                    app.buildConfig.framework === 'vite' ? 'Vite' : 'Plain HTML / CSS / JavaScript'
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
                >{{
                  app.workflowPrState === 'merged'
                    ? 'View merged PR'
                    : app.workflowPrState === 'closed'
                      ? 'View closed PR'
                      : 'Open workflow PR'
                }}<UiIcon name="arrow" /></a
              ><UiButton :disabled="busy || app.deletionRequested" @click="workflow">{{
                busy ? 'Working…' : 'Update workflow PR'
              }}</UiButton>
            </div>
            <p class="field-hint">
              Review and merge workflow changes on GitHub to apply new build settings.
            </p>
          </section>
          <section class="detail-section danger-section">
            <h2>Delete application</h2>
            <p>
              Stop serving this application and remove its published files and deployment history.
              Your GitHub repository and workflow stay intact.
            </p>
            <RouterLink
              class="button danger-button"
              :to="{
                path: `/console/applications/${app.id}/delete`,
                query: route.query,
                state: { modalReturn: route.fullPath },
              }"
              >{{ app.deletionRequested ? 'Retry removal' : 'Delete application' }}</RouterLink
            >
          </section></TabsContent
        >
        <TabsContent value="deployments" class="application-detail-grid">
          <section class="detail-section">
            <h2>Deployments</h2>
            <p v-if="!deployments.length" class="field-hint">
              No deployments yet. Merge the workflow PR and push to your connected branch.
            </p>
            <ol v-else class="detail-deployments">
              <li v-for="deployment in deployments" :key="deployment.id">
                <div>
                  <span class="deployment-status" :data-status="deployment.status">{{
                    deployment.status
                  }}</span
                  ><a
                    :href="`https://github.com/${app.repository}/commit/${deployment.commitSha}`"
                    target="_blank"
                    rel="noopener noreferrer"
                    :aria-label="`View commit ${deployment.commitSha.slice(0, 7)} on GitHub`"
                    ><code>{{ deployment.commitSha.slice(0, 7) }}</code></a
                  ><RouterLink :to="`/console/deployments/${deployment.id}`"
                    >Run #{{ deployment.runId }}</RouterLink
                  ><time :datetime="new Date(deployment.createdAt * 1000).toISOString()">{{
                    new Date(deployment.createdAt * 1000).toLocaleString()
                  }}</time>
                </div>
                <p v-if="deployment.error" class="application-error">{{ deployment.error }}</p>
              </li>
            </ol>
            <p v-if="deployments.length" class="field-hint">
              Showing {{ deployments.length }} deployments.
            </p>
            <UiButton v-if="hasOlder" :disabled="loadingOlder" @click="loadOlder">{{
              loadingOlder ? 'Loading…' : 'Load older deployments'
            }}</UiButton>
          </section>
        </TabsContent>
      </TabsRoot>
    </template>
  </section>
</template>
