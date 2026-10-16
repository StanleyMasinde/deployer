<script setup lang="ts">
import { ref, reactive, computed, watchEffect, onMounted, onUnmounted } from "vue";
import { buildPreset, type Ecosystem, type PackageManager } from "../build";
import { appUrl } from "../https";
import { signInSchema, applicationSchema, useValidation } from "../validation";
import AuthLayout from "../components/AuthLayout.vue";
import AppDetails from "../components/AppDetails.vue";
import UiCheckbox from "../components/ui/UiCheckbox.vue";
import UiSelect from "../components/ui/UiSelect.vue";
import {
  Label,
  TabsRoot,
  TabsList,
  TabsTrigger,
  TabsContent,
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
} from "reka-ui";
import UiIcon from "../components/ui/UiIcon.vue";
import "../console.css";
import UiInput from "../components/ui/UiInput.vue";
import UiButton from "../components/ui/UiButton.vue";
import { useRouter } from "vue-router";
import { ApiError, request, body, type App, type Deployment, type Repository } from "../api/client";

definePage({
  name: "console",
  meta: { ssg: false, title: "Apps · Deployer" },
});

const router = useRouter();
const view = ref("apps");
const search = ref("");
const filter = ref("all");
const refreshing = ref(false);
const lastUpdated = ref<Date>();
const authenticated = ref(false);
const checking = ref(true);
watchEffect(() => {
  if (typeof document !== "undefined")
    document.title = `${authenticated.value ? (view.value === "apps" ? "Applications" : "Deployments") : "Sign in"} · Deployer`;
});
const busy = ref(false);
const error = ref("");
const notice = ref("");
const password = ref("");
const email = ref("");
const { errors: fieldErrors, validate } = useValidation();
const apps = ref<App[]>([]);
const deployments = ref<Deployment[]>([]);
const repositories = ref<Repository[]>([]);
const adding = ref(false);
const editing = ref<number | null>(null);
const detailAppId = ref<number | null>(null);
const detailsOpen = ref(false);
const loadingRepositories = ref(false);
const form = reactive({
  repository: "",
  branch: "main",
  domain: "",
  ...buildPreset("none"),
  spa: false,
});
const repositoryOptions = computed(() => {
  const options = repositories.value.map((repo) => ({
    value: repo.fullName,
    label: repo.fullName,
  }));
  if (
    editing.value !== null &&
    form.repository &&
    !options.some((option) => option.value === form.repository)
  )
    options.unshift({ value: form.repository, label: form.repository });
  return options;
});
let timer: ReturnType<typeof setTimeout> | undefined;
let stopped = false;
let formTrigger: HTMLElement | null = null;
let detailTrigger: HTMLElement | null = null;

async function refresh() {
  try {
    const [nextApps, nextDeployments] = await Promise.all([
      request<App[]>("/apps"),
      request<Deployment[]>("/deployments"),
    ]);

    apps.value = nextApps;
    deployments.value = nextDeployments;
    lastUpdated.value = new Date();
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 401) authenticated.value = false;
    error.value = cause instanceof Error ? cause.message : "Could not refresh deployments.";
  }
}

async function poll() {
  if (stopped || !authenticated.value) return;

  await refresh();

  if (!stopped && authenticated.value) timer = setTimeout(poll, 5000);
}

async function signIn() {
  if (!validate(signInSchema, { email: email.value, password: password.value })) return;
  busy.value = true;
  error.value = "";

  try {
    await request("/session", {
      method: "POST",
      body: body("sessions", { email: email.value.trim(), password: password.value }),
    });
    password.value = "";
    authenticated.value = true;
    await poll();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not sign in.";
  } finally {
    busy.value = false;
  }
}

async function signOut() {
  try {
    await request("/session", { method: "DELETE" });
    authenticated.value = false;
    detailsOpen.value = false;
    adding.value = false;
    apps.value = [];
    deployments.value = [];
    clearTimeout(timer);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not sign out.";
  }
}

function openDetails(app: App) {
  detailTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  detailAppId.value = app.id;
  detailsOpen.value = true;
}
async function openForm(app?: App) {
  fieldErrors.value = {};
  detailsOpen.value = false;
  editing.value = app?.id ?? null;
  Object.assign(
    form,
    app
      ? {
          repository: app.repository,
          branch: app.branch,
          domain: app.domain,
          buildConfig: { ...app.buildConfig },
          installCommand: app.installCommand,
          buildCommand: app.buildCommand,
          outputDirectory: app.outputDirectory,
          spa: app.spa,
        }
      : { repository: "", branch: "main", domain: "", ...buildPreset("none"), spa: false },
  );
  formTrigger = app
    ? detailTrigger
    : document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  adding.value = true;
  loadingRepositories.value = true;
  error.value = "";

  try {
    repositories.value = await request<Repository[]>("/repositories");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not list repositories.";
  } finally {
    loadingRepositories.value = false;
  }
}

function restoreDetailFocus(event: Event) {
  event.preventDefault();
  if (!adding.value && detailTrigger?.isConnected) detailTrigger.focus({ preventScroll: true });
}

function restoreFormFocus(event: Event) {
  if (formTrigger?.isConnected) {
    event.preventDefault();
    formTrigger.focus({ preventScroll: true });
  }
}

function changeEcosystem(ecosystem: string) {
  Object.assign(form, buildPreset(ecosystem as Ecosystem));
}

function changePackageManager(packageManager: string) {
  const preset = buildPreset("javascript", packageManager as PackageManager);
  form.buildConfig = preset.buildConfig;
  form.installCommand = preset.installCommand;
  form.buildCommand = preset.buildCommand;
}

function chooseRepository() {
  form.branch =
    repositories.value.find((repo) => repo.fullName === form.repository)?.defaultBranch ?? "main";
}

async function addApp() {
  if (!validate(applicationSchema, form)) return;
  busy.value = true;
  error.value = "";
  notice.value = "";

  try {
    await request(editing.value === null ? "/apps" : `/apps/${editing.value}`, {
      method: editing.value === null ? "POST" : "PUT",
      body: body("apps", form),
    });
    adding.value = false;
    notice.value =
      editing.value === null
        ? "Workflow PR opened. Review and merge it on GitHub to enable deployments."
        : "Settings saved. Domain and routing updates are queued for the worker. Update the workflow PR to apply build changes.";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not add the app.";
  } finally {
    await refresh();
    busy.value = false;
  }
}

async function retryWorkflow(app: App) {
  busy.value = true;

  try {
    await request(`/apps/${app.id}/workflow`, { method: "POST" });
    await refresh();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not open the workflow PR.";
  } finally {
    busy.value = false;
  }
}

function latest(app: App) {
  return deployments.value
    .filter((d) => d.appId === app.id)
    .sort((a, b) => b.createdAt - a.createdAt || b.id - a.id)[0];
}
function matchesStatus(status: string | undefined) {
  return (
    filter.value === "all" ||
    (filter.value === "attention" ? status === "failed" || !status : status === filter.value)
  );
}
const filteredApps = computed(() =>
  apps.value.filter((app) => {
    const query = search.value.toLowerCase();
    return (
      `${app.domain} ${app.repository} ${app.branch}`.toLowerCase().includes(query) &&
      matchesStatus(latest(app)?.status)
    );
  }),
);
const sortedDeployments = computed(() =>
  [...deployments.value].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
);
const filteredDeployments = computed(() =>
  sortedDeployments.value.filter((deployment) => {
    const app = apps.value.find((a) => a.id === deployment.appId);
    return (
      `${app?.domain ?? ""} ${app?.repository ?? ""} ${deployment.commitSha}`
        .toLowerCase()
        .includes(search.value.toLowerCase()) && matchesStatus(deployment.status)
    );
  }),
);
const liveApps = computed(
  () => apps.value.filter((app) => !app.deletionRequested && latest(app)?.status === "live").length,
);
const attentionApps = computed(
  () => apps.value.filter((app) => !latest(app) || latest(app)?.status === "failed").length,
);
const activeDeployments = computed(
  () => deployments.value.filter((d) => ["queued", "deploying"].includes(d.status)).length,
);
function statusLabel(status?: string) {
  return (
    (
      {
        live: "Live",
        failed: "Failed",
        queued: "Queued",
        deploying: "Deploying",
        superseded: "Superseded",
      } as Record<string, string>
    )[status ?? ""] ??
    status ??
    "Awaiting build"
  );
}
function dateLabel(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
function appName(id: number) {
  return apps.value.find((app) => app.id === id)?.domain ?? `App ${id}`;
}
async function manualRefresh() {
  refreshing.value = true;
  error.value = "";
  try {
    await refresh();
  } finally {
    refreshing.value = false;
  }
}
function changeView(value: string | number) {
  view.value = String(value);
  search.value = "";
  filter.value = "all";
}

onMounted(async () => {
  try {
    const setup = await request<{ required: boolean }>("/setup");

    if (setup.required) {
      await router.replace("/setup");
      return;
    }

    await request("/session");
    authenticated.value = true;
    await poll();
  } catch (cause) {
    if (!(cause instanceof ApiError && cause.status === 401)) {
      error.value = cause instanceof Error ? cause.message : "Could not check your session.";
    }
  } finally {
    checking.value = false;
  }
});

onUnmounted(() => {
  stopped = true;
  clearTimeout(timer);
});
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
        >{{ busy ? "Signing in…" : "Sign in" }}<UiIcon name="arrow"
      /></UiButton>
    </form>
  </AuthLayout>
  <TabsRoot v-else class="deployment-console" :model-value="view" @update:model-value="changeView">
    <aside class="console-sidebar">
      <RouterLink class="console-brand" to="/" aria-label="Deployer home"
        ><span class="brand-symbol" aria-hidden="true">↥</span>Deployer</RouterLink
      >
      <nav aria-label="Workspace navigation">
        <TabsList class="workspace-nav" aria-label="Workspace sections">
          <TabsTrigger value="apps"
            ><UiIcon name="apps" />Applications<span v-if="authenticated" class="nav-count">{{
              apps.length
            }}</span></TabsTrigger
          >
          <TabsTrigger value="activity"
            ><UiIcon name="activity" />Deployments<span
              v-if="activeDeployments"
              class="nav-count"
              >{{ activeDeployments }}</span
            ></TabsTrigger
          >
        </TabsList>
      </nav>
      <div class="console-account">
        <span class="workspace-label">Your instance</span>
        <UiButton v-if="authenticated" aria-label="Sign out" :disabled="busy" @click="signOut"
          ><UiIcon name="logout" /><span>Sign out</span></UiButton
        >
      </div>
    </aside>

    <div class="console-workspace">
      <main id="main" class="workspace-main">
        <div class="workspace-heading">
          <div>
            <p class="page-eyebrow"><UiIcon name="server" /> INSTANCE CONSOLE</p>
            <h1>
              {{
                !authenticated && !checking
                  ? "Sign in to Deployer"
                  : view === "apps"
                    ? "Applications"
                    : "Deployments"
              }}
            </h1>
            <p>
              {{
                view === "apps"
                  ? "Manage repositories, domains, and releases."
                  : "Track release progress and deployment history."
              }}
            </p>
          </div>
          <UiButton v-if="authenticated" class="primary" :disabled="busy" @click="openForm()"
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
          <dl class="workspace-summary" aria-label="Deployment summary">
            <div>
              <dt>Applications</dt>
              <dd>{{ apps.length }}<span>Total connected</span></dd>
            </div>
            <div>
              <dt><span class="state-dot is-live"></span>Live</dt>
              <dd>{{ liveApps }}<span>Latest deployment live</span></dd>
            </div>
            <div>
              <dt><span class="state-dot is-working"></span>In progress</dt>
              <dd>{{ activeDeployments }}<span>Queued or deploying</span></dd>
            </div>
            <div>
              <dt><span class="state-dot is-waiting"></span>Needs attention</dt>
              <dd>{{ attentionApps }}<span>Failed or awaiting a build</span></dd>
            </div>
          </dl>

          <TabsContent
            :value="view"
            as="section"
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
                <UiButton
                  @click="
                    search = '';
                    filter = 'all';
                  "
                  >Clear filters</UiButton
                >
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
                        <a :href="appUrl(app)" target="_blank" rel="noopener noreferrer"
                          >{{ app.domain }}<UiIcon name="arrow"
                        /></a>
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
                        ><span>{{ app.spa ? "SPA" : "Static site" }}</span>
                      </div>
                    </div>
                  </div>
                  <div class="application-deployment">
                    <span
                      class="deployment-status"
                      :data-status="app.deletionRequested ? 'deleting' : latest(app)?.status"
                      ><span class="state-dot"></span
                      >{{
                        app.deletionRequested ? "Deleting" : statusLabel(latest(app)?.status)
                      }}</span
                    ><span v-if="latest(app)" class="deployment-stamp"
                      ><code>{{ latest(app)!.commitSha.slice(0, 7) }}</code
                      ><time :datetime="new Date(latest(app)!.createdAt * 1000).toISOString()">{{
                        dateLabel(latest(app)!.createdAt)
                      }}</time></span
                    ><span v-else class="deployment-stamp">Merge your workflow PR to begin</span>
                  </div>
                  <div class="application-action">
                    <UiButton class="details-button" @click="openDetails(app)">Details</UiButton>
                    <a
                      v-if="app.workflowPr"
                      class="workflow-link"
                      :href="app.workflowPr"
                      target="_blank"
                      rel="noopener noreferrer"
                      >Workflow PR<UiIcon name="arrow" /></a
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
                  {{ deployments.length ? "No matching deployments" : "Ready when you are." }}
                </h2>
                <p>
                  {{
                    deployments.length
                      ? "Try another search or status filter."
                      : "After a successful GitHub workflow, your deployment appears here. We’ll take it from there."
                  }}
                </p>
                <UiButton
                  v-if="deployments.length"
                  @click="
                    search = '';
                    filter = 'all';
                  "
                  >Clear filters</UiButton
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
                      <code>{{ deployment.commitSha.slice(0, 7) }}</code
                      ><span>Run #{{ deployment.runId }} · Attempt {{ deployment.attempt }}</span>
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
                view === "apps"
                  ? filteredApps.length + " applications"
                  : filteredDeployments.length + " deployments"
              }}</span
              ><span v-if="lastUpdated"
                >Updated
                {{ lastUpdated.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
                }}<span class="refresh-caption"> · Refreshes every 5s</span></span
              >
            </div>
          </TabsContent>
        </template>
      </main>
    </div>

    <AppDetails
      v-model:open="detailsOpen"
      :app-id="detailAppId"
      @close-auto-focus="restoreDetailFocus"
      @edit="openForm"
      @changed="refresh"
    />
    <DialogRoot v-model:open="adding"
      ><DialogPortal
        ><DialogOverlay class="console-dialog-overlay" /><DialogContent
          class="console-modal"
          @interact-outside="busy && $event.preventDefault()"
          @escape-key-down="busy && $event.preventDefault()"
          @close-auto-focus="restoreFormFocus"
        >
          <div class="modal-heading">
            <span class="modal-icon"><UiIcon name="repo" /></span
            ><DialogClose as-child
              ><UiButton :disabled="busy" aria-label="Close new application"
                ><UiIcon name="close" /></UiButton
            ></DialogClose>
          </div>
          <DialogTitle class="modal-title">{{
            editing === null ? "Connect a repository" : "Edit application"
          }}</DialogTitle
          ><DialogDescription class="modal-description">{{
            editing === null
              ? "Give your next app a home. We’ll open a pull request with your build workflow for you to review."
              : "Update your source, build, and destination settings. Build changes require an updated workflow PR."
          }}</DialogDescription>
          <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
          <form novalidate class="modal-form" @submit.prevent="addApp">
            <fieldset :disabled="busy">
              <legend>Source</legend>
              <Label for="repository">Repository</Label
              ><UiSelect
                id="repository"
                :error="fieldErrors['repository']"
                v-model="form.repository"
                name="repository"
                :options="repositoryOptions"
                :placeholder="loadingRepositories ? 'Loading repositories…' : 'Select a repository'"
                :disabled="busy || loadingRepositories"
                @update:model-value="chooseRepository"
              />
              <p v-if="!loadingRepositories && !repositories.length" class="field-hint">
                No repositories found. Install your GitHub App on a repository, then reopen this
                form.
              </p>
              <Label for="branch">Production branch</Label
              ><UiInput
                id="branch"
                :error="fieldErrors['branch']"
                v-model="form.branch"
                :disabled="busy"
              />
              <p class="field-hint">Pushes to this branch trigger a deployment.</p>
            </fieldset>
            <fieldset :disabled="busy">
              <legend>Build setup</legend>
              <Label for="deployment-type">Deployment type</Label>
              <UiSelect
                id="deployment-type"
                :error="fieldErrors['buildConfig.deploymentType']"
                v-model="form.buildConfig.deploymentType"
                :options="[{ value: 'static', label: 'Static site' }]"
                :disabled="busy"
              />
              <p class="field-hint">Serve HTML, CSS, and JavaScript from your VPS.</p>
              <Label for="ecosystem">Build ecosystem</Label>
              <UiSelect
                id="ecosystem"
                :error="fieldErrors['buildConfig.ecosystem']"
                :model-value="form.buildConfig.ecosystem"
                :options="[
                  { value: 'none', label: 'None — plain static files' },
                  { value: 'javascript', label: 'JavaScript' },
                ]"
                :disabled="busy"
                @update:model-value="changeEcosystem"
              />
              <p class="field-hint">Choose how your site is built.</p>
              <template v-if="form.buildConfig.ecosystem === 'javascript'">
                <Label for="package-manager">Package manager</Label>
                <UiSelect
                  id="package-manager"
                  :error="fieldErrors['buildConfig.packageManager']"
                  :model-value="form.buildConfig.packageManager ?? 'npm'"
                  :options="[
                    { value: 'npm', label: 'npm (Node.js 24)' },
                    { value: 'pnpm', label: 'pnpm 12' },
                    { value: 'yarn', label: 'Yarn 4' },
                    { value: 'bun', label: 'Bun 1' },
                  ]"
                  :disabled="busy"
                  @update:model-value="changePackageManager"
                />
                <p class="field-hint">
                  Installs the selected tool in your GitHub workflow. Commit its lockfile before
                  building.
                </p>
              </template>
              <Label for="framework">Framework preset</Label>
              <UiSelect
                id="framework"
                :error="fieldErrors['buildConfig.framework']"
                v-model="form.buildConfig.framework"
                :options="
                  form.buildConfig.ecosystem === 'none'
                    ? [{ value: 'plainHtml', label: 'Plain HTML / CSS / JavaScript' }]
                    : [{ value: 'vite', label: 'Vite' }]
                "
                :disabled="busy"
              />
              <p class="field-hint">
                {{
                  form.buildConfig.ecosystem === "none"
                    ? "Publish files as they are, with no runtime or build step."
                    : "Build with Vite, then publish the output directory. Commands remain editable."
                }}
              </p>
              <div v-if="form.buildConfig.ecosystem === 'javascript'" class="modal-field-grid">
                <div>
                  <Label for="install">Install command</Label
                  ><UiInput
                    id="install"
                    :error="fieldErrors['installCommand']"
                    v-model="form.installCommand"
                    :disabled="busy"
                  />
                </div>
                <div>
                  <Label for="build">Build command</Label
                  ><UiInput
                    id="build"
                    :error="fieldErrors['buildCommand']"
                    v-model="form.buildCommand"
                    :disabled="busy"
                  />
                </div>
              </div>
              <Label for="output">Output directory</Label
              ><UiInput
                id="output"
                :error="fieldErrors['outputDirectory']"
                v-model="form.outputDirectory"
                :disabled="busy"
              />
              <p class="field-hint">
                {{
                  form.buildConfig.ecosystem === "none"
                    ? "The folder containing index.html. Use . for the repository root."
                    : "The folder containing your built index.html."
                }}
              </p>
            </fieldset>
            <fieldset :disabled="busy">
              <legend>Destination</legend>
              <Label for="domain">Domain</Label
              ><UiInput
                id="domain"
                :error="fieldErrors['domain']"
                v-model="form.domain"
                placeholder="www.example.com"
                :disabled="busy"
                autocapitalize="none"
              />
              <p class="field-hint">
                Point DNS at your VPS. Enable HTTPS in application details. Domain changes require a
                certificate for the new hostname.
              </p>
              <Label for="spa" class="checkbox"
                ><UiCheckbox id="spa" v-model="form.spa" :disabled="busy" />Enable SPA
                routing</Label
              >
              <p class="field-hint">Serve index.html for routes that don’t match a file.</p>
            </fieldset>
            <div class="modal-actions">
              <DialogClose as-child><UiButton :disabled="busy">Cancel</UiButton></DialogClose
              ><UiButton
                type="submit"
                class="primary"
                :disabled="busy || loadingRepositories || !form.repository"
                >{{
                  busy
                    ? editing === null
                      ? "Opening PR…"
                      : "Saving…"
                    : editing === null
                      ? "Create workflow PR"
                      : "Save settings"
                }}<UiIcon name="arrow"
              /></UiButton>
            </div>
          </form> </DialogContent></DialogPortal
    ></DialogRoot>
  </TabsRoot>
</template>
