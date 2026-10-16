<script setup lang="ts">
function clearFilters() {
  void router.replace({ query: { ...route.query, q: undefined, status: undefined } })
}

import BrandMark from './BrandMark.vue'
import AccountMenu from './AccountMenu.vue'
import ProfileDetails from './ProfileDetails.vue'
import type { ConsoleData } from '../router/data'
import { ref, reactive, computed, watch, watchEffect, provide, onMounted, onUnmounted } from 'vue'
import { buildPreset, type Ecosystem, type PackageManager } from '../build'
import { signInSchema, applicationSchema, useValidation } from '../validation'
import { applicationFormKey } from './applicationFormContext'
import DeploymentDetails from './DeploymentDetails.vue'
import PlannedPage from '../components/PlannedPage.vue'
import ConsoleNavigation from '../components/ConsoleNavigation.vue'
import ConsoleDashboard from '../components/ConsoleDashboard.vue'
import AuthLayout from '../components/AuthLayout.vue'
import AppDetails from '../components/AppDetails.vue'
import UiSelect from '../components/ui/UiSelect.vue'
import { Label } from 'reka-ui'
import UiIcon from '../components/ui/UiIcon.vue'
import '../console.css'
import '../workspace.css'
import UiInput from '../components/ui/UiInput.vue'
import UiButton from '../components/ui/UiButton.vue'
import { useRouter, useRoute, type RouteLocationNormalizedLoadedGeneric } from 'vue-router'
import { ApiError, request, body, type App, type Deployment, type Repository } from '../api/client'

const router = useRouter()
const serverHostname = typeof location === 'undefined' ? 'Deployer' : location.hostname
const route: RouteLocationNormalizedLoadedGeneric = useRoute()
const view = computed(
  () =>
    ({
      applications: 'apps',
      deployments: 'activity',
      profile: 'profile',
      personal: 'personal',
      runtimes: 'runtimes',
      resources: 'resources',
      services: 'services',
      previews: 'previews',
      domains: 'domains',
      settings: 'settings',
    })[route.path.split('/')[2] ?? ''] ?? 'dashboard',
)
const pageTitle = computed(
  () =>
    ({
      apps: 'Applications',
      activity: 'Deployments',
      dashboard: 'Dashboard',
      profile: 'Profile',
      personal: 'Personal settings',
      runtimes: 'Runtimes & tools',
      resources: 'Resource usage',
      services: 'Services',
      previews: 'PR previews',
      domains: 'Domains',
      settings: 'Settings',
    })[view.value] ?? 'Dashboard',
)
const deploymentPage = computed(() => Boolean(route.params.deploymentId))
const application = computed(() => (route.params.appId ? route.meta.applicationData : undefined))
const applicationPage = computed(() => Boolean(route.params.appId))
const planned = computed(() =>
  ['personal', 'runtimes', 'resources', 'services', 'previews', 'domains', 'settings'].includes(
    view.value,
  ),
)
const pageDescription = computed(
  () =>
    ({
      dashboard: 'Your applications and the latest release activity.',
      apps: 'Manage repositories, domains, and releases.',
      activity: 'Track release progress and deployment history.',
      profile: 'Your owner account.',
      personal: 'Account security and personal preferences.',
      runtimes: 'Application runtimes, package managers, and their versions.',
      resources: 'Server capacity and per-service resource consumption.',
      services: 'Manage native applications running under systemd.',
      previews: 'Temporary environments for pull requests.',
      domains: 'Application addresses, custom domains, and HTTPS.',
      settings: 'Instance configuration and GitHub connections.',
    })[view.value],
)
const search = computed({
  get: () => (typeof route.query.q === 'string' ? route.query.q : ''),
  set: (value) => {
    void router.replace({ query: { ...route.query, q: value || undefined } })
  },
})
const filter = computed({
  get: () =>
    typeof route.query.status === 'string' &&
    ['live', 'building', 'deploying', 'queued', 'failed', 'attention'].includes(route.query.status)
      ? route.query.status
      : 'all',
  set: (value) => {
    void router.replace({ query: { ...route.query, status: value === 'all' ? undefined : value } })
  },
})
const refreshing = ref(false)
const lastUpdated = ref<Date>(new Date())
const authenticated = ref(route.meta.consoleData?.authenticated ?? false)
const checking = ref(false)
watchEffect(() => {
  if (typeof document !== 'undefined')
    document.title = authenticated.value
      ? route.meta.modal
        ? (route.meta.title ?? 'Deployer')
        : `${applicationPage.value ? (application.value?.domain ?? 'Application') : deploymentPage.value ? `Deployment #${route.params.deploymentId}` : pageTitle.value} · Deployer`
      : 'Sign in · Deployer'
})
const busy = ref(false)
const error = ref(route.meta.consoleData?.error ?? '')
const notice = ref('')
const password = ref('')
const email = ref('')
const { errors: fieldErrors, validate } = useValidation()
const apps = ref<App[]>(route.meta.consoleData?.apps ?? [])
const deployments = ref<Deployment[]>(route.meta.consoleData?.deployments ?? [])
const repositories = ref<Repository[]>([])
const adding = computed(() => route.meta.modal === 'new' || route.meta.modal === 'edit')
const editing = ref<number | null>(null)
const loadingRepositories = ref(false)
const form = reactive({
  repository: '',
  branch: 'main',
  domain: '',
  ...buildPreset('none'),
  spa: false,
})
const repositoryOptions = computed(() => {
  const options = repositories.value.map((repo) => ({
    value: repo.fullName,
    label: repo.fullName,
  }))
  if (
    editing.value !== null &&
    form.repository &&
    !options.some((option) => option.value === form.repository)
  )
    options.unshift({ value: form.repository, label: form.repository })
  return options
})
let timer: ReturnType<typeof setTimeout> | undefined
let stopped = false
async function poll() {
  if (stopped || !authenticated.value) return
  await refresh()
  if (!stopped && authenticated.value) timer = setTimeout(poll, 5000)
}
onMounted(() => {
  if (authenticated.value) timer = setTimeout(poll, 5000)
})
onUnmounted(() => {
  stopped = true
  clearTimeout(timer)
})
let formTrigger: HTMLElement | null = null

async function refresh() {
  try {
    const [nextApps, nextDeployments] = await Promise.all([
      request<App[]>('/apps'),
      request<Deployment[]>('/deployments'),
    ])

    apps.value = nextApps
    deployments.value = nextDeployments
    lastUpdated.value = new Date()
    route.meta.consoleData = {
      ...route.meta.consoleData,
      authenticated: true,
      apps: nextApps,
      deployments: nextDeployments,
      error: '',
    }
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 401) authenticated.value = false
    error.value = cause instanceof Error ? cause.message : 'Could not refresh deployments.'
  }
}

async function signIn() {
  if (!validate(signInSchema, { email: email.value, password: password.value })) return
  busy.value = true
  error.value = ''

  try {
    await request('/session', {
      method: 'POST',
      body: body('sessions', { email: email.value.trim(), password: password.value }),
    })
    password.value = ''
    const session = await request<{ user?: ConsoleData['user'] }>('/session')
    authenticated.value = true
    route.meta.consoleData = {
      authenticated: true,
      user: session.user,
      apps: apps.value,
      deployments: deployments.value,
      error: '',
    }
    await poll()
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not sign in.'
  } finally {
    busy.value = false
  }
}

async function signOut() {
  try {
    await request('/session', { method: 'DELETE' })
    authenticated.value = false
    apps.value = []
    deployments.value = []
    route.meta.consoleData = { authenticated: false, apps: [], deployments: [], error: '' }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not sign out.'
  }
}

function openDetails(app: App) {
  void router.push(`/console/applications/${app.id}`)
}
function openForm(app?: App) {
  formTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null
  void router.push({
    path: app ? `/console/applications/${app.id}/edit` : '/console/applications/new',
    query: route.query,
    state: { modalReturn: route.fullPath },
  })
}
function closeModal() {
  const parent = route.path.replace(/\/(new|edit)$/, '')
  const state = router.options.history.state
  if (state.modalReturn && state.back === state.modalReturn) router.back()
  else void router.replace({ path: parent, query: route.query })
}
watch(
  () => route.fullPath,
  () => {
    const snapshot = route.meta.consoleData
    if (snapshot) {
      authenticated.value = snapshot.authenticated
      apps.value = snapshot.apps
      deployments.value = snapshot.deployments
      error.value = snapshot.error
      lastUpdated.value = new Date()
    }
    if (!adding.value) return
    fieldErrors.value = {}
    const app = route.meta.modal === 'edit' ? route.meta.applicationData : undefined
    editing.value = app?.id ?? null
    Object.assign(
      form,
      app
        ? { ...app, buildConfig: { ...app.buildConfig } }
        : { repository: '', branch: 'main', domain: '', ...buildPreset('none'), spa: false },
    )
    repositories.value = route.meta.repositories ?? []
  },
  { immediate: true },
)
function restoreFormFocus(event: Event) {
  if (formTrigger?.isConnected) {
    event.preventDefault()
    formTrigger.focus({ preventScroll: true })
  }
}

function changeEcosystem(ecosystem: string) {
  Object.assign(form, buildPreset(ecosystem as Ecosystem))
}

function changePackageManager(packageManager: string) {
  const preset = buildPreset('javascript', packageManager as PackageManager)
  form.buildConfig = preset.buildConfig
  form.installCommand = preset.installCommand
  form.buildCommand = preset.buildCommand
}

function chooseRepository() {
  form.branch =
    repositories.value.find((repo) => repo.fullName === form.repository)?.defaultBranch ?? 'main'
}

async function addApp() {
  if (!validate(applicationSchema, form)) return
  busy.value = true
  error.value = ''
  notice.value = ''

  try {
    await request(editing.value === null ? '/apps' : `/apps/${editing.value}`, {
      method: editing.value === null ? 'POST' : 'PUT',
      body: body('apps', form),
    })
    await router.replace(
      editing.value === null ? '/console/applications' : `/console/applications/${editing.value}`,
    )
    notice.value =
      editing.value === null
        ? 'Workflow PR opened. Review and merge it on GitHub to enable deployments.'
        : 'Settings saved. Domain and routing updates are queued for the worker. Update the workflow PR to apply build changes.'
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not add the app.'
  } finally {
    await refresh()
    busy.value = false
  }
}

async function retryWorkflow(app: App) {
  busy.value = true

  try {
    await request(`/apps/${app.id}/workflow`, { method: 'POST' })
    await refresh()
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Could not open the workflow PR.'
  } finally {
    busy.value = false
  }
}

function latest(app: App) {
  return deployments.value
    .filter((d) => d.appId === app.id)
    .sort((a, b) => b.createdAt - a.createdAt || b.id - a.id)[0]
}
function matchesStatus(status: string | undefined) {
  return (
    filter.value === 'all' ||
    (filter.value === 'attention' ? status === 'failed' || !status : status === filter.value)
  )
}
const filteredApps = computed(() =>
  apps.value.filter((app) => {
    const query = search.value.toLowerCase()
    return (
      `${app.domain} ${app.repository} ${app.branch}`.toLowerCase().includes(query) &&
      matchesStatus(latest(app)?.status)
    )
  }),
)
const sortedDeployments = computed(() =>
  [...deployments.value].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
)
const filteredDeployments = computed(() =>
  sortedDeployments.value.filter((deployment) => {
    const app = apps.value.find((a) => a.id === deployment.appId)
    return (
      `${app?.domain ?? ''} ${app?.repository ?? ''} ${deployment.commitSha}`
        .toLowerCase()
        .includes(search.value.toLowerCase()) && matchesStatus(deployment.status)
    )
  }),
)
function statusLabel(status?: string) {
  return (
    (
      {
        live: 'Live',
        failed: 'Failed',
        building: 'Building',
        queued: 'Queued',
        deploying: 'Deploying',
        superseded: 'Superseded',
      } as Record<string, string>
    )[status ?? ''] ??
    status ??
    'Awaiting build'
  )
}
function dateLabel(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}
function commitUrl(deployment: Deployment) {
  const app = apps.value.find((app) => app.id === deployment.appId)
  return app ? `https://github.com/${app.repository}/commit/${deployment.commitSha}` : undefined
}
function appName(id: number) {
  return apps.value.find((app) => app.id === id)?.domain ?? `App ${id}`
}
async function manualRefresh() {
  refreshing.value = true
  error.value = ''
  try {
    await refresh()
  } finally {
    refreshing.value = false
  }
}
function changeView(value: string | number) {
  void router.push(
    value === 'apps'
      ? '/console/applications'
      : value === 'activity'
        ? '/console/deployments'
        : '/console',
  )
}

provide(applicationFormKey, {
  form,
  fieldErrors,
  editing,
  busy,
  error,
  loadingRepositories,
  repositoryOptions,
  repositories,
  addApp,
  chooseRepository,
  changeEcosystem,
  changePackageManager,
  closeModal,
  restoreFormFocus,
})
</script>

<template>
  <AuthLayout
    v-if="!checking && !authenticated"
    title="Sign in to Deployer"
    description="Use your root account to manage this instance."
  >
    <form novalidate class="auth-form" @submit.prevent="signIn">
      <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
      <Label for="email">Email address</Label
      ><UiInput
        id="email"
        :error="fieldErrors['email']"
        v-model="email"
        type="email"
        autocomplete="username"
        :disabled="busy"
      /><Label for="password">Password</Label
      ><UiInput
        id="password"
        :error="fieldErrors['password']"
        v-model="password"
        type="password"
        autocomplete="current-password"
        :disabled="busy"
      /><UiButton type="submit" class="primary" :disabled="busy"
        >{{ busy ? 'Signing in…' : 'Sign in' }}<UiIcon name="arrow"
      /></UiButton>
    </form>
  </AuthLayout>
  <div v-else class="deployment-console console-app">
    <aside class="console-sidebar">
      <RouterLink class="console-brand" to="/" aria-label="Deployer home"
        ><BrandMark />Deployer</RouterLink
      >
      <div class="sidebar-instance">
        <UiIcon name="server" />
        <div>
          <strong>{{ serverHostname }}</strong
          ><span>Deployment workspace</span>
        </div>
      </div>
      <p class="sidebar-section-label">WORKSPACE</p>
      <ConsoleNavigation :active="view" />
      <div class="sidebar-footnote"><UiIcon name="server" />Self-hosted instance</div>
    </aside>

    <div class="console-workspace">
      <header class="console-topbar">
        <div class="topbar-context">
          <UiIcon name="server" /><span>{{ serverHostname }}</span
          ><span aria-hidden="true">/</span><strong>{{ pageTitle }}</strong>
        </div>
        <AccountMenu :user="route.meta.consoleData?.user" :busy="busy" @sign-out="signOut" />
      </header>
      <main id="main" class="workspace-main">
        <div class="workspace-heading">
          <div>
            <p class="page-eyebrow"><UiIcon name="server" /> INSTANCE CONSOLE</p>
            <RouterLink v-if="applicationPage" class="application-back" to="/console/applications"
              >← Applications</RouterLink
            >
            <h1>
              {{
                applicationPage
                  ? (application?.domain ?? 'Application')
                  : deploymentPage
                    ? `Deployment #${route.params.deploymentId}`
                    : pageTitle
              }}
            </h1>
            <p>
              {{
                applicationPage
                  ? 'Application configuration and deployment history.'
                  : pageDescription
              }}
            </p>
          </div>
          <UiButton
            v-if="
              authenticated && !planned && view !== 'profile' && !applicationPage && !deploymentPage
            "
            class="primary"
            :disabled="busy"
            @click="openForm()"
            ><UiIcon name="plus" />New application</UiButton
          >
        </div>
        <p v-if="error && !adding" class="console-message is-error" role="alert">{{ error }}</p>
        <p v-if="notice" class="console-message" role="status">
          <UiIcon name="check" />{{ notice }}
        </p>

        <div v-if="checking" class="workspace-loading" role="status">
          <span class="loading-line"></span><span class="loading-line"></span>
          <p>Loading your workspace…</p>
        </div>
        <template v-else>
          <ConsoleDashboard
            v-if="view === 'dashboard'"
            :apps="apps"
            :deployments="deployments"
            @details="openDetails"
            @create="openForm()"
          />
          <AppDetails
            v-else-if="applicationPage"
            :key="String(route.params.appId)"
            :open="true"
            :app-id="Number(route.params.appId)"
            :initial-app="route.meta.applicationData"
            :initial-deployments="route.meta.applicationDeployments"
            @edit="openForm"
            @changed="refresh"
          />
          <DeploymentDetails
            v-else-if="deploymentPage"
            :deployment="route.meta.deploymentData"
            :app="route.meta.applicationData"
          />
          <ProfileDetails v-else-if="view === 'profile'" :user="route.meta.consoleData?.user" />
          <PlannedPage v-else-if="planned" :area="view" />
          <section
            v-else
            class="inventory"
            :aria-label="view === 'apps' ? 'Applications' : 'Deployment activity'"
          >
            <div class="inventory-toolbar">
              <div class="inventory-search">
                <UiIcon name="search" /><UiInput
                  id="workspace-search"
                  v-model="search"
                  type="search"
                  :aria-label="view === 'apps' ? 'Search applications' : 'Search deployments'"
                  :placeholder="view === 'apps' ? 'Search applications…' : 'Search deployments…'"
                />
              </div>
              <div class="inventory-tools">
                <UiSelect
                  id="status-filter"
                  v-model="filter"
                  aria-label="Filter by status"
                  :options="[
                    { value: 'all', label: 'All statuses' },
                    { value: 'live', label: 'Live' },
                    { value: 'failed', label: 'Failed' },
                    { value: 'building', label: 'Building' },
                    { value: 'deploying', label: 'Deploying' },
                    { value: 'queued', label: 'Queued' },
                    { value: 'attention', label: 'Needs attention' },
                  ]"
                /><UiButton
                  class="refresh-button"
                  :disabled="refreshing"
                  aria-label="Refresh workspace"
                  @click="manualRefresh"
                  ><UiIcon name="refresh"
                /></UiButton>
              </div>
            </div>

            <template v-if="view === 'apps'">
              <div v-if="!apps.length" class="inventory-empty">
                <span class="empty-app-icon"><UiIcon name="apps" /></span>
                <h2>Your next launch starts here.</h2>
                <p>
                  Connect a GitHub repository. Deployer opens a build workflow PR and brings the
                  finished app to your server.
                </p>
                <UiButton class="primary" :disabled="busy" @click="openForm()"
                  ><UiIcon name="plus" />Connect repository</UiButton
                ><span class="empty-footnote">Your code stays on GitHub. Your app lives here.</span>
              </div>
              <div v-else-if="!filteredApps.length" class="inventory-empty">
                <UiIcon name="search" />
                <h2>No matching applications</h2>
                <p>Try another name or show all statuses.</p>
                <UiButton @click="clearFilters">Clear filters</UiButton>
              </div>
              <div v-else class="application-list">
                <div class="application-columns" aria-hidden="true">
                  <span>Application</span><span>Latest deployment</span><span>Workflow</span>
                </div>
                <article v-for="app in filteredApps" :key="app.id" class="application-item">
                  <div class="application-identity">
                    <span class="application-avatar" aria-hidden="true">{{
                      app.domain.charAt(0).toUpperCase()
                    }}</span>
                    <div>
                      <h2>
                        <RouterLink :to="`/console/applications/${app.id}`"
                          >{{ app.domain }}<UiIcon name="arrow"
                        /></RouterLink>
                      </h2>
                      <a
                        class="repository-link"
                        :href="`https://github.com/${app.repository}`"
                        target="_blank"
                        rel="noopener noreferrer"
                        ><UiIcon name="repo" />{{ app.repository }}</a
                      >
                      <div class="application-meta">
                        <span><UiIcon name="branch" />{{ app.branch }}</span
                        ><span>{{ app.spa ? 'SPA' : 'Static site' }}</span>
                      </div>
                    </div>
                  </div>
                  <div class="application-deployment">
                    <span
                      class="deployment-status"
                      :data-status="app.deletionRequested ? 'deleting' : latest(app)?.status"
                      ><span class="state-dot"></span
                      >{{
                        app.deletionRequested ? 'Deleting' : statusLabel(latest(app)?.status)
                      }}</span
                    ><span v-if="latest(app)" class="deployment-stamp"
                      ><a
                        :href="commitUrl(latest(app)!)"
                        target="_blank"
                        rel="noopener noreferrer"
                        :aria-label="`View commit ${latest(app)!.commitSha.slice(0, 7)} on GitHub`"
                        ><code>{{ latest(app)!.commitSha.slice(0, 7) }}</code></a
                      ><time :datetime="new Date(latest(app)!.createdAt * 1000).toISOString()">{{
                        dateLabel(latest(app)!.createdAt)
                      }}</time></span
                    ><span v-else class="deployment-stamp">{{
                      app.workflowPrState === 'merged'
                        ? 'Workflow merged. Waiting for a build.'
                        : app.workflowPrState === 'closed'
                          ? 'Workflow PR closed without merging.'
                          : 'Merge your workflow PR to begin'
                    }}</span>
                  </div>
                  <div class="application-action">
                    <UiButton class="details-button" @click="openDetails(app)">Details</UiButton>
                    <a
                      v-if="app.workflowPr"
                      class="workflow-link"
                      :href="app.workflowPr"
                      target="_blank"
                      rel="noopener noreferrer"
                      >{{
                        app.workflowPrState === 'merged'
                          ? 'PR merged'
                          : app.workflowPrState === 'closed'
                            ? 'PR closed'
                            : 'Workflow PR'
                      }}<UiIcon name="arrow" /></a
                    ><UiButton
                      v-else
                      :disabled="busy || app.deletionRequested"
                      @click="retryWorkflow(app)"
                      >Retry workflow</UiButton
                    >
                  </div>
                  <p v-if="latest(app)?.error" class="application-error">
                    {{ latest(app)?.error }}
                  </p>
                </article>
              </div>
            </template>

            <template v-else>
              <div v-if="!filteredDeployments.length" class="inventory-empty">
                <span class="empty-app-icon"><UiIcon name="activity" /></span>
                <h2>
                  {{ deployments.length ? 'No matching deployments' : 'Ready when you are.' }}
                </h2>
                <p>
                  {{
                    deployments.length
                      ? 'Try another search or status filter.'
                      : 'After a successful GitHub workflow, your deployment appears here. We’ll take it from there.'
                  }}
                </p>
                <UiButton v-if="deployments.length" @click="clearFilters">Clear filters</UiButton
                ><UiButton v-else @click="changeView('apps')">View applications</UiButton>
              </div>
              <ol v-else class="console-deployments">
                <li v-for="deployment in filteredDeployments" :key="deployment.id">
                  <span class="timeline-marker" :data-status="deployment.status"
                    ><UiIcon :name="deployment.status === 'live' ? 'check' : 'activity'"
                  /></span>
                  <div class="timeline-content">
                    <div>
                      <strong>{{ appName(deployment.appId) }}</strong
                      ><span class="deployment-status" :data-status="deployment.status"
                        ><span class="state-dot"></span>{{ statusLabel(deployment.status) }}</span
                      >
                    </div>
                    <p>
                      <a
                        :href="commitUrl(deployment)"
                        target="_blank"
                        rel="noopener noreferrer"
                        :aria-label="`View commit ${deployment.commitSha.slice(0, 7)} on GitHub`"
                        ><code>{{ deployment.commitSha.slice(0, 7) }}</code></a
                      ><RouterLink :to="`/console/deployments/${deployment.id}`"
                        >Run #{{ deployment.runId }}</RouterLink
                      ><span>Attempt {{ deployment.attempt }}</span>
                    </p>
                    <p v-if="deployment.error" class="application-error">{{ deployment.error }}</p>
                  </div>
                  <time :datetime="new Date(deployment.createdAt * 1000).toISOString()">{{
                    dateLabel(deployment.createdAt)
                  }}</time>
                </li>
              </ol>
            </template>
            <div class="inventory-footer">
              <span>{{
                view === 'apps'
                  ? filteredApps.length + ' applications'
                  : filteredDeployments.length + ' deployments'
              }}</span
              ><span v-if="lastUpdated"
                >Updated
                {{
                  lastUpdated.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
                }}</span
              >
            </div>
          </section>
        </template>
      </main>
    </div>

    <ConsoleNavigation :active="view" mobile />
    <RouterView />
  </div>
</template>
