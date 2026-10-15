<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted } from "vue";
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
const busy = ref(false);
const error = ref("");
const notice = ref("");
const password = ref("");
const email = ref("");
const apps = ref<App[]>([]);
const deployments = ref<Deployment[]>([]);
const repositories = ref<Repository[]>([]);
const adding = ref(false);
const loadingRepositories = ref(false);
const form = reactive({
  repository: "",
  branch: "main",
  domain: "",
  installCommand: "npm ci",
  buildCommand: "npm run build",
  outputDirectory: "dist",
  spa: false,
});
let timer: ReturnType<typeof setTimeout> | undefined;
let stopped = false;
let formTrigger: HTMLElement | null = null;

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
    apps.value = [];
    deployments.value = [];
    clearTimeout(timer);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Could not sign out.";
  }
}

async function openForm() {
  formTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
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

function restoreFormFocus(event: Event) {
  if (formTrigger?.isConnected) {
    event.preventDefault();
    formTrigger.focus({ preventScroll: true });
  }
}

function chooseRepository() {
  form.branch =
    repositories.value.find((repo) => repo.fullName === form.repository)?.defaultBranch ?? "main";
}

async function addApp() {
  busy.value = true;
  error.value = "";
  notice.value = "";

  try {
    await request("/apps", { method: "POST", body: body("apps", form) });
    adding.value = false;
    notice.value = "Workflow PR opened. Review and merge it on GitHub to enable deployments.";
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
const liveApps = computed(() => apps.value.filter((app) => latest(app)?.status === "live").length);
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
  <TabsRoot class="deployment-console" :model-value="view" @update:model-value="changeView">
    <aside class="console-sidebar">
      <RouterLink class="console-brand" to="/" aria-label="Deployer home"
        ><span class="brand-symbol"><UiIcon name="arrow" /></span>deployer<span class="brand-period"
          >.</span
        ></RouterLink
      >
      <div class="instance-switch">
        <UiIcon name="server" />
        <div><strong>Your instance</strong><span>Personal workspace</span></div>
      </div>
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
      <div class="sidebar-bottom">
        <div class="ownership-note">
          <UiIcon name="server" /><strong>Your server.<br />Your apps.</strong>
          <p>Built on GitHub.<br />Served from your VPS.</p>
        </div>
        <UiButton
          v-if="authenticated"
          class="signout-button"
          aria-label="Sign out"
          :disabled="busy"
          @click="signOut"
          ><span class="owner-avatar">R</span><span>Root account<small>Instance owner</small></span
          ><UiIcon name="logout"
        /></UiButton>
        <RouterLink v-else class="sidebar-home" to="/"
          >Back to home <span aria-hidden="true">↗</span></RouterLink
        >
      </div>
    </aside>

    <div class="console-workspace">
      <header class="workspace-header">
        <div class="workspace-breadcrumb">
          Workspace<span aria-hidden="true">/</span
          ><strong>{{ view === "apps" ? "Applications" : "Deployments" }}</strong>
        </div>
        <div class="workspace-header-actions">
          <span class="workspace-label">Self-hosted</span
          ><UiButton
            v-if="authenticated"
            class="mobile-signout"
            aria-label="Sign out"
            :disabled="busy"
            @click="signOut"
            ><UiIcon name="logout"
          /></UiButton>
        </div>
      </header>
      <main id="main" class="workspace-main">
        <div class="workspace-heading">
          <div>
            <h1>{{ view === "apps" ? "Applications" : "Deployments" }}</h1>
            <p>
              {{
                view === "apps"
                  ? "A place for everything you ship."
                  : "Every release, from workflow to web."
              }}
            </p>
          </div>
          <UiButton v-if="authenticated" class="primary" :disabled="busy" @click="openForm"
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
        <section v-else-if="!authenticated" class="console-auth">
          <div class="auth-intro">
            <span class="auth-icon"><UiIcon name="server" /></span>
            <h2>Welcome back.</h2>
            <p>Your applications, deployments, and next big idea. All in one place.</p>
            <div class="auth-detail">
              <UiIcon name="repo" />GitHub builds. Your server delivers.
            </div>
          </div>
          <form class="console-signin" @submit.prevent="signIn">
            <h2>Sign in to your instance</h2>
            <p>Use your root account to continue.</p>
            <Label for="email">Email address</Label
            ><UiInput
              id="email"
              v-model="email"
              type="email"
              autocomplete="username"
              maxlength="254"
              required
              :disabled="busy"
            /><Label for="password">Password</Label
            ><UiInput
              id="password"
              v-model="password"
              type="password"
              autocomplete="current-password"
              required
              :disabled="busy"
            /><UiButton type="submit" class="primary" :disabled="busy"
              >{{ busy ? "Signing in…" : "Sign in" }}<UiIcon name="arrow"
            /></UiButton>
          </form>
        </section>

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
                <UiButton class="primary" :disabled="busy" @click="openForm"
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
                        <a :href="`http://${app.domain}`" target="_blank" rel="noopener noreferrer"
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
                    <span class="deployment-status" :data-status="latest(app)?.status"
                      ><span class="state-dot"></span>{{ statusLabel(latest(app)?.status) }}</span
                    ><span v-if="latest(app)" class="deployment-stamp"
                      ><code>{{ latest(app)!.commitSha.slice(0, 7) }}</code
                      ><time :datetime="new Date(latest(app)!.createdAt * 1000).toISOString()">{{
                        dateLabel(latest(app)!.createdAt)
                      }}</time></span
                    ><span v-else class="deployment-stamp">Merge your workflow PR to begin</span>
                  </div>
                  <div class="application-action">
                    <a
                      v-if="app.workflowPr"
                      class="workflow-link"
                      :href="app.workflowPr"
                      target="_blank"
                      rel="noopener noreferrer"
                      >Workflow PR<UiIcon name="arrow" /></a
                    ><UiButton v-else :disabled="busy" @click="retryWorkflow(app)"
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
          <aside v-if="view === 'apps'" class="workflow-note">
            <UiIcon name="branch" />
            <div>
              <strong>A push is all it takes.</strong>
              <p>
                Merge your workflow PR, then push to your connected branch. GitHub builds your app;
                Deployer publishes it.
              </p>
            </div>
          </aside>
        </template>
      </main>
      <footer class="workspace-footer">
        <span>Your server. Your apps.</span><span>Powered by GitHub Actions & Nginx</span>
      </footer>
    </div>

    <DialogRoot v-model:open="adding"
      ><DialogPortal
        ><DialogOverlay class="console-dialog-overlay" /><DialogContent
          class="console-drawer"
          @interact-outside="busy && $event.preventDefault()"
          @escape-key-down="busy && $event.preventDefault()"
          @close-auto-focus="restoreFormFocus"
        >
          <div class="drawer-heading">
            <span class="drawer-icon"><UiIcon name="repo" /></span
            ><DialogClose as-child
              ><UiButton :disabled="busy" aria-label="Close new application"
                ><UiIcon name="close" /></UiButton
            ></DialogClose>
          </div>
          <DialogTitle class="drawer-title">Connect a repository</DialogTitle
          ><DialogDescription class="drawer-description"
            >Give your next app a home. We’ll open a pull request with your build workflow for you
            to review.</DialogDescription
          >
          <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
          <form class="drawer-form" @submit.prevent="addApp">
            <fieldset :disabled="busy">
              <legend>Source</legend>
              <Label for="repository">Repository</Label
              ><UiSelect
                id="repository"
                v-model="form.repository"
                name="repository"
                required
                :options="
                  repositories.map((repo) => ({ value: repo.fullName, label: repo.fullName }))
                "
                :placeholder="loadingRepositories ? 'Loading repositories…' : 'Select a repository'"
                :disabled="busy || loadingRepositories"
                @update:model-value="chooseRepository"
              />
              <p v-if="!loadingRepositories && !repositories.length" class="field-hint">
                No repositories found. Install your GitHub App on a repository, then reopen this
                form.
              </p>
              <Label for="branch">Production branch</Label
              ><UiInput id="branch" v-model="form.branch" required :disabled="busy" />
              <p class="field-hint">Pushes to this branch trigger a deployment.</p>
            </fieldset>
            <fieldset :disabled="busy">
              <legend>Build settings</legend>
              <div class="drawer-field-grid">
                <div>
                  <Label for="install">Install command</Label
                  ><UiInput id="install" v-model="form.installCommand" required :disabled="busy" />
                </div>
                <div>
                  <Label for="build">Build command</Label
                  ><UiInput id="build" v-model="form.buildCommand" required :disabled="busy" />
                </div>
              </div>
              <Label for="output">Output directory</Label
              ><UiInput id="output" v-model="form.outputDirectory" required :disabled="busy" />
              <p class="field-hint">The folder containing your built index.html.</p>
            </fieldset>
            <fieldset :disabled="busy">
              <legend>Destination</legend>
              <Label for="domain">Domain</Label
              ><UiInput
                id="domain"
                v-model="form.domain"
                placeholder="www.example.com"
                required
                :disabled="busy"
                autocapitalize="none"
              />
              <p class="field-hint">
                Point DNS at your VPS. Configure TLS with your certificate tooling.
              </p>
              <Label for="spa" class="checkbox"
                ><UiCheckbox id="spa" v-model="form.spa" :disabled="busy" />Enable SPA
                routing</Label
              >
              <p class="field-hint">Serve index.html for routes that don’t match a file.</p>
            </fieldset>
            <div class="drawer-actions">
              <DialogClose as-child><UiButton :disabled="busy">Cancel</UiButton></DialogClose
              ><UiButton
                type="submit"
                class="primary"
                :disabled="busy || loadingRepositories || !form.repository"
                >{{ busy ? "Opening PR…" : "Create workflow PR" }}<UiIcon name="arrow"
              /></UiButton>
            </div>
          </form> </DialogContent></DialogPortal
    ></DialogRoot>
  </TabsRoot>
</template>
