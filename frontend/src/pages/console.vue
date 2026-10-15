<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted } from "vue";
import { useRouter } from "vue-router";
import { ApiError, request, body, type App, type Deployment, type Repository } from "../api/client";

definePage({
  name: "console",
  meta: { ssg: false, title: "Apps · Deployer" },
});

const router = useRouter();
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

async function refresh() {
  try {
    const [nextApps, nextDeployments] = await Promise.all([
      request<App[]>("/apps"),
      request<Deployment[]>("/deployments"),
    ]);

    apps.value = nextApps;
    deployments.value = nextDeployments;
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
  return deployments.value.find((deployment) => deployment.appId === app.id);
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
  <main id="main" class="console">
    <p class="eyebrow">YOUR VPS / DEPLOYMENTS</p>
    <div class="section-heading">
      <div>
        <h1>Apps</h1>
        <p>From your repository to your domain.</p>
      </div>
      <div v-if="authenticated" class="actions">
        <button :disabled="busy" @click="signOut">Sign out</button>
        <button class="primary" :disabled="busy" @click="openForm">
          Add a app <span aria-hidden="true">+</span>
        </button>
      </div>
    </div>

    <p v-if="error" class="message error" role="alert">{{ error }}</p>
    <p v-if="notice" class="message" role="status">{{ notice }}</p>
    <p v-if="checking" role="status">Checking your session…</p>

    <form v-else-if="!authenticated" class="signin" @submit.prevent="signIn">
      <h2>Sign in to your instance</h2>
      <p>Use the root account you created for this instance.</p>
      <label for="email">Email address</label>
      <input
        id="email"
        v-model="email"
        type="email"
        autocomplete="username"
        maxlength="254"
        required
        :disabled="busy"
      />
      <label for="password">Password</label>
      <input
        id="password"
        v-model="password"
        type="password"
        autocomplete="current-password"
        required
        :disabled="busy"
      />
      <button class="primary" :disabled="busy">{{ busy ? "Signing in…" : "Sign in" }}</button>
    </form>

    <template v-else>
      <section v-if="adding" class="add-app" aria-labelledby="add-heading">
        <div class="section-heading">
          <h2 id="add-heading">Connect a repository</h2>
          <button :disabled="busy" @click="adding = false">Cancel</button>
        </div>
        <p>
          Deployer opens a PR with your build workflow. Nothing changes on the default branch until
          you merge it.
        </p>
        <form class="form-grid" @submit.prevent="addApp">
          <label class="full"
            >Repository
            <select
              v-model="form.repository"
              required
              :disabled="busy || loadingRepositories"
              @change="chooseRepository"
            >
              <option disabled value="">
                {{ loadingRepositories ? "Loading repositories…" : "Select a repository" }}
              </option>
              <option v-for="repo in repositories" :key="repo.fullName" :value="repo.fullName">
                {{ repo.fullName }}
              </option>
            </select>
          </label>
          <p v-if="!loadingRepositories && !repositories.length" class="full">
            No repositories found. Install your GitHub App on a repository, then reopen this form.
          </p>
          <label>Branch<input v-model="form.branch" required :disabled="busy" /></label>
          <label
            >Domain<input
              v-model="form.domain"
              placeholder="www.example.com"
              required
              :disabled="busy"
              autocapitalize="none"
          /></label>
          <label
            >Install command<input v-model="form.installCommand" required :disabled="busy"
          /></label>
          <label
            >Build command<input v-model="form.buildCommand" required :disabled="busy"
          /></label>
          <label
            >Output directory<input v-model="form.outputDirectory" required :disabled="busy"
          /></label>
          <label class="checkbox"
            ><input v-model="form.spa" type="checkbox" :disabled="busy" /> Use index.html for SPA
            routes</label
          >
          <p class="full hint">
            Point your domain’s DNS at this VPS. App serving starts over HTTP; configure TLS with
            your certificate tooling.
          </p>
          <div class="full">
            <button class="primary" :disabled="busy || loadingRepositories || !form.repository">
              {{ busy ? "Opening workflow PR…" : "Create workflow PR" }}
            </button>
          </div>
        </form>
      </section>

      <section v-if="!apps.length" class="empty">
        <span class="empty-mark" aria-hidden="true">↥</span>
        <h2>Your first app belongs here.</h2>
        <p>
          Connect a repository available to your GitHub App. Your VPS will only download and serve
          the finished files.
        </p>
        <button :disabled="busy" @click="openForm">
          Connect a repository <span aria-hidden="true">→</span>
        </button>
      </section>

      <section v-else class="app-list" aria-label="Your apps">
        <article v-for="app in apps" :key="app.id" class="app-row">
          <div>
            <h2>
              <a :href="`http://${app.domain}`" target="_blank" rel="noopener noreferrer"
                >{{ app.domain }} ↗</a
              >
            </h2>
            <p class="mono">
              {{ app.repository }} <span class="muted">/ {{ app.branch }}</span>
            </p>
          </div>
          <div class="app-state">
            <span class="status" :data-status="latest(app)?.status">{{
              latest(app)?.status ?? "Awaiting build"
            }}</span>
            <a
              v-if="app.workflowPr"
              :href="app.workflowPr"
              target="_blank"
              rel="noopener noreferrer"
              >Workflow PR ↗</a
            >
            <button v-else :disabled="busy" @click="retryWorkflow(app)">Retry workflow PR</button>
          </div>
        </article>
      </section>

      <section class="activity" aria-labelledby="activity-heading">
        <div class="section-heading">
          <h2 id="activity-heading">Deployment activity</h2>
          <span class="hint">Refreshes every 5 seconds</span>
        </div>
        <p v-if="!deployments.length" class="muted">
          Successful workflow runs will appear here after GitHub delivers a webhook.
        </p>
        <ol v-else class="deployment-list">
          <li v-for="deployment in deployments" :key="deployment.id">
            <div class="deployment-summary">
              <span class="status" :data-status="deployment.status">{{ deployment.status }}</span
              ><strong>{{
                apps.find((app) => app.id === deployment.appId)?.domain ?? `App ${deployment.appId}`
              }}</strong
              ><code>{{ deployment.commitSha.slice(0, 7) }}</code
              ><time :datetime="new Date(deployment.createdAt * 1000).toISOString()">{{
                new Date(deployment.createdAt * 1000).toLocaleString()
              }}</time>
            </div>
            <p v-if="deployment.error" class="deployment-error">{{ deployment.error }}</p>
          </li>
        </ol>
      </section>
    </template>
  </main>
</template>
